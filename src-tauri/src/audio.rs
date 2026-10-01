use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::Notify;

const FLAG_CONFIG: u64 = 1 << 63;
const OUTPUT_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
struct OutputDeviceState {
    active_name: Option<String>,
}

impl OutputDeviceState {
    fn needs_reopen(&mut self, observed_name: Option<String>) -> bool {
        let Some(observed_name) = observed_name else {
            self.active_name = None;
            return false;
        };

        if self.active_name.as_deref() == Some(observed_name.as_str()) {
            return false;
        }

        self.active_name = None;
        true
    }

    fn set_active(&mut self, name: String) {
        self.active_name = Some(name);
    }
}

pub struct AudioHandle {
    output: Arc<Mutex<AudioOutputState>>,
    _shutdown_tx: std::sync::mpsc::Sender<()>,
}

struct AudioOutputState {
    sink: Arc<Sink>,
    muted: bool,
}

impl AudioHandle {
    pub fn new() -> Result<Self> {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
        let (output_tx, output_rx) = std::sync::mpsc::sync_channel::<
            std::result::Result<Arc<Mutex<AudioOutputState>>, String>,
        >(1);

        std::thread::spawn(move || {
            let host = cpal::default_host();
            let (stream, sink) = match create_default_output() {
                Ok(output) => output,
                Err(e) => {
                    let _ = output_tx.send(Err(format!("Failed to open audio output: {}", e)));
                    return;
                }
            };

            let output = Arc::new(Mutex::new(AudioOutputState { sink, muted: false }));
            let _ = output_tx.send(Ok(output.clone()));

            let mut active_stream = Some(stream);
            let mut device_state = OutputDeviceState::default();
            if let Some(device_name) = host.default_output_device().and_then(|d| d.name().ok()) {
                device_state.set_active(device_name);
            }
            let mut last_error_device = None;

            loop {
                match shutdown_rx.recv_timeout(OUTPUT_DEVICE_POLL_INTERVAL) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }

                let Some(device) = host.default_output_device() else {
                    device_state.needs_reopen(None);
                    continue;
                };
                let Ok(device_name) = device.name() else {
                    device_state.needs_reopen(None);
                    continue;
                };
                if !device_state.needs_reopen(Some(device_name.clone())) {
                    continue;
                }

                match create_output(&device) {
                    Ok((stream, sink)) => {
                        let mut current = output.lock().unwrap_or_else(|e| e.into_inner());
                        sink.set_volume(if current.muted {
                            0.0
                        } else {
                            1.0
                        });
                        current.sink = sink;
                        active_stream = Some(stream);
                        drop(current);
                        device_state.set_active(device_name.clone());
                        last_error_device = None;
                        eprintln!("[audio] switched output device: {}", device_name);
                    }
                    Err(e) => {
                        if last_error_device.as_deref() != Some(device_name.as_str()) {
                            eprintln!("[audio] failed to open output device {}: {}", device_name, e);
                            last_error_device = Some(device_name);
                        }
                    }
                }
            }

            drop(active_stream);
        });

        let output = output_rx
            .recv()
            .map_err(|_| anyhow::anyhow!("Failed to initialize audio output"))?
            .map_err(anyhow::Error::msg)?;

        Ok(Self {
            output,
            _shutdown_tx: shutdown_tx,
        })
    }

    fn current_sink(&self) -> Arc<Sink> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sink
            .clone()
    }

    pub fn set_muted(&self, muted: bool) {
        let mut output = self.output.lock().unwrap_or_else(|e| e.into_inner());
        output.muted = muted;
        output.sink.set_volume(if muted { 0.0 } else { 1.0 });
    }
}

fn create_output(device: &cpal::Device) -> Result<(OutputStream, Arc<Sink>)> {
    let (stream, handle) = OutputStream::try_from_device(device)?;
    let sink = Arc::new(Sink::try_new(&handle)?);
    Ok((stream, sink))
}

fn create_default_output() -> Result<(OutputStream, Arc<Sink>)> {
    let (stream, handle) = OutputStream::try_default()?;
    let sink = Arc::new(Sink::try_new(&handle)?);
    Ok((stream, sink))
}

pub async fn stream_audio(
    mut audio_socket: TcpStream,
    audio: Arc<AudioHandle>,
    shutdown: Arc<Notify>,
) {
    let result = tokio::select! {
        r = playback_loop(&mut audio_socket, &audio) => r,
        _ = shutdown.notified() => Ok(()),
    };

    if let Err(e) = result {
        eprintln!("[audio] stream ended: {}", e);
    }
}

async fn playback_loop(socket: &mut TcpStream, audio: &AudioHandle) -> Result<()> {
    let mut packet_count: u64 = 0;

    loop {
        let mut header = [0u8; 12];
        socket.read_exact(&mut header).await?;

        let pts_flags = u64::from_be_bytes(header[0..8].try_into()?);
        let is_config = pts_flags & FLAG_CONFIG != 0;
        let size = u32::from_be_bytes(header[8..12].try_into()?) as usize;

        if size == 0 {
            continue;
        }

        let mut data = vec![0u8; size];
        socket.read_exact(&mut data).await?;

        if is_config {
            eprintln!("[audio] config packet: {} bytes", size);
            continue;
        }

        packet_count += 1;
        let sink = audio.current_sink();
        if packet_count <= 3 || packet_count % 500 == 0 {
            eprintln!(
                "[audio] packet #{}: {} bytes, sink queue: {}",
                packet_count,
                size,
                sink.len()
            );
        }

        // A vanished output device cannot drain its sink. Drop new packets instead of
        // blocking in Sink::clear(), so a replacement sink can receive later packets.
        if sink.len() >= 8 {
            continue;
        }

        let samples: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();

        if samples.is_empty() {
            continue;
        }

        let source = SamplesBuffer::new(2, 48000, samples);
        sink.append(source);
    }
}

#[cfg(test)]
mod tests {
    use super::OutputDeviceState;

    #[test]
    fn requests_reopen_when_default_output_changes() {
        let mut state = OutputDeviceState::default();

        assert!(state.needs_reopen(Some("Speakers".to_string())));
        state.set_active("Speakers".to_string());
        assert!(!state.needs_reopen(Some("Speakers".to_string())));
        assert!(state.needs_reopen(Some("Headphones".to_string())));
        assert!(state.needs_reopen(Some("Speakers".to_string())));
    }

    #[test]
    fn requests_reopen_when_default_device_returns_after_disappearing() {
        let mut state = OutputDeviceState::default();

        state.set_active("Speakers".to_string());
        assert!(!state.needs_reopen(None));
        assert!(state.needs_reopen(Some("Speakers".to_string())));
    }
}
