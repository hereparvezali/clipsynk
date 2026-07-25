use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use crate::frame::Frame;

#[derive(Debug, Default, Clone)]
pub struct ClipboardState {
    pub hash: u64,
    pub timestamp: u128,
}

#[derive(Debug, Clone)]
pub struct ClipboardManager {
    pub clipboard: Arc<Mutex<ClipboardState>>,
}

impl ClipboardManager {
    pub async fn new() -> (Self, mpsc::UnboundedReceiver<Frame>) {
        let cm = Self {
            clipboard: Arc::new(Mutex::new(ClipboardState::default())),
        };
        let local_rx = cm.watch().await;

        (cm, local_rx)
    }

    #[cfg(target_os = "linux")]
    pub async fn get_content() -> Vec<u8> {
        use std::io::Read as _;

        let mut content = Vec::new();
        wl_clipboard_rs::paste::get_contents(
            wl_clipboard_rs::paste::ClipboardType::Primary,
            wl_clipboard_rs::paste::Seat::Unspecified,
            wl_clipboard_rs::paste::MimeType::Text,
        )
        .unwrap()
        .0
        .read_to_end(&mut content)
        .unwrap();
        content
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
        wl_clipboard_rs::copy::Options::new()
            .copy(
                wl_clipboard_rs::copy::Source::Bytes(bytes.to_vec().into_boxed_slice()),
                wl_clipboard_rs::copy::MimeType::Text,
            )
            .unwrap();
        dbg!("Clipboard updated");
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

        dbg!("Clipboard updated");
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

        dbg!("Clipboard updated");
    }

    #[cfg(target_os = "linux")]
    async fn watch(&self) -> tokio::sync::mpsc::UnboundedReceiver<Frame> {
        let (local_tx, local_rx) = mpsc::unbounded_channel::<Frame>();
        let cb = self.clipboard.clone();

        tokio::spawn(async move {
            let mut clipboard_stream = wayland_clipboard_listener::WlClipboardPasteStream::init(
                wayland_clipboard_listener::WlListenType::ListenOnCopy,
            )
            .unwrap();
            while let Some(Ok(msg)) = clipboard_stream.paste_stream().next() {
                let frame = Frame::new(&msg.context.context);
                if cb.lock().await.hash == frame.hash {
                    continue;
                } else {
                    let mut cb = cb.lock().await;
                    cb.hash = frame.hash;
                    cb.timestamp = frame.timestamp;
                }
                dbg!("Watched");
                local_tx.send(frame).unwrap();
            }
        });
        local_rx
    }
    #[cfg(target_os = "windows")]
    async fn watch(&self) -> mpsc::UnboundedReceiver<Frame> {
        use clipboard_master::{CallbackResult, ClipboardHandler, Master};
        use clipboard_win::get_clipboard_string;

        let (local_tx, local_rx) = mpsc::unbounded_channel::<Frame>();
        let cb = self.clipboard.clone();

        struct Handler {
            local_tx: mpsc::UnboundedSender<Frame>,
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

                        dbg!("Watched");
                        let _ = self.local_tx.send(frame);
                    }
                }

                CallbackResult::Next
            }
        }

        tokio::task::spawn_blocking(move || {
            let mut master = Master::new(Handler { local_tx, cb });
            let _ = master.run();
        });

        local_rx
    }
    #[cfg(target_os = "macos")]
    async fn watch(&self) -> mpsc::UnboundedReceiver<Frame> {
        use clipboard_master::{CallbackResult, ClipboardHandler, Master};

        let (local_tx, local_rx) = mpsc::unbounded_channel::<Frame>();
        let cb = self.clipboard.clone();

        struct Handler {
            local_tx: mpsc::UnboundedSender<Frame>,
            cb: std::sync::Arc<tokio::sync::Mutex<ClipboardState>>,
        }

        impl ClipboardHandler for Handler {
            fn on_clipboard_change(&mut self) -> CallbackResult {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        let frame = Frame::new(text.as_bytes());

                        let mut cb = self.cb.blocking_lock();

                        if cb.hash != frame.hash {
                            cb.hash = frame.hash;
                            cb.timestamp = frame.timestamp;

                            dbg!("Watched");
                            let _ = self.local_tx.send(frame);
                        }
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
