use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use clipsynk_core::{DEFAULT_CHANNEL_CAPACITY, Frame};

#[derive(Debug, Default, Clone)]
pub struct ClipboardState {
    pub hash: u64,
    pub timestamp: u128,
}

#[derive(Debug, Clone, Default)]
pub struct DesktopClipboard {
    pub clipboard: Arc<Mutex<ClipboardState>>,
}

impl DesktopClipboard {
    pub fn new() -> Self {
        Self {
            clipboard: Arc::new(Mutex::new(ClipboardState::default())),
        }
    }

    pub async fn start(&self) -> (mpsc::Receiver<Frame>, mpsc::Sender<Frame>) {
        let local_rx = self.watch().await;
        let (remote_tx, mut remote_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);

        let this = self.clone();
        tokio::spawn(async move {
            while let Some(frame) = remote_rx.recv().await {
                let mut cb = this.clipboard.lock().await;
                if frame.hash != cb.hash && frame.timestamp > cb.timestamp {
                    cb.hash = frame.hash;
                    cb.timestamp = frame.timestamp;
                    this.set_content(&frame.bytes).await;
                }
            }
        });

        (local_rx, remote_tx)
    }

    #[cfg(target_os = "linux")]
    pub async fn get_content() -> Vec<u8> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            use std::io::Read as _;
            let mut content = Vec::new();
            wl_clipboard_rs::paste::get_contents(
                wl_clipboard_rs::paste::ClipboardType::Regular,
                wl_clipboard_rs::paste::Seat::Unspecified,
                wl_clipboard_rs::paste::MimeType::Text,
            )
            .unwrap()
            .0
            .read_to_end(&mut content)
            .unwrap();
            content
        } else {
            tokio::task::spawn_blocking(|| {
                arboard::Clipboard::new()
                    .and_then(|mut cb| cb.get_text())
                    .map(|s| s.into_bytes())
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default()
        }
    }
    #[cfg(target_os = "windows")]
    pub async fn get_content() -> Vec<u8> {
        tokio::task::spawn_blocking(|| {
            use clipboard_win::get_clipboard_string;

            get_clipboard_string()
                .map(|s| s.into_bytes())
                .unwrap_or_default()
        })
        .await
        .unwrap()
    }
    #[cfg(target_os = "macos")]
    pub async fn get_content() -> Vec<u8> {
        tokio::task::spawn_blocking(|| {
            arboard::Clipboard::new()
                .and_then(|mut cb| cb.get_text())
                .map(|s| s.into_bytes())
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default()
    }

    #[cfg(target_os = "linux")]
    pub async fn set_content(&self, bytes: &[u8]) {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            wl_clipboard_rs::copy::Options::new()
                .copy(
                    wl_clipboard_rs::copy::Source::Bytes(bytes.to_vec().into_boxed_slice()),
                    wl_clipboard_rs::copy::MimeType::Text,
                )
                .unwrap();
        } else {
            let text = String::from_utf8_lossy(bytes).to_string();
            tokio::task::spawn_blocking(move || {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(text);
                }
            })
            .await
            .unwrap_or_default();
        }
    }
    #[cfg(target_os = "windows")]
    pub async fn set_content(&self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes).to_string();

        tokio::task::spawn_blocking(move || {
            use clipboard_win::set_clipboard_string;

            let _ = set_clipboard_string(&text);
        })
        .await
        .unwrap();
    }
    #[cfg(target_os = "macos")]
    pub async fn set_content(&self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes).to_string();

        tokio::task::spawn_blocking(move || {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(text);
            }
        })
        .await
        .unwrap_or_default();
    }

    #[cfg(target_os = "linux")]
    async fn watch(&self) -> mpsc::Receiver<Frame> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            self.watch_wayland().await
        } else {
            self.watch_x11().await
        }
    }

    #[cfg(target_os = "linux")]
    async fn watch_wayland(&self) -> mpsc::Receiver<Frame> {
        let (local_tx, local_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);
        let cb = self.clipboard.clone();

        tokio::spawn(async move {
            loop {
                match wayland_clipboard_listener::WlClipboardPasteStream::init(
                    wayland_clipboard_listener::WlListenType::ListenOnCopy,
                ) {
                    Ok(mut clipboard_stream) => {
                        while let Some(Ok(msg)) = clipboard_stream.paste_stream().next() {
                            let frame = Frame::new(&msg.context.context);
                            let mut cb = cb.lock().await;
                            if cb.hash == frame.hash {
                                continue;
                            }
                            cb.hash = frame.hash;
                            cb.timestamp = frame.timestamp;
                            let _ = local_tx.send(frame).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("[WAYLAND] Failed to connect to display server ({:?}), retrying in 2s...", e);
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        });
        local_rx
    }

    #[cfg(target_os = "linux")]
    async fn watch_x11(&self) -> mpsc::Receiver<Frame> {
        use clipboard_master::{CallbackResult, ClipboardHandler, Master};

        let (local_tx, local_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);
        let cb = self.clipboard.clone();

        struct Handler {
            local_tx: mpsc::Sender<Frame>,
            cb: std::sync::Arc<tokio::sync::Mutex<ClipboardState>>,
        }

        impl ClipboardHandler for Handler {
            fn on_clipboard_change(&mut self) -> CallbackResult {
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(text) = clipboard.get_text()
                {
                    let frame = Frame::new(text.as_bytes());
                    let mut cb = self.cb.blocking_lock();
                    if cb.hash != frame.hash {
                        cb.hash = frame.hash;
                        cb.timestamp = frame.timestamp;
                        let _ = self.local_tx.blocking_send(frame);
                    }
                }
                CallbackResult::Next
            }
        }

        tokio::task::spawn_blocking(move || {
            loop {
                match Master::new(Handler {
                    local_tx: local_tx.clone(),
                    cb: cb.clone(),
                }) {
                    Ok(mut master) => {
                        let _ = master.run();
                    }
                    Err(e) => {
                        eprintln!("[X11] Failed to connect to X11 display ({:?}), retrying in 2s...", e);
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            }
        });

        local_rx
    }
    #[cfg(target_os = "windows")]
    async fn watch(&self) -> mpsc::Receiver<Frame> {
        use clipboard_master::{CallbackResult, ClipboardHandler, Master};
        use clipboard_win::get_clipboard_string;

        let (local_tx, local_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);
        let cb = self.clipboard.clone();

        struct Handler {
            local_tx: mpsc::Sender<Frame>,
            cb: std::sync::Arc<tokio::sync::Mutex<ClipboardState>>,
        }

        impl ClipboardHandler for Handler {
            fn on_clipboard_change(&mut self) -> CallbackResult {
                if let Ok(text) = get_clipboard_string() {
                    let frame = Frame::new(text.as_bytes());

                    let mut cb = self.cb.blocking_lock();

                    if cb.hash != frame.hash {
                        cb.hash = frame.hash;
                        cb.timestamp = frame.timestamp;

                        let _ = self.local_tx.blocking_send(frame);
                    }
                }

                CallbackResult::Next
            }
        }

        tokio::task::spawn_blocking(move || {
            let mut master = Master::new(Handler { local_tx, cb }).unwrap();
            let _ = master.run();
        });

        local_rx
    }
    #[cfg(target_os = "macos")]
    async fn watch(&self) -> mpsc::Receiver<Frame> {
        use clipboard_master::{CallbackResult, ClipboardHandler, Master};

        let (local_tx, local_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);
        let cb = self.clipboard.clone();

        struct Handler {
            local_tx: mpsc::Sender<Frame>,
            cb: std::sync::Arc<tokio::sync::Mutex<ClipboardState>>,
        }

        impl ClipboardHandler for Handler {
            fn on_clipboard_change(&mut self) -> CallbackResult {
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(text) = clipboard.get_text()
                {
                    let frame = Frame::new(text.as_bytes());

                    let mut cb = self.cb.blocking_lock();

                    if cb.hash != frame.hash {
                        cb.hash = frame.hash;
                        cb.timestamp = frame.timestamp;

                        let _ = self.local_tx.blocking_send(frame);
                    }
                }

                CallbackResult::Next
            }
        }

        tokio::task::spawn_blocking(move || {
            let mut master = Master::new(Handler { local_tx, cb }).unwrap();
            let _ = master.run();
        });

        local_rx
    }
}
