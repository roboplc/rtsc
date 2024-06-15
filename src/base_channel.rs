use std::{marker::PhantomData, sync::Arc, time::Duration};

use crate::{data_policy::StorageTryPushOutput, Error, Result};
use object_id::UniqueId;
use parking_lot_rt::{Condvar, Mutex};

/// Channel storage trait
pub trait ChannelStorage<T: Sized> {
    /// Creates a new storage with the specified capacity and ordering
    fn with_capacity_and_ordering(capacity: usize, ordering: bool) -> Self
    where
        Self: Sized;
    /// Tries to push a value into the storage
    fn try_push(&mut self, value: T) -> StorageTryPushOutput<T>;
    /// Gets a value from the storage
    fn get(&mut self) -> Option<T>;
    /// Returns the length of the storage
    fn len(&self) -> usize;
    /// Returns true if the storage is full
    fn is_full(&self) -> bool;
    /// Returns true if the storage is empty
    fn is_empty(&self) -> bool;
}

/// An abstract trait for data channels and hubs
pub trait DataChannel<T: Sized> {
    /// Sends a value to the channel
    fn send(&self, value: T) -> Result<()>;
    /// Tries to send a value to the channel (non-blocking)
    fn try_send(&self, value: T) -> Result<()>;
    /// Receives a value from the channel
    fn recv(&self) -> Result<T>;
    /// Tries to receive a value from the channel (non-blocking)
    fn try_recv(&self) -> Result<T>;
    /// Returns true if the channel is alive
    fn is_alive(&self) -> bool {
        true
    }
}

impl<T, S> DataChannel<T> for BaseSender<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn send(&self, value: T) -> Result<()> {
        self.send(value)
    }
    fn try_send(&self, value: T) -> Result<()> {
        self.try_send(value)
    }
    fn try_recv(&self) -> Result<T> {
        Err(Error::Unimplemented)
    }
    fn recv(&self) -> Result<T> {
        Err(Error::Unimplemented)
    }
    fn is_alive(&self) -> bool {
        self.is_alive()
    }
}

impl<T, S> DataChannel<T> for BaseReceiver<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn send(&self, _value: T) -> Result<()> {
        Err(Error::Unimplemented)
    }
    fn try_send(&self, _value: T) -> Result<()> {
        Err(Error::Unimplemented)
    }
    fn try_recv(&self) -> Result<T> {
        self.try_recv()
    }
    fn recv(&self) -> Result<T> {
        self.recv()
    }
    fn is_alive(&self) -> bool {
        self.is_alive()
    }
}

/// Base channel implementation
pub struct BaseChannel<T: Sized, S: ChannelStorage<T>>(Arc<ChannelInner<T, S>>);

impl<T, S> BaseChannel<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn id(&self) -> usize {
        self.0.id.as_usize()
    }
}

impl<T, S> Eq for BaseChannel<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
}

impl<T, S> PartialEq for BaseChannel<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<T, S> Clone for BaseChannel<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

struct ChannelInner<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    id: UniqueId,
    data: Mutex<InnerData<T, S>>,
    data_available: Condvar,
    space_available: Condvar,
}

impl<T, S> ChannelInner<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn send(&self, mut value: T) -> Result<()> {
        let mut data = self.data.lock();
        let pushed = loop {
            if data.receivers == 0 {
                return Err(Error::ChannelClosed);
            }
            let push_result = data.queue.try_push(value);
            let StorageTryPushOutput::Full(val) = push_result else {
                break push_result;
            };
            value = val;
            self.space_available.wait(&mut data);
        };
        match pushed {
            StorageTryPushOutput::Pushed => {
                self.data_available.notify_one();
                Ok(())
            }
            StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
            StorageTryPushOutput::Full(_) => unreachable!(),
        }
    }
    fn send_timeout(&self, mut value: T, timeout: Duration) -> Result<()> {
        let mut pc = self.data.lock();
        let pushed = loop {
            if pc.receivers == 0 {
                return Err(Error::ChannelClosed);
            }
            let push_result = pc.queue.try_push(value);
            let StorageTryPushOutput::Full(val) = push_result else {
                break push_result;
            };
            value = val;
            if self.space_available.wait_for(&mut pc, timeout).timed_out() {
                return Err(Error::Timeout);
            }
        };
        match pushed {
            StorageTryPushOutput::Pushed => {
                self.data_available.notify_one();
                Ok(())
            }
            StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
            StorageTryPushOutput::Full(_) => unreachable!(),
        }
    }
    fn try_send(&self, value: T) -> Result<()> {
        let mut data = self.data.lock();
        if data.receivers == 0 {
            return Err(Error::ChannelClosed);
        }
        match data.queue.try_push(value) {
            StorageTryPushOutput::Pushed => {
                self.data_available.notify_one();
                Ok(())
            }
            StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
            StorageTryPushOutput::Full(_) => Err(Error::ChannelFull),
        }
    }
    fn recv(&self) -> Result<T> {
        let mut data = self.data.lock();
        loop {
            if let Some(val) = data.queue.get() {
                self.space_available.notify_one();
                return Ok(val);
            } else if data.senders == 0 {
                return Err(Error::ChannelClosed);
            }
            self.data_available.wait(&mut data);
        }
    }
    fn recv_timeout(&self, timeout: Duration) -> Result<T> {
        let mut data = self.data.lock();
        loop {
            if let Some(val) = data.queue.get() {
                self.space_available.notify_one();
                return Ok(val);
            } else if data.senders == 0 {
                return Err(Error::ChannelClosed);
            }
            if self.data_available.wait_for(&mut data, timeout).timed_out() {
                return Err(Error::Timeout);
            };
        }
    }
    fn try_recv(&self) -> Result<T> {
        let mut pc = self.data.lock();
        if let Some(val) = pc.queue.get() {
            self.space_available.notify_one();
            Ok(val)
        } else if pc.senders == 0 {
            Err(Error::ChannelClosed)
        } else {
            Err(Error::ChannelEmpty)
        }
    }
}

impl<T, S> BaseChannel<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    /// Creates a new channel with the specified capacity and ordering
    pub fn new(capacity: usize, ordering: bool) -> Self {
        Self(
            ChannelInner {
                id: <_>::default(),
                data: Mutex::new(InnerData::new(capacity, ordering)),
                data_available: Condvar::new(),
                space_available: Condvar::new(),
            }
            .into(),
        )
    }
}

struct InnerData<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    queue: S,
    senders: usize,
    receivers: usize,
    _phantom: PhantomData<T>,
}

impl<T, S> InnerData<T, S>
where
    S: ChannelStorage<T>,
{
    fn new(capacity: usize, ordering: bool) -> Self {
        assert!(capacity > 0, "channel capacity MUST be > 0");
        Self {
            queue: S::with_capacity_and_ordering(capacity, ordering),
            senders: 1,
            receivers: 1,
            _phantom: PhantomData,
        }
    }
}

/// Base channel sender
#[derive(Eq, PartialEq)]
pub struct BaseSender<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    channel: BaseChannel<T, S>,
}

impl<T, S> BaseSender<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    /// Sends a value to the channel
    #[inline]
    pub fn send(&self, value: T) -> Result<()> {
        self.channel.0.send(value)
    }
    /// Sends a value to the channel with a timeout
    #[inline]
    pub fn send_timeout(&self, value: T, timeout: Duration) -> Result<()> {
        self.channel.0.send_timeout(value, timeout)
    }
    /// Tries to send a value to the channel (non-blocking)
    #[inline]
    pub fn try_send(&self, value: T) -> Result<()> {
        self.channel.0.try_send(value)
    }
    /// Returns the length of the channel storage
    #[inline]
    pub fn len(&self) -> usize {
        self.channel.0.data.lock().queue.len()
    }
    /// Returns true if the channel storage is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.channel.0.data.lock().queue.is_full()
    }
    /// Returns true if the channel storage is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.channel.0.data.lock().queue.is_empty()
    }
    /// Returns true if the channel is alive
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.channel.0.data.lock().receivers > 0
    }
}

impl<T, S> Clone for BaseSender<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn clone(&self) -> Self {
        self.channel.0.data.lock().senders += 1;
        Self {
            channel: self.channel.clone(),
        }
    }
}

impl<T, S> Drop for BaseSender<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn drop(&mut self) {
        let mut pc = self.channel.0.data.lock();
        pc.senders -= 1;
        if pc.senders == 0 {
            self.channel.0.data_available.notify_all();
        }
    }
}

/// Base channel receiver
#[derive(Eq, PartialEq)]
pub struct BaseReceiver<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    channel: BaseChannel<T, S>,
}

impl<T, S> Iterator for BaseReceiver<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.recv().ok()
    }
}

impl<T, S> BaseReceiver<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    /// Receives a value from the channel
    #[inline]
    pub fn recv(&self) -> Result<T> {
        self.channel.0.recv()
    }
    /// Receives a value from the channel with a timeout
    #[inline]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T> {
        self.channel.0.recv_timeout(timeout)
    }
    /// Tries to receive a value from the channel (non-blocking)
    #[inline]
    pub fn try_recv(&self) -> Result<T> {
        self.channel.0.try_recv()
    }
    /// Returns the length of the channel storage
    #[inline]
    pub fn len(&self) -> usize {
        self.channel.0.data.lock().queue.len()
    }
    /// Returns true if the channel storage is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.channel.0.data.lock().queue.is_full()
    }
    /// Returns true if the channel storage is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.channel.0.data.lock().queue.is_empty()
    }
    /// Returns true if the channel is alive
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.channel.0.data.lock().senders > 0
    }
}

impl<T, S> Clone for BaseReceiver<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn clone(&self) -> Self {
        self.channel.0.data.lock().receivers += 1;
        Self {
            channel: self.channel.clone(),
        }
    }
}

impl<T, S> Drop for BaseReceiver<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn drop(&mut self) {
        let mut pc = self.channel.0.data.lock();
        pc.receivers -= 1;
        if pc.receivers == 0 {
            self.channel.0.data_available.notify_all();
        }
    }
}

pub(crate) fn make_channel<T: Sized, S: ChannelStorage<T>>(
    ch: BaseChannel<T, S>,
) -> (BaseSender<T, S>, BaseReceiver<T, S>) {
    let tx = BaseSender {
        channel: ch.clone(),
    };
    let rx = BaseReceiver { channel: ch };
    (tx, rx)
}
