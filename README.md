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

By default, all components use Mutex/Condvar synchronization primitives from
[parking_lot_rt](https://crates.io/crates/parking_lot_rt) This is a relatively
safe real-time Mutex with minimal user-space spin-waiting.

For mission-critical applications, it is recommended to switch the crate to use
an included priority-inheritance Mutex implementation (see [`pi::Mutex`]). The
Mutex is available with no extra features, however IT IS NOT TURNED ON BY
DEFAULT.

To turn on the priority-inheritance Mutex, disable the default features and
enable `pi-locking`:

```
cargo add rtsc --no-default-features --features pi-locking
```

Notes:

* The priority-inheritance Mutex is slower than the default one.

* The priority-inheritance Mutex is available for Linux only.

## References

RTSC is a part of [RoboPLC](https://www.roboplc.com) project.
