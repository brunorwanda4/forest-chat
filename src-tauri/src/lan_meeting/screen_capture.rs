use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use base64::Engine;
use image::{ColorType, codecs::jpeg::JpegEncoder};
use serde::{Deserialize, Serialize};
use xcap::{Monitor, Window};

/// Converts a captured RGBA frame to RGB, downsampling so the long edge is at
/// most 1280px.
///
/// The JPEG encoder rejects RGBA buffers outright, and full 4K frames are far
/// more than a LAN stream needs, so both happen in the same pass.
fn to_rgb_720p(rgba: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    to_rgb_scaled(rgba, width, height, 1280)
}

/// Converts RGBA to RGB, downsampling so the long edge is at most `max_edge`.
fn to_rgb_scaled(rgba: &[u8], width: u32, height: u32, max_edge: u32) -> (Vec<u8>, u32, u32) {
    let step = (width.max(height) as f32 / max_edge as f32).max(1.0);
    let out_width = ((width as f32 / step) as u32).max(1);
    let out_height = ((height as f32 / step) as u32).max(1);

    let mut rgb = Vec::with_capacity((out_width * out_height * 3) as usize);
    for y in 0..out_height {
        let src_y = ((y as f32 * step) as u32).min(height - 1);
        for x in 0..out_width {
            let src_x = ((x as f32 * step) as u32).min(width - 1);
            let i = ((src_y * width + src_x) * 4) as usize;
            rgb.extend_from_slice(&rgba[i..i + 3]);
        }
    }

    (rgb, out_width, out_height)
}

/// What the user picked to share: a whole monitor, or one application window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum CaptureTarget {
    Monitor(u32),
    Window(u32),
}

impl Default for CaptureTarget {
    /// Sharing with no explicit choice means the primary monitor, which is what
    /// the button did before there was a picker.
    fn default() -> Self {
        let id = Monitor::all()
            .ok()
            .and_then(|monitors| {
                monitors
                    .into_iter()
                    .find(|m| m.is_primary().unwrap_or(false))
                    .or_else(|| Monitor::all().ok()?.into_iter().next())
            })
            .and_then(|m| m.id().ok())
            .unwrap_or(0);
        CaptureTarget::Monitor(id)
    }
}

/// One entry in the share picker.
#[derive(Debug, Clone, Serialize)]
pub struct ShareSource {
    pub target: CaptureTarget,
    pub title: String,
    pub app_name: String,
    /// A small JPEG preview as a data URL, or `None` if the preview failed.
    pub thumbnail: Option<String>,
}

/// Lists every monitor and visible application window the user can share.
///
/// Each entry carries a small preview so the picker looks like the one people
/// already know from Meet and Zoom. Windows that fail to capture are still
/// listed, just without a preview.
pub fn list_sources() -> Result<Vec<ShareSource>, String> {
    let mut sources = Vec::new();

    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    for monitor in monitors {
        let Ok(id) = monitor.id() else { continue };
        let name = monitor.name().unwrap_or_else(|_| format!("Display {id}"));
        let is_primary = monitor.is_primary().unwrap_or(false);

        sources.push(ShareSource {
            target: CaptureTarget::Monitor(id),
            title: if is_primary {
                format!("{name} (primary)")
            } else {
                name
            },
            app_name: "Entire screen".to_string(),
            thumbnail: monitor.capture_image().ok().and_then(|img| {
                let (w, h) = (img.width(), img.height());
                thumbnail_data_url(&img, w, h)
            }),
        });
    }

    let windows = Window::all().map_err(|e| format!("Failed to list windows: {e}"))?;
    for window in windows {
        let Ok(id) = window.id() else { continue };
        if window.is_minimized().unwrap_or(false) {
            continue;
        }

        let title = window.title().unwrap_or_default();
        let app_name = window.app_name().unwrap_or_default();
        if title.trim().is_empty() && app_name.trim().is_empty() {
            continue;
        }

        sources.push(ShareSource {
            target: CaptureTarget::Window(id),
            title: if title.trim().is_empty() {
                app_name.clone()
            } else {
                title
            },
            app_name,
            thumbnail: window.capture_image().ok().and_then(|img| {
                let (w, h) = (img.width(), img.height());
                thumbnail_data_url(&img, w, h)
            }),
        });
    }

    Ok(sources)
}

/// Encodes a preview small enough to sit in a picker grid.
fn thumbnail_data_url(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    if width == 0 || height == 0 {
        return None;
    }

    let (rgb, w, h) = to_rgb_scaled(rgba, width, height, 320);
    let mut jpeg = Vec::with_capacity(16 * 1024);
    JpegEncoder::new_with_quality(&mut Cursor::new(&mut jpeg), 55)
        .encode(&rgb, w, h, ColorType::Rgb8.into())
        .ok()?;

    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&jpeg)
    ))
}

pub struct ScreenCapturer {
    is_capturing: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl ScreenCapturer {
    pub fn new() -> Self {
        Self {
            is_capturing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::Relaxed)
    }

    /// Starts capturing the primary screen and invoking `on_frame` with JPEG compressed frame bytes.
    pub fn start_capture<F>(&self, target: CaptureTarget, on_frame: F) -> Result<(), String>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        if self.is_capturing.swap(true, Ordering::SeqCst) {
            return Ok(()); // Already capturing
        }

        let is_capturing = self.is_capturing.clone();
        let on_frame = Arc::new(on_frame);

        std::thread::spawn(move || {
            log::info!("Screen capture loop started");
            // Resolve the chosen source once; the loop then just grabs frames.
            // Monitor and Window hold raw platform handles, so they are resolved
            // here, inside the capture thread, rather than passed into it.
            let grab: Box<dyn Fn() -> Result<xcap::image::RgbaImage, String>> = match target {
                CaptureTarget::Monitor(id) => {
                    let monitor = Monitor::all()
                        .ok()
                        .and_then(|monitors| {
                            monitors.into_iter().find(|m| m.id().is_ok_and(|m| m == id))
                        })
                        .or_else(|| Monitor::all().ok()?.into_iter().next());

                    match monitor {
                        Some(monitor) => Box::new(move || {
                            monitor.capture_image().map_err(|e| e.to_string())
                        }),
                        None => {
                            log::error!("Monitor {id} is gone, cannot share it");
                            is_capturing.store(false, Ordering::Relaxed);
                            return;
                        }
                    }
                }
                CaptureTarget::Window(id) => {
                    let window = Window::all()
                        .ok()
                        .and_then(|windows| {
                            windows.into_iter().find(|w| w.id().is_ok_and(|w| w == id))
                        });

                    match window {
                        Some(window) => Box::new(move || {
                            if window.is_minimized().unwrap_or(false) {
                                // A minimised window has nothing to show; keep the
                                // last frame up rather than sending garbage.
                                return Err("window is minimised".to_string());
                            }
                            window.capture_image().map_err(|e| e.to_string())
                        }),
                        None => {
                            log::error!("Window {id} is gone, cannot share it");
                            is_capturing.store(false, Ordering::Relaxed);
                            return;
                        }
                    }
                }
            };

            while is_capturing.load(Ordering::Relaxed) {
                let start = std::time::Instant::now();

                match grab() {
                    Ok(rgba_img) => {
                        let (rgb_pixels, width, height) = to_rgb_720p(
                            &rgba_img,
                            rgba_img.width(),
                            rgba_img.height(),
                        );

                        let mut jpeg_buf = Vec::with_capacity(64 * 1024);
                        let mut cursor = Cursor::new(&mut jpeg_buf);
                        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, 60);

                        match encoder.encode(&rgb_pixels, width, height, ColorType::Rgb8.into()) {
                            Ok(()) => on_frame(jpeg_buf),
                            Err(e) => log::warn!("Screen frame encode failed: {e}"),
                        }
                    }
                    Err(e) => {
                        log::warn!("Screen frame capture failed: {e}");
                    }
                }

                // Target ~15 fps (66ms interval) for real-time responsiveness and low bandwidth
                let elapsed = start.elapsed();
                let target = Duration::from_millis(66);
                if elapsed < target {
                    std::thread::sleep(target - elapsed);
                }
            }

            log::info!("Screen capture loop ended");
        });

        Ok(())
    }

    /// Stops screen capture immediately.
    pub fn stop_capture(&self) {
        self.is_capturing.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::to_rgb_720p;

    #[test]
    fn drops_the_alpha_channel_and_keeps_small_frames_untouched() {
        let rgba = vec![1, 2, 3, 255, 4, 5, 6, 255];
        let (rgb, width, height) = to_rgb_720p(&rgba, 2, 1);

        assert_eq!((width, height), (2, 1));
        assert_eq!(rgb, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn downsamples_a_4k_frame_to_a_1280px_long_edge() {
        let (width, height) = (3840, 2160);
        let rgba = vec![7u8; (width * height * 4) as usize];
        let (rgb, out_width, out_height) = to_rgb_720p(&rgba, width, height);

        assert_eq!((out_width, out_height), (1280, 720));
        assert_eq!(rgb.len(), (1280 * 720 * 3) as usize);
    }
}
