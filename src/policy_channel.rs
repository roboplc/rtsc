use crate::{
    base_channel::{make_channel, BaseChannel, BaseReceiver, BaseSender, ChannelStorage},
    condvar_api::RawCondvar,
    data_policy::{DataDeliveryPolicy, StorageTryPushOutput},
    locking::{Condvar, RawMutex},
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
pub type Sender<T, M, CV> = BaseSender<T, pdeque::Deque<T>, M, CV>;

/// Channel receiver
pub type Receiver<T, M, CV> = BaseReceiver<T, pdeque::Deque<T>, M, CV>;

/// Bounded policy channel structure. Used to be destructurized into a sender and a receiver. A
/// workaround to let the user use the default Mutex and Condvar types if others are not required
///
/// # Panics
///
/// Will panic if the capacity is zero
pub struct Bounded<T, M = RawMutex, CV = Condvar>
where
    T: DataDeliveryPolicy,
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
    T: DataDeliveryPolicy,
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

/// Bounded and ordered policy channel structure. Used to be destructurized into a sender and a
/// receiver. A workaround to let the user use the default Mutex and Condvar types if others are
/// not required
///
/// # Panics
///
/// Will panic if the capacity is zero
pub struct Ordered<T, M = RawMutex, CV = Condvar>
where
    T: DataDeliveryPolicy,
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    /// Channel sender
    pub tx: Sender<T, M, CV>,
    /// Channel receiver
    pub rx: Receiver<T, M, CV>,
}

impl<T, M, CV> Ordered<T, M, CV>
where
    T: DataDeliveryPolicy,
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    /// Create a new bounded channel
    pub fn new(capacity: usize) -> Self {
        let ch = BaseChannel::new(capacity, true);
        let (tx, rx) = make_channel(ch);
        Self { tx, rx }
    }
}

/// Creates a bounded sync channel which respects [`DataDeliveryPolicy`] rules with no message
/// priority ordering
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn bounded<T, M, CV>(capacity: usize) -> (Sender<T, M, CV>, Receiver<T, M, CV>)
where
    T: DataDeliveryPolicy,
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    let Bounded { tx, rx } = Bounded::new(capacity);
    (tx, rx)
}

/// Creates a bounded channel which respects [`DataDeliveryPolicy`] rules and has got message
/// priority ordering turned on
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn ordered<T, M, CV>(capacity: usize) -> (Sender<T, M, CV>, Receiver<T, M, CV>)
where
    T: DataDeliveryPolicy,
    M: lock_api::RawMutex,
    CV: RawCondvar,
{
    let Ordered { tx, rx } = Ordered::new(capacity);
    (tx, rx)
}

/// Create a new bounded policy channel and automatically destructurize it into a sender and a
/// receiver
#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! policy_channel_bounded {
    ($capacity: expr) => {{
        let $crate::policy_channel::Bounded { tx, rx } =
            $crate::policy_channel::Bounded::<_>::new($capacity);
        (tx, rx)
    }};
}

/// Create a new bounded and ordered policy channel and automatically destructurize it into a
/// sender and a receiver
#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! policy_channel_bounded_ordered {
    ($capacity: expr) => {{
        let $crate::policy_channel::Ordered { tx, rx } =
            $crate::policy_channel::Ordered::<_>::new($capacity);
        (tx, rx)
    }};
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::{
        data_policy::{DataDeliveryPolicy, DeliveryPolicy},
        Error,
    };

    use crate::policy_channel_bounded;

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
        let (tx, rx) = policy_channel_bounded!(1);
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
        let (tx, rx) = policy_channel_bounded!(512);
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
            let super::Bounded { tx, rx }: super::Bounded<Message> = super::Bounded::new(512);
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
        let (tx, rx) =
            super::bounded::<usize, parking_lot_rt::RawMutex, parking_lot_rt::Condvar>(1);
        tx.send(42).unwrap();
        assert_eq!(rx.recv().unwrap(), 42);
        assert!(tx.is_empty());
        assert!(rx.is_empty());
    }
}
