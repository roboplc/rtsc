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

## Locking policy

All the components support both the default and custom locking policies, mean
Mutex and Condvar implementations can be replaced with 3rd party ones for each
component instance used.

This allows to use RTSC components in various environments, e.g.:

* For standard real-time: use
  [parking_lot_rt](https://crates.io/crates/parking_lot_rt) (default for all
  systems except Linux).

* For safety-critical real-time use the provided [`pi`] components which
  support priority inheritance (default for Linux, works on Linux only).

* For latency-critical real-time use spin-based locks.

* For high-load non-real-time use
  [parking_lot](https://crates.io/crates/parking_lot) components.

### The default locking policy

```rust
// For the implicit <T>
let (tx, rx) = rtsc::channel_bounded!(10);
tx.send(42).unwrap();

// For the explicit <T>
use rtsc::channel;

// The `Bounded` structure is used as a workaround to specify the default
// Mutex/Condvar, being destructuring to the sender and the receiver.
let channel::Bounded { tx, rx } = channel::Bounded::<i32>::new(10);
```

On Linux the crate uses built-in priority-inheritance [`pi::Mutex`]
implementation and re-exports this module as `locking`.

On other platforms, all components use Mutex/Condvar synchronization primitives
from [parking_lot_rt](https://crates.io/crates/parking_lot_rt) - a real-time
fork of the well-known [parking_lot](https://crates.io/crates/parking_lot)
crate. This is a relatively safe Mutex/RwLock with minimal user-space
spin-waiting. The module is re-exported as [`locking`] as well.

### Custom locking policy

The crate components provide API to specify 3rd party Mutex/Condvar
implementation.

```rust
// Forcibly use the parking_lot_rt Mutex/Condvar
let (tx, rx) = rtsc::channel::bounded::<i32,
    parking_lot_rt::RawMutex, parking_lot_rt::Condvar>(1);
```

Note: asynchronous channels use `parking_lot_rt` locking only.

#### Supported mutexes

All mutexes, which implement `locking_api::RawMutex` trait from the
[lock_api](https://crates.io/crates/lock_api) crate.

#### Supported condition variables

For the condition variables, the trait [`condvar_api::RawCondvar`] must
be implemented.

The trait is automatically implemented for:

* The Built-in locks provided

* `parking_lot::Condvar` (requires `parking_lot` feature)

## References

RTSC is a part of [RoboPLC](https://www.roboplc.com) project.
