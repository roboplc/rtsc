<h2>
  RTSC - Real-time Synchronization Components
  <a href="https://crates.io/crates/rtsc"><img alt="crates.io page" src="https://img.shields.io/crates/v/rtsc.svg"></img></a>
  <a href="https://docs.rs/rtsc"><img alt="docs.rs page" src="https://docs.rs/rtsc/badge.svg"></img></a>
</h2>

Requirements for data synchronization in real-time applications differ from
traditional high-load ones. The main difference is that real-time
synchronization components must carefully respect and follow the traditional
operating-system approach, avoid user-space spin-loops and other busy-waiting
techniques (see [Channels in Rust. Part
2](https://medium.com/@disserman/channels-in-rust-part-2-603721567ee6) where
such problems are clearly described).

## Components

This crate provides a pack of real-time-safe synchronization components for
various typical and custom use-cases.

* Data Buffer
* Synchronization cells
* Sync/async channels
* Policy-based channels
* Semaphore
* Time tools

## Locking

On Linux the crate uses built-in priority-inheritance [`pi::Mutex]
implementation and re-exports this module as `locking`.

On other platforms, all components use Mutex/Condvar synchronization primitives
from [parking_lot_rt](https://crates.io/crates/parking_lot_rt) - a real-time
fork of the well-known [parking_lot](https://crates.io/crates/parking_lot)
crate. This is a relatively safe Mutex/RwLock with minimal user-space
spin-waiting. The module is re-exported as `locking` as well.

## References

RTSC is a part of [RoboPLC](https://www.roboplc.com) project.
