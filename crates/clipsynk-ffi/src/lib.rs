use clipsynk_core::{DEFAULT_BROADCAST_PORT, DEFAULT_CHANNEL_CAPACITY, Frame, Transport};
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
    local_tx: mpsc::Sender<Frame>,
    _rt: std::sync::Mutex<Option<tokio::runtime::Runtime>>,
}

#[uniffi::export]
impl ClipSynkEngine {
    /// Start the sync engine. Provides a callback for when remote frames arrive.
    #[uniffi::constructor]
    pub fn start(receiver: Box<dyn MobileClipboardReceiver>) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();

        let (local_tx, local_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);
        let (remote_tx, mut remote_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);

        let device_id = Uuid::new_v4();

        // Start networking in background
        rt.spawn(async move {
            let _ =
                Transport::new_start(device_id, DEFAULT_BROADCAST_PORT, local_rx, remote_tx).await;
        });

        // Forward remote_rx to FFI callback
        rt.spawn(async move {
            while let Some(frame) = remote_rx.recv().await {
                // Conflict resolution logic is pushed up to Kotlin/Swift layer.
                receiver.on_remote_frame(frame.hash, frame.timestamp as u64, frame.bytes);
            }
        });

        Self {
            local_tx,
            _rt: std::sync::Mutex::new(Some(rt)),
        }
    }

    /// Stops the background networking tasks explicitly.
    pub fn stop(&self) {
        let mut rt_guard = self._rt.lock().unwrap();
        *rt_guard = None; // Drops the tokio runtime, cancelling all tasks instantly
    }

    /// Called by Kotlin/Swift when local clipboard changes.
    pub fn send_local_frame(&self, bytes: Vec<u8>) {
        let frame = Frame::new(&bytes);
        let _ = self.local_tx.blocking_send(frame);
    }
}
