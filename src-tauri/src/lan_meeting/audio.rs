use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc,
    },
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[allow(dead_code)]
pub struct AudioEngine {
    input_stop: Option<std_mpsc::Sender<()>>,
    output_stop: Option<std_mpsc::Sender<()>>,
    is_muted: Arc<AtomicBool>,
    output_queue: Arc<Mutex<VecDeque<f32>>>,
}

unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            input_stop: None,
            output_stop: None,
            is_muted: Arc::new(AtomicBool::new(false)),
            output_queue: Arc::new(Mutex::new(VecDeque::with_capacity(48000))),
        }
    }

    /// Starts capturing microphone audio using cpal on a dedicated OS thread.
    /// `on_samples` is called whenever a chunk of audio samples is captured.
    pub fn start_input<F>(&mut self, mut on_samples: F) -> Result<(), String>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        self.stop_input();

        let (stop_tx, stop_rx) = std_mpsc::channel::<()>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), String>>();
        let is_muted = self.is_muted.clone();

        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("No default microphone device found".to_string()));
                    return;
                }
            };

            let supported_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Failed to get microphone config: {e}")));
                    return;
                }
            };

            let channels = supported_config.channels() as usize;
            let err_fn = |err| {
                log::warn!("Microphone audio error: {err}");
            };

            let stream_res = match supported_config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if is_muted.load(Ordering::Relaxed) {
                            return;
                        }
                        if channels == 1 {
                            on_samples(data);
                        } else {
                            let mut mono = Vec::with_capacity(data.len() / channels);
                            for frame in data.chunks_exact(channels) {
                                let sum: f32 = frame.iter().copied().sum();
                                mono.push(sum / channels as f32);
                            }
                            on_samples(&mono);
                        }
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if is_muted.load(Ordering::Relaxed) {
                            return;
                        }
                        let mut float_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks_exact(channels) {
                            let sum: f32 = frame.iter().map(|&s| s as f32 / i16::MAX as f32).sum();
                            float_samples.push(sum / channels as f32);
                        }
                        on_samples(&float_samples);
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if is_muted.load(Ordering::Relaxed) {
                            return;
                        }
                        let mut float_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks_exact(channels) {
                            let sum: f32 = frame.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum();
                            float_samples.push(sum / channels as f32);
                        }
                        on_samples(&float_samples);
                    },
                    err_fn,
                    None,
                ),
                _ => {
                    let _ = ready_tx.send(Err("Unsupported microphone format".to_string()));
                    return;
                }
            };

            let stream = match stream_res {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Failed to build microphone stream: {e}")));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(format!("Failed to play microphone stream: {e}")));
                return;
            }

            let _ = ready_tx.send(Ok(()));

            // Keep the stream alive until stop signal is received
            let _ = stop_rx.recv();
            drop(stream);
        });

        match ready_rx.recv().map_err(|e| e.to_string())? {
            Ok(()) => {
                self.input_stop = Some(stop_tx);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Starts speaker output stream on a dedicated OS thread to play audio received from remote peers.
    pub fn start_output(&mut self) -> Result<(), String> {
        self.stop_output();

        let (stop_tx, stop_rx) = std_mpsc::channel::<()>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), String>>();
        let queue = self.output_queue.clone();

        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_output_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("No default speaker device found".to_string()));
                    return;
                }
            };

            let supported_config = match device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Failed to get speaker config: {e}")));
                    return;
                }
            };

            let channels = supported_config.channels() as usize;
            let err_fn = |err| {
                log::warn!("Speaker audio error: {err}");
            };

            let stream_res = match supported_config.sample_format() {
                cpal::SampleFormat::F32 => device.build_output_stream(
                    &supported_config.into(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut q = match queue.lock() {
                            Ok(g) => g,
                            Err(_) => return,
                        };
                        for frame in data.chunks_exact_mut(channels) {
                            let sample = q.pop_front().unwrap_or(0.0);
                            for ch in frame.iter_mut() {
                                *ch = sample;
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                _ => {
                    let _ = ready_tx.send(Err("Unsupported speaker format".to_string()));
                    return;
                }
            };

            let stream = match stream_res {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Failed to build speaker stream: {e}")));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(format!("Failed to play speaker stream: {e}")));
                return;
            }

            let _ = ready_tx.send(Ok(()));

            let _ = stop_rx.recv();
            drop(stream);
        });

        match ready_rx.recv().map_err(|e| e.to_string())? {
            Ok(()) => {
                self.output_stop = Some(stop_tx);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Pushes received audio samples into the speaker playback queue.
    pub fn push_incoming_audio(&self, samples: &[f32]) {
        if let Ok(mut q) = self.output_queue.lock() {
            if q.len() > 24000 {
                q.drain(..12000);
            }
            q.extend(samples.iter().copied());
        }
    }

    pub fn toggle_mute(&self) -> bool {
        let current = self.is_muted.load(Ordering::Relaxed);
        let new_state = !current;
        self.is_muted.store(new_state, Ordering::Relaxed);
        new_state
    }

    #[allow(dead_code)]
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }

    pub fn stop_input(&mut self) {
        if let Some(stop) = self.input_stop.take() {
            let _ = stop.send(());
        }
    }

    pub fn stop_output(&mut self) {
        if let Some(stop) = self.output_stop.take() {
            let _ = stop.send(());
        }
    }

    pub fn stop(&mut self) {
        self.stop_input();
        self.stop_output();
        if let Ok(mut q) = self.output_queue.lock() {
            q.clear();
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
