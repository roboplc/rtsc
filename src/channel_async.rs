use std::collections::VecDeque;

use crate::base_channel_async::{
    make_channel, BaseChannelAsync, BaseReceiverAsync, BaseSenderAsync,
};

/// Channel sender
pub type Sender<T> = BaseSenderAsync<T, VecDeque<T>>;

/// Channel receiver
pub type Receiver<T> = BaseReceiverAsync<T, VecDeque<T>>;

/// Create a new bounded async channel
///
/// # Panics
///
/// Will panic if the capacity is zero
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let ch = BaseChannelAsync::new(capacity, false);
    make_channel(ch)
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::bounded;

    #[tokio::test]
    async fn test_delivery() {
        let (tx, rx) = bounded::<u32>(1);
        tokio::spawn(async move {
            for _ in 0..10 {
                tx.send(123).await.unwrap();
                tx.send(456).await.unwrap();
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        let mut messages = Vec::new();
        while let Ok(msg) = rx.recv().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
            messages.push(msg);
        }
        insta::assert_debug_snapshot!(messages.len(), @"20");
        let data = rx.channel.0.data.lock();
        assert!(data.send_fut_wakers.is_empty());
        assert!(data.send_fut_waker_ids.is_empty());
        assert!(data.send_fut_pending.is_empty());
        assert!(data.recv_fut_wakers.is_empty());
        assert!(data.recv_fut_waker_ids.is_empty());
        assert!(data.recv_fut_pending.is_empty());
    }

    #[tokio::test]
    async fn test_tx_ordering() {
        let (tx, rx) = bounded(1);
        tx.send(0).await.unwrap();
        for i in 1..=10 {
            let tx = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(10 * i)).await;
                tx.send(i).await.unwrap();
            });
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        for i in 0..=10 {
            assert_eq!(rx.recv().await.unwrap(), i);
        }
    }

    #[tokio::test]
    async fn test_rx_ordering() {
        let (tx, rx) = bounded(1);
        let (res_tx, res_rx) = bounded(1024);
        for i in 0..10 {
            let rx = rx.clone();
            let res_tx = res_tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(10 * i)).await;
                let val = rx.recv().await.unwrap();
                res_tx.send(val).await.unwrap();
            });
        }
        for i in 0..10 {
            tx.send(i).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        for i in 0..10 {
            assert_eq!(res_rx.recv().await.unwrap(), i);
        }
    }

    #[tokio::test]
    async fn test_poisoning() {
        let n = 5_000;
        for i in 0..n {
            let (tx, rx) = bounded::<u32>(512);
            let rx_t = tokio::spawn(async move { while rx.recv().await.is_ok() {} });
            tokio::spawn(async move {
                let _t = tx;
            });
            for _ in 0..100 {
                if rx_t.is_finished() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            assert!(rx_t.is_finished(), "RX poisined {}", i);
        }
    }
}
