use std::collections::VecDeque;

use crate::{
    base_channel::{make_channel, BaseChannel, BaseReceiver, BaseSender, ChannelStorage},
    data_policy::StorageTryPushOutput,
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
pub type Sender<T> = BaseSender<T, VecDeque<T>>;

/// Channel receiver
pub type Receiver<T> = BaseReceiver<T, VecDeque<T>>;

/// Create a new bounded channel
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let ch = BaseChannel::new(capacity, false);
    make_channel(ch)
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use super::bounded;

    #[test]
    fn test_delivery() {
        let (tx, rx) = bounded::<u32>(1);
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
        let (tx, rx) = bounded(1);
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
        let (tx, rx) = bounded(1);
        let (res_tx, res_rx) = bounded(1024);
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
            let (tx, rx) = bounded::<u32>(512);
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
}
