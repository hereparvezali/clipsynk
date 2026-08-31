pub mod discover;
pub mod errors;
pub mod frame;
pub mod transport;

pub use discover::Discovery;
pub use frame::{Frame, HandShake};
pub use transport::Transport;

pub const DEFAULT_BROADCAST_PORT: u16 = 51515;
pub const DEFAULT_CHANNEL_CAPACITY: usize = 64;
