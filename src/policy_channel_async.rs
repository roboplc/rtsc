use crate::{
    base_channel_async::{make_channel, BaseChannelAsync, BaseReceiverAsync, BaseSenderAsync},
    data_policy::DataDeliveryPolicy,
    pdeque,
};

/// Channel sender
pub type Sender<T> = BaseSenderAsync<T, pdeque::Deque<T>>;

/// Channel receiver
pub type Receiver<T> = BaseReceiverAsync<T, pdeque::Deque<T>>;

/// Creates a bounded async channel which respects [`DataDeliveryPolicy`] rules with no message
/// priority ordering
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn bounded<T: DataDeliveryPolicy>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let ch = BaseChannelAsync::new(capacity, false);
    make_channel(ch)
}

/// Creates a bounded async channel which respects [`DataDeliveryPolicy`] rules and has got message
/// priority ordering turned on
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn ordered<T: DataDeliveryPolicy>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let ch = BaseChannelAsync::new(capacity, true);
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

    #[tokio::test]
    async fn test_delivery_policy_optional() {
        let (tx, rx) = bounded::<Message>(1);
        tokio::spawn(async move {
            for _ in 0..10 {
                tx.send(Message::Test(123)).await.unwrap();
                if let Err(e) = tx.send(Message::Spam).await {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
                tx.send(Message::Temperature(123.0)).await.unwrap();
            }
        });
        thread::sleep(Duration::from_secs(1));
        let mut messages = Vec::new();
        while let Ok(msg) = rx.recv().await {
            thread::sleep(Duration::from_millis(10));
            if matches!(msg, Message::Spam) {
                panic!("delivery policy not respected ({:?})", msg);
            }
            messages.push(msg);
        }
        insta::assert_debug_snapshot!(messages.len(), @"20");
    }

    #[tokio::test]
    async fn test_delivery_policy_single() {
        let (tx, rx) = bounded::<Message>(512);
        tokio::spawn(async move {
            for _ in 0..10 {
                tx.send(Message::Test(123)).await.unwrap();
                if let Err(e) = tx.send(Message::Spam).await {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
                tx.send(Message::Temperature(123.0)).await.unwrap();
            }
        });
        thread::sleep(Duration::from_secs(1));
        let mut c = 0;
        let mut t = 0;
        while let Ok(msg) = rx.recv().await {
            match msg {
                Message::Test(_) => c += 1,
                Message::Temperature(_) => t += 1,
                Message::Spam => {}
            }
        }
        insta::assert_snapshot!(c, @"10");
        insta::assert_snapshot!(t, @"1");
    }

    #[tokio::test]
    async fn test_sync_send_async_recv() {
        let (tx, rx) = bounded::<Message>(8);
        let tx_t = tx.clone();
        tokio::spawn(async move {
            for _ in 0..10 {
                tx.send(Message::Test(123)).await.unwrap();
                if let Err(e) = tx.send(Message::Spam).await {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
            }
        });
        tokio::task::spawn_blocking(move || {
            for _ in 0..10 {
                tx_t.send_blocking(Message::Test(123)).unwrap();
                if let Err(e) = tx_t.send_blocking(Message::Spam) {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
            }
        });
        thread::sleep(Duration::from_secs(1));
        let mut c = 0;
        while let Ok(msg) = rx.recv().await {
            if let Message::Test(_) = msg {
                c += 1;
            }
        }
        insta::assert_snapshot!(c, @"20");
    }
    #[tokio::test]
    async fn test_sync_send_sync_recv() {
        let (tx, rx) = bounded::<Message>(8);
        let tx_t = tx.clone();
        tokio::spawn(async move {
            for _ in 0..10 {
                tx.send(Message::Test(123)).await.unwrap();
                if let Err(e) = tx.send(Message::Spam).await {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
                tx.send(Message::Temperature(123.0)).await.unwrap();
            }
        });
        tokio::task::spawn_blocking(move || {
            for _ in 0..10 {
                tx_t.send_blocking(Message::Test(123)).unwrap();
                if let Err(e) = tx_t.send_blocking(Message::Spam) {
                    assert!(matches!(e, Error::ChannelSkipped), "{}", e);
                }
                tx_t.send_blocking(Message::Temperature(123.0)).unwrap();
            }
        });
        thread::sleep(Duration::from_secs(1));
        let c = tokio::task::spawn_blocking(move || {
            let mut c = 0;
            while let Ok(msg) = rx.recv_blocking() {
                if let Message::Test(_) = msg {
                    c += 1;
                }
            }
            c
        })
        .await
        .unwrap();
        insta::assert_snapshot!(c, @"20");
    }

    #[tokio::test]
    async fn test_poisoning() {
        let n = 5_000;
        for _ in 0..n {
            let (tx, rx) = bounded::<Message>(512);
            let rx_t = tokio::spawn(async move { while rx.recv().await.is_ok() {} });
            tokio::spawn(async move {
                let _t = tx;
            });
            tokio::time::timeout(Duration::from_millis(100), rx_t)
                .await
                .unwrap()
                .unwrap();
        }
    }
}
