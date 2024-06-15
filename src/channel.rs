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
        self.len()
    }

    fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

/// Channel sender
pub type Sender<T> = BaseSender<T, VecDeque<T>>;

/// Channel receiver
pub type Receiver<T> = BaseReceiver<T, VecDeque<T>>;

/// Create a new bounded channel
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
