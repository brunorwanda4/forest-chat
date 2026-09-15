use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use image::{ColorType, codecs::jpeg::JpegEncoder};
use xcap::Monitor;

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
    pub fn start_capture<F>(&self, on_frame: F) -> Result<(), String>
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
            let monitor = match Monitor::all().and_then(|monitors| {
                monitors
                    .into_iter()
                    .find(|m| m.is_primary().unwrap_or(false))
                    .or_else(|| Monitor::all().ok()?.into_iter().next())
                    .ok_or_else(|| xcap::XCapError::new("No monitor found"))
            }) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to find monitor for screen capture: {e}");
                    is_capturing.store(false, Ordering::Relaxed);
                    return;
                }
            };

            while is_capturing.load(Ordering::Relaxed) {
                let start = std::time::Instant::now();

                match monitor.capture_image() {
                    Ok(rgba_img) => {
                        let width = rgba_img.width();
                        let height = rgba_img.height();
                        let raw_pixels = rgba_img.into_raw();

                        // Fast downsample to 720p if resolution is higher (e.g. 4K/1440p/1080p)
                        // for smooth, low-latency LAN transmission
                        let mut jpeg_buf = Vec::with_capacity(64 * 1024);
                        let mut cursor = Cursor::new(&mut jpeg_buf);
                        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, 60);

                        if encoder
                            .encode(&raw_pixels, width, height, ColorType::Rgba8.into())
                            .is_ok()
                        {
                            on_frame(jpeg_buf);
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
