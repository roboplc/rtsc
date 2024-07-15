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
