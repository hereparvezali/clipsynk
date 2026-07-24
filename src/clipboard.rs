use std::{
    io::Read as _,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::mpsc;

use crate::{frame::Frame, utils::do_hash};

pub struct Clipboard {
    pub hash: u64,
    pub timestamp: u128,
}

impl Clipboard {
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
    pub async fn set_content(&mut self, bytes: &[u8]) {
        wl_clipboard_rs::copy::Options::new()
            .copy(
                wl_clipboard_rs::copy::Source::Bytes(bytes.to_vec().into_boxed_slice()),
                wl_clipboard_rs::copy::MimeType::Text,
            )
            .unwrap();
    }
    #[cfg(target_os = "linux")]
    async fn watch() -> tokio::sync::mpsc::UnboundedReceiver<Frame> {
        use crate::frame::Frame;

        let (tx, rx) = mpsc::unbounded_channel::<Frame>();

        tokio::spawn(async move {
            let mut clipboard_stream = wayland_clipboard_listener::WlClipboardPasteStream::init(
                wayland_clipboard_listener::WlListenType::ListenOnCopy,
            )
            .unwrap();
            while let Some(Ok(msg)) = clipboard_stream.paste_stream().next() {
                let frame = Frame::new(&msg.context.context);
                tx.send(frame).unwrap();
            }
        });
        rx
    }

    #[cfg(target_os = "windows")]
    pub async fn watch() -> tokio::sync::mpsc::UnboundedReceiver<String> {
        let (tx, rx) = mpsc::unbounded_channel::<String>();

        rx
    }
}
