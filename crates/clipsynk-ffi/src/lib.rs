use clipsynk_core::{DEFAULT_BROADCAST_PORT, Frame, Transport};
use tokio::sync::mpsc;
use uuid::Uuid;

uniffi::setup_scaffolding!();

/// Callback interface implemented by Kotlin/Swift to receive frames.
#[uniffi::export(callback_interface)]
pub trait MobileClipboardReceiver: Send + Sync {
    fn on_remote_frame(&self, hash: u64, timestamp: u64, bytes: Vec<u8>);
}

#[derive(uniffi::Object)]
pub struct ClipSynkEngine {
    local_tx: mpsc::UnboundedSender<Frame>,
}

#[uniffi::export]
impl ClipSynkEngine {
    /// Start the sync engine. Provides a callback for when remote frames arrive.
    #[uniffi::constructor]
    pub fn start(receiver: Box<dyn MobileClipboardReceiver>) -> Self {
        let (local_tx, local_rx) = mpsc::unbounded_channel::<Frame>();
        let (remote_tx, mut remote_rx) = mpsc::unbounded_channel::<Frame>();

        let device_id = Uuid::new_v4();

        // Start networking in background
        tokio::spawn(async move {
            let _ =
                Transport::new_start(device_id, DEFAULT_BROADCAST_PORT, local_rx, remote_tx).await;
        });

        // Forward remote_rx to FFI callback
        tokio::spawn(async move {
            while let Some(frame) = remote_rx.recv().await {
                // Conflict resolution logic is pushed up to Kotlin/Swift layer.
                receiver.on_remote_frame(frame.hash, frame.timestamp as u64, frame.bytes);
            }
        });

        Self { local_tx }
    }

    /// Called by Kotlin/Swift when local clipboard changes.
    pub fn send_local_frame(&self, bytes: Vec<u8>) {
        let frame = Frame::new(&bytes);
        let _ = self.local_tx.send(frame);
    }
}
