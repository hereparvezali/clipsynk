use std::error::Error;

use uuid::Uuid;

use crate::{clipboard::ClipboardManager, transport::Transport};

pub mod clipboard;
pub mod errors;
pub mod frame;
pub mod transport;
pub mod utils;

const BROADCAST_PORT: u16 = 51515;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let device_id = Uuid::new_v4();
    let (cm, local_rx) = ClipboardManager::new().await;
    Transport::new_start(device_id, BROADCAST_PORT, local_rx, cm).await?;
    Ok(())
}
