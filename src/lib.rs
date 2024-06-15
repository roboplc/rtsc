#![ doc = include_str!( concat!( env!( "CARGO_MANIFEST_DIR" ), "/", "README.md" ) ) ]
#![deny(missing_docs)]
/// Data buffer
pub mod buf;
/// Cell synchronization
pub mod cell;
/// Semaphore
pub mod semaphore;
/// Time tools
pub mod time;
/// Locking primitives
pub use parking_lot_rt as locking;

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("channel closed")]
    /// Channel/cell is closed
    ChannelClosed,
    #[error("channel closed")]
    /// Channel/cell is empty
    ChannelEmpty,
}

/// Result type
pub type Result<T> = std::result::Result<T, Error>;
