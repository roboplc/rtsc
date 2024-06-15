use crate::{
    base_channel::{make_channel, BaseChannel, BaseReceiver, BaseSender, ChannelStorage},
    data_policy::{DataDeliveryPolicy, StorageTryPushOutput},
    pdeque,
};

impl<T> ChannelStorage<T> for pdeque::Deque<T>
where
    T: DataDeliveryPolicy,
{
    fn with_capacity_and_ordering(capacity: usize, ordering: bool) -> Self
    where
        Self: Sized,
    {
        pdeque::Deque::bounded(capacity).set_ordering(ordering)
    }

    fn try_push(&mut self, value: T) -> StorageTryPushOutput<T> {
        Self::try_push(self, value)
    }

    fn get(&mut self) -> Option<T> {
        Self::get(self)
    }

    fn len(&self) -> usize {
        Self::len(self)
    }

    fn is_full(&self) -> bool {
        Self::is_full(self)
    }

    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }
}

/// Channel sender
pub type Sender<T> = BaseSender<T, pdeque::Deque<T>>;

/// Channel receiver
pub type Receiver<T> = BaseReceiver<T, pdeque::Deque<T>>;

/// Creates a bounded sync channel which respects [`DataDeliveryPolicy`] rules with no message
/// priority ordering
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>)
where
    T: DataDeliveryPolicy,
{
    let ch = BaseChannel::new(capacity, false);
    make_channel(ch)
}

/// Creates a bounded channel which respects [`DataDeliveryPolicy`] rules and has got message
/// priority ordering turned on
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn ordered<T>(capacity: usize) -> (Sender<T>, Receiver<T>)
where
    T: DataDeliveryPolicy,
{
    let ch = BaseChannel::new(capacity, true);
    make_channel(ch)
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::{
        data_policy::{DataDeliveryPolicy, DeliveryPolicy},
        Error,
    };

    use super::bounded;

    #[derive(Debug)]
    enum Message {
        #[allow(dead_code)]
        Test(usize),
        #[allow(dead_code)]
        Temperature(f64),
        Spam,
    }

    impl DataDeliveryPolicy for Message {
        fn delivery_policy(&self) -> DeliveryPolicy {
            match self {
                Message::Test(_) => DeliveryPolicy::Always,
                Message::Temperature(_) => DeliveryPolicy::Single,
                Message::Spam => DeliveryPolicy::Optional,
            }
        }
    }

    #[test]
    fn test_delivery_policy_optional() {
        let (tx, rx) = bounded::<Message>(1);
        thread::spawn(move || {
            for _ in 0..10 {
                tx.send(Message::Test(123)).unwrap();
                if let Err(e) = tx.send(Message::Spam) {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
                tx.send(Message::Temperature(123.0)).unwrap();
            }
        });
        thread::sleep(Duration::from_secs(1));
        let mut messages = Vec::new();
        while let Ok(msg) = rx.recv() {
            thread::sleep(Duration::from_millis(10));
            if matches!(msg, Message::Spam) {
                panic!("delivery policy not respected ({:?})", msg);
            }
            messages.push(msg);
        }
        insta::assert_debug_snapshot!(messages.len(), @"20");
    }

    #[test]
    fn test_delivery_policy_single() {
        let (tx, rx) = bounded::<Message>(512);
        thread::spawn(move || {
            for _ in 0..10 {
                tx.send(Message::Test(123)).unwrap();
                if let Err(e) = tx.send(Message::Spam) {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
                tx.send(Message::Temperature(123.0)).unwrap();
            }
        });
        thread::sleep(Duration::from_secs(1));
        let mut c = 0;
        let mut t = 0;
        while let Ok(msg) = rx.recv() {
            match msg {
                Message::Test(_) => c += 1,
                Message::Temperature(_) => t += 1,
                Message::Spam => {}
            }
        }
        insta::assert_snapshot!(c, @"10");
        insta::assert_snapshot!(t, @"1");
    }

    #[test]
    fn test_poisoning() {
        let n = 5_000;
        for i in 0..n {
            let (tx, rx) = bounded::<Message>(512);
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
