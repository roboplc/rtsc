use std::collections::VecDeque;

use crate::{
    base_channel::{make_channel, BaseChannel, BaseReceiver, BaseSender, ChannelStorage},
    condvar_api::RawCondvar,
    data_policy::StorageTryPushOutput,
    locking::{Condvar, RawMutex},
};

impl<T> ChannelStorage<T> for VecDeque<T>
where
    T: Sized,
{
    fn with_capacity_and_ordering(capacity: usize, ordering: bool) -> Self
    where
        Self: Sized,
    {
        assert!(!ordering, "Ordering is not supported for VecDeque");
        VecDeque::with_capacity(capacity)
    }

    fn try_push(&mut self, value: T) -> StorageTryPushOutput<T> {
        if self.len() == self.capacity() {
            StorageTryPushOutput::Full(value)
        } else {
            self.push_back(value);
            StorageTryPushOutput::Pushed
        }
    }

    fn get(&mut self) -> Option<T> {
        self.pop_front()
    }

    fn len(&self) -> usize {
        Self::len(self)
    }

    fn is_full(&self) -> bool {
        Self::len(self) == self.capacity()
    }

    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }
}

/// Channel sender
pub type Sender<T, M, CV> = BaseSender<T, VecDeque<T>, M, CV>;

/// Channel receiver
pub type Receiver<T, M, CV> = BaseReceiver<T, VecDeque<T>, M, CV>;

/// Bounded channel structure. Used to be destructurized into a sender and a receiver. A workaround
/// to let the user use the default Mutex and Condvar types if others are not required
///
/// # Panics
///
/// Will panic if the capacity is zero
pub struct Bounded<T, M = RawMutex, CV = Condvar>
where
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    /// Channel sender
    pub tx: Sender<T, M, CV>,
    /// Channel receiver
    pub rx: Receiver<T, M, CV>,
}

impl<T, M, CV> Bounded<T, M, CV>
where
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    /// Create a new bounded channel
    pub fn new(capacity: usize) -> Self {
        let ch = BaseChannel::new(capacity, false);
        let (tx, rx) = make_channel(ch);
        Self { tx, rx }
    }
}

/// Create a bounded channel and return it as a tuple of a sender and a receiver
pub fn bounded<T, M, CV>(capacity: usize) -> (Sender<T, M, CV>, Receiver<T, M, CV>)
where
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    let Bounded { tx, rx } = Bounded::new(capacity);
    (tx, rx)
}

/// Create a new bounded channel and automatically destructurize it into a sender and a receiver
#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! channel_bounded {
    ($capacity: expr) => {{
        let $crate::channel::Bounded { tx, rx } = $crate::channel::Bounded::<_>::new($capacity);
        (tx, rx)
    }};
}

#[cfg(test)]
mod test {
    use crate::channel_bounded;
    use std::{thread, time::Duration};

    #[test]
    fn test_delivery() {
        let (tx, rx) = channel_bounded!(1);
        thread::spawn(move || {
            for _ in 0..10 {
                tx.send(123).unwrap();
                tx.send(456).unwrap();
            }
        });
        thread::sleep(Duration::from_secs(1));
        let mut messages = Vec::new();
        while let Ok(msg) = rx.recv() {
            thread::sleep(Duration::from_millis(10));
            messages.push(msg);
        }
        insta::assert_debug_snapshot!(messages.len(), @"20");
    }

    #[test]
    fn test_tx_ordering() {
        let (tx, rx) = channel_bounded!(1);
        tx.send(0).unwrap();
        for i in 1..=10 {
            let tx = tx.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(10 * i));
                tx.send(i).unwrap();
            });
        }
        thread::sleep(Duration::from_millis(500));
        for i in 0..=10 {
            assert_eq!(rx.recv().unwrap(), i);
        }
    }

    #[test]
    fn test_rx_ordering() {
        let (tx, rx) = channel_bounded!(1);
        let (res_tx, res_rx) = channel_bounded!(1024);
        for i in 0..10 {
            let rx = rx.clone();
            let res_tx = res_tx.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(10 * i));
                let val = rx.recv().unwrap();
                res_tx.send(val).unwrap();
            });
        }
        for i in 0..10 {
            tx.send(i).unwrap();
        }
        thread::sleep(Duration::from_millis(500));
        for i in 0..10 {
            assert_eq!(res_rx.recv().unwrap(), i);
        }
    }

    #[test]
    fn test_poisoning() {
        let n = 5_000;
        for i in 0..n {
            let super::Bounded { tx, rx }: super::Bounded<i32> = super::Bounded::new(512);
            let rx_t = thread::spawn(move || while rx.recv().is_ok() {});
            thread::spawn(move || {
                let _t = tx;
            });
            for _ in 0..100 {
                if rx_t.is_finished() {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }
            assert!(rx_t.is_finished(), "RX poisined {}", i);
        }
    }

    #[test]
    fn test_other_mutex() {
        let (tx, rx) = super::bounded::<i32, parking_lot_rt::RawMutex, parking_lot_rt::Condvar>(1);
        tx.send(42).unwrap();
        assert_eq!(rx.recv().unwrap(), 42);
        assert!(tx.is_empty());
        assert!(rx.is_empty());
    }
}
