#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use clipsynk_core::{DEFAULT_BROADCAST_PORT, Transport};
use uuid::Uuid;

pub mod clipboard;
use clipboard::DesktopClipboard;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_id = Uuid::new_v4();
    let clipboard = DesktopClipboard::new();
    let (local_rx, remote_tx) = clipboard.start().await;

    Transport::new_start(device_id, DEFAULT_BROADCAST_PORT, local_rx, remote_tx).await?;

    tokio::signal::ctrl_c().await?;
    println!("[INFO] Shutting down..");
    std::process::exit(0);
}
