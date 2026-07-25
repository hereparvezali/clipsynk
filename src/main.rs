use std::error::Error;

use crate::{clipboard::ClipboardManager, transport::Transport};

pub mod clipboard;
pub mod frame;
pub mod transport;
pub mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (cm, local_rx) = ClipboardManager::new().await;
    Transport::new_start(local_rx, cm).await?;
    Ok(())
}
