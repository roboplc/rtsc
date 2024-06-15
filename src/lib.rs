#![ doc = include_str!( concat!( env!( "CARGO_MANIFEST_DIR" ), "/", "README.md" ) ) ]
#![deny(missing_docs)]
/// Data buffer
pub mod buf;
/// Cell synchronization
pub mod cell;
/// Channel
pub mod channel;
/// Data policies
pub mod data_policy;
/// Policy-based channel
pub mod pchannel;
/// Semaphore
pub mod semaphore;
/// Time tools
pub mod time;
/// Locking primitives
pub use parking_lot_rt as locking;
/// Policy-based deque
pub mod pdeque;

mod base_channel;

pub use base_channel::DataChannel;

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// the channel is full and the value can not be sent
    #[error("channel full")]
    ChannelFull,
    /// the channel is full, an optional value is skipped. the error can be ignored but should be
    /// logged
    #[error("channel message skipped")]
    ChannelSkipped,
    /// Channel/cell is closed
    #[error("channel closed")]
    ChannelClosed,
    /// Channel/cell is empty
    #[error("channel closed")]
    ChannelEmpty,
    /// The requested operation is not implemented
    #[error("not implemented")]
    Unimplemented,
    /// Timeouts
    #[error("timed out")]
    Timeout,
    /// Invalid data receied / parameters provided
    #[error("Invalid data")]
    InvalidData(String),
}

/// Result type
pub type Result<T> = std::result::Result<T, Error>;
