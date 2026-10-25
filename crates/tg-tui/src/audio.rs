use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub struct AudioStatus {
    pub title: String,
    pub path: PathBuf,
    pub started_at: Instant,
    pub duration_secs: i32,
}

#[derive(Clone)]
pub struct AudioPlayer {
    current: Arc<Mutex<Option<AudioStatus>>>,
    is_playing: Arc<AtomicBool>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            current: Arc::new(Mutex::new(None)),
            is_playing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Play an audio or voice note file in the background.
    pub fn play(&self, path: &Path, title: String, duration_secs: i32) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("Audio file does not exist: {:?}", path));
        }

        self.stop();

        let path_buf = path.to_path_buf();
        let status = AudioStatus {
            title: title.clone(),
            path: path_buf.clone(),
            started_at: Instant::now(),
            duration_secs,
        };

        *self.current.lock() = Some(status);
        self.is_playing.store(true, Ordering::SeqCst);

        let is_playing_flag = self.is_playing.clone();
        let current_slot = self.current.clone();

        // 1. Try rodio first (pure-Rust playback for WAV, FLAC, Vorbis)
        let rodio_path = path_buf.clone();
        #[cfg(target_os = "windows")]
        let can_try_rodio = path.extension().and_then(|e| e.to_str()).map(|ext| {
            let ext = ext.to_lowercase();
            ext == "ogg" || ext == "wav" || ext == "flac" || ext == "mp3"
        }).unwrap_or(false);

        std::thread::spawn(move || {
            #[cfg(target_os = "windows")]
            let mut played_with_rodio = false;

            #[cfg(target_os = "windows")]
            if can_try_rodio {
                if let Ok((_stream, stream_handle)) = rodio::OutputStream::try_default() {
                    if let Ok(file) = std::fs::File::open(&rodio_path) {
                        let reader = std::io::BufReader::new(file);
                        if let Ok(source) = rodio::Decoder::new(reader) {
                            if let Ok(sink) = rodio::Sink::try_new(&stream_handle) {
                                sink.append(source);
                                played_with_rodio = true;
                                while !sink.empty() && is_playing_flag.load(Ordering::SeqCst) {
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                }
                                sink.stop();
                            }
                        }
                    }
                }
            }

            #[cfg(not(target_os = "windows"))]
            let played_with_rodio = false;

            // 2. Fallback to native OS background audio player if rodio couldn't decode (e.g. Telegram Opus in OGG container)
            if !played_with_rodio && is_playing_flag.load(Ordering::SeqCst) {
                #[cfg(target_os = "windows")]
                {
                    // Use Windows PowerShell / Windows Media Player to stream
                    let _ = std::process::Command::new("powershell")
                        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!(
                            "Start-Process -FilePath \"{}\"",
                            rodio_path.display()
                        )])
                        .spawn();
                }

                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("afplay")
                        .arg(&rodio_path)
                        .spawn();
                }

                #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                {
                    // Linux: try mpv, paplay, aplay or xdg-open
                    let mut child = std::process::Command::new("mpv")
                        .args(["--no-video", &rodio_path.to_string_lossy()])
                        .spawn()
                        .or_else(|_| {
                            std::process::Command::new("paplay").arg(&rodio_path).spawn()
                        })
                        .or_else(|_| {
                            std::process::Command::new("xdg-open").arg(&rodio_path).spawn()
                        });

                    if let Ok(ref mut proc) = child {
                        let _ = proc.wait();
                    }
                }
            }

            // Sleep up to duration if duration is known
            if duration_secs > 0 && is_playing_flag.load(Ordering::SeqCst) {
                let dur = std::time::Duration::from_secs(duration_secs as u64);
                let start = Instant::now();
                while start.elapsed() < dur && is_playing_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }

            is_playing_flag.store(false, Ordering::SeqCst);
            *current_slot.lock() = None;
        });

        Ok(())
    }

    /// Stop current audio playback.
    pub fn stop(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        *self.current.lock() = None;
    }

    /// Returns true if audio is currently actively playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    /// Check if audio is currently playing and return status display string if so.
    pub fn current_status(&self) -> Option<String> {
        if !self.is_playing.load(Ordering::SeqCst) {
            return None;
        }

        let guard = self.current.lock();
        if let Some(ref st) = *guard {
            let elapsed = st.started_at.elapsed().as_secs() as i32;
            let total = st.duration_secs.max(1);
            let elapsed_clamped = elapsed.min(total);
            Some(format!(
                "🔊 Playing: {} [{:02}:{:02} / {:02}:{:02}] (Press [s] to stop)",
                st.title,
                elapsed_clamped / 60,
                elapsed_clamped % 60,
                total / 60,
                total % 60
            ))
        } else {
            None
        }
    }
}
