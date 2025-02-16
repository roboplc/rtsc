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
/// Event map
pub mod event_map;
/// Priority-inverting-safe locking (Linux only)
#[cfg(target_os = "linux")]
pub mod pi;
/// Policy-based sync channel
pub mod policy_channel;
/// Policy-based async channel
pub mod policy_channel_async;
#[cfg(not(target_os = "linux"))]
pub use parking_lot_rt as locking;
#[cfg(target_os = "linux")]
pub use pi as locking;
/// Semaphore
pub mod semaphore;
/// System tools
pub mod system;
/// Time tools
pub mod time;
/// Timestamps
pub use bma_ts;
/// Base channel type, allows to build sync channels with a custom storage
pub mod base_channel;
/// Base async channel type, allows to build async channels with a custom storage
pub mod base_channel_async;
/// Conditional traits
pub mod condvar_api;
/// Time-limited operations
pub mod ops;
/// Policy-based deque
pub mod pdeque;
/// Thread scheduling
pub mod thread_rt;

pub use base_channel::DataChannel;

pub use rtsc_derive::DataPolicy;

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
    /// All other errors
    #[error("operation failed: {0}")]
    Failed(String),
    /// System call or internal API access denied
    #[error("access denied")]
    AccessDenied,
    /// I/O errors
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    /// CPU affinity set error
    #[error("CPU affinity set error: {0}")]
    RTSchedSetAffinity(String),
    /// Real-time priority set error
    #[error("Real-time priority set error: {0}")]
    RTSchedSetScheduler(String),
}

/// Result type
pub type Result<T> = std::result::Result<T, Error>;
