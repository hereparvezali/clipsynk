use std::{
    io::Read as _,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::mpsc;

use crate::{frame::Frame, utils::do_hash};

pub struct ClipboardManager {
    pub hash: u64,
    pub timestamp: u128,
}

impl ClipboardManager {
    pub async fn new() -> (Self, mpsc::UnboundedReceiver<Frame>) {
        let content = Self::get_content().await;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        let hash = do_hash(&content).await;

        let local_rx = Self::watch().await;

        (Self { hash, timestamp }, local_rx)
    }

    #[cfg(target_os = "linux")]
    pub async fn get_content() -> Vec<u8> {
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
            use clipboard_master::Clipboard;

            let mut clipboard = Clipboard::new().unwrap();

            clipboard
                .get_string()
                .map(|s| s.into_bytes())
                .unwrap_or_default()
        })
        .await
        .unwrap()
    }
    #[cfg(target_os = "linux")]
    pub async fn set_content(&mut self, bytes: &[u8]) {
        wl_clipboard_rs::copy::Options::new()
            .copy(
                wl_clipboard_rs::copy::Source::Bytes(bytes.to_vec().into_boxed_slice()),
                wl_clipboard_rs::copy::MimeType::Text,
            )
            .unwrap();
        println!("Clipboard updated");
    }
    #[cfg(target_os = "windows")]
    pub async fn set_content(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes).to_string();

        tokio::task::spawn_blocking(move || {
            use clipboard_master::Clipboard;

            let mut clipboard = Clipboard::new().unwrap();
            let _ = clipboard.set_string(text);
        })
        .await
        .unwrap();
        println!("Clipboard updated");
    }
    #[cfg(target_os = "linux")]
    async fn watch() -> tokio::sync::mpsc::UnboundedReceiver<Frame> {
        let (tx, rx) = mpsc::unbounded_channel::<Frame>();

        tokio::spawn(async move {
            let mut clipboard_stream = wayland_clipboard_listener::WlClipboardPasteStream::init(
                wayland_clipboard_listener::WlListenType::ListenOnCopy,
            )
            .unwrap();
            while let Some(Ok(msg)) = clipboard_stream.paste_stream().next() {
                let frame = Frame::new(&msg.context.context);
                println!("Watched");
                tx.send(frame).unwrap();
            }
        });
        rx
    }

    #[cfg(target_os = "windows")]
    async fn watch() -> mpsc::UnboundedReceiver<Frame> {
        use clipboard_master::{CallbackResult, ClipboardHandler, Master};

        let (tx, rx) = mpsc::unbounded_channel::<Frame>();

        struct Handler {
            tx: mpsc::UnboundedSender<Frame>,
        }

        impl ClipboardHandler for Handler {
            fn on_clipboard_change(&mut self) -> CallbackResult {
                let mut clipboard = clipboard_master::Clipboard::new().unwrap();

                if let Ok(text) = clipboard.get_string() {
                    let _ = self.tx.send(Frame::new(text.as_bytes()));
                }
                println!("Watched");

                CallbackResult::Next
            }
        }

        std::thread::spawn(move || {
            let handler = Handler { tx };
            Master::new(handler).run().unwrap();
        });

        rx
    }
}
