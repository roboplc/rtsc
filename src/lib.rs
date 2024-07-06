#![ doc = include_str!( concat!( env!( "CARGO_MANIFEST_DIR" ), "/", "README.md" ) ) ]
#![deny(missing_docs)]
/// Data buffer
pub mod buf;
/// Cell synchronization
pub mod cell;
/// Sync channel
pub mod channel;
/// Async channel
pub mod channel_async;
/// Data policies
pub mod data_policy;
/// Policy-based sync channel
pub mod pchannel;
/// Policy-based async channel
pub mod pchannel_async;
/// Priority-inverting-safe locking (Linux only)
#[cfg(target_os = "linux")]
pub mod pi;
#[cfg(not(target_os = "linux"))]
pub use parking_lot_rt as locking;
#[cfg(target_os = "linux")]
pub use pi as locking;
/// Semaphore
pub mod semaphore;
/// Time tools
pub mod time;
/// Timestamps
pub use bma_ts;
/// Base channel type, allows to build sync channels with a custom storage
pub mod base_channel;
/// Base async channel type, allows to build async channels with a custom storage
pub mod base_channel_async;
/// Time-limited operations
pub mod ops;
/// Policy-based deque
pub mod pdeque;

pub use base_channel::DataChannel;

pub use rtsc_derive::DataPolicy;

/// Error type
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
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
    /// All other errors
    #[error("operation failed: {0}")]
    Failed(String),
}

/// Result type
pub type Result<T> = std::result::Result<T, Error>;
