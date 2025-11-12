use std::{
    collections::{BTreeSet, VecDeque},
    future::Future,
    marker::PhantomData,
    mem,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
    time::Duration,
};

use crate::{base_channel::ChannelStorage, data_policy::StorageTryPushOutput, Error, Result};
use object_id::UniqueId;
use parking_lot_rt::{Condvar, Mutex};
use pin_project::{pin_project, pinned_drop};

type ClientId = usize;

/// Base async channel
pub struct BaseChannelAsync<T: Sized, S: ChannelStorage<T>>(pub(crate) Arc<ChannelInner<T, S>>);

impl<T: Sized, S: ChannelStorage<T>> BaseChannelAsync<T, S> {
    fn id(&self) -> usize {
        self.0.id.as_usize()
    }
}

impl<T: Sized, S: ChannelStorage<T>> Eq for BaseChannelAsync<T, S> {}

impl<T: Sized, S: ChannelStorage<T>> PartialEq for BaseChannelAsync<T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<T, S> Clone for BaseChannelAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub(crate) struct ChannelInner<T: Sized, S: ChannelStorage<T>> {
    id: UniqueId,
    pub(crate) data: Mutex<InnerData<T, S>>,
    next_op_id: AtomicUsize,
    space_available: Arc<Condvar>,
    data_available: Arc<Condvar>,
}

impl<T: Sized, S: ChannelStorage<T>> BaseChannelAsync<T, S> {
    pub(crate) fn new(capacity: usize, ordering: bool) -> Self {
        let pc = InnerData::new(capacity, ordering);
        let space_available = pc.space_available.clone();
        let data_available = pc.data_available.clone();
        Self(
            ChannelInner {
                id: <_>::default(),
                data: Mutex::new(pc),
                next_op_id: <_>::default(),
                space_available,
                data_available,
            }
            .into(),
        )
    }
    fn op_id(&self) -> usize {
        self.0.next_op_id.fetch_add(1, Ordering::SeqCst)
    }
}

pub(crate) struct InnerData<T: Sized, S: ChannelStorage<T>> {
    queue: S,
    senders: usize,
    receivers: usize,
    pub(crate) send_fut_wakers: VecDeque<Option<(Waker, ClientId)>>,
    pub(crate) send_fut_waker_ids: BTreeSet<ClientId>,
    pub(crate) send_fut_pending: BTreeSet<ClientId>,
    pub(crate) recv_fut_wakers: VecDeque<Option<(Waker, ClientId)>>,
    pub(crate) recv_fut_waker_ids: BTreeSet<ClientId>,
    pub(crate) recv_fut_pending: BTreeSet<ClientId>,
    data_available: Arc<Condvar>,
    space_available: Arc<Condvar>,
    _phatom: PhantomData<T>,
}

impl<T, S> InnerData<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn new(capacity: usize, ordering: bool) -> Self {
        assert!(capacity > 0, "channel capacity MUST be > 0");
        Self {
            queue: S::with_capacity_and_ordering(capacity, ordering),
            senders: 1,
            receivers: 1,
            send_fut_wakers: <_>::default(),
            send_fut_waker_ids: <_>::default(),
            send_fut_pending: <_>::default(),
            recv_fut_wakers: <_>::default(),
            recv_fut_waker_ids: <_>::default(),
            recv_fut_pending: <_>::default(),
            data_available: <_>::default(),
            space_available: <_>::default(),
            _phatom: PhantomData,
        }
    }

    // senders

    #[inline]
    fn notify_data_sent(&mut self) {
        self.wake_next_recv();
    }

    #[inline]
    fn wake_next_send(&mut self) {
        if let Some(w) = self.send_fut_wakers.pop_front() {
            if let Some((waker, id)) = w {
                self.send_fut_waker_ids.remove(&id);
                self.send_fut_pending.insert(id);
                waker.wake();
            } else {
                self.space_available.notify_one();
            }
        }
    }
    #[inline]
    fn wake_all_sends(&mut self) {
        self.send_fut_waker_ids.clear();
        for (waker, _) in mem::take(&mut self.send_fut_wakers).into_iter().flatten() {
            waker.wake();
        }
        self.space_available.notify_all();
    }

    #[inline]
    fn notify_send_fut_drop(&mut self, id: ClientId) {
        if let Some(pos) = self
            .send_fut_wakers
            .iter()
            .position(|w| w.as_ref().is_some_and(|(_, i)| *i == id))
        {
            self.send_fut_wakers.remove(pos);
            self.send_fut_waker_ids.remove(&id);
        }
        if self.send_fut_pending.remove(&id) {
            self.wake_next_send();
        }
    }

    #[inline]
    fn confirm_send_fut_waked(&mut self, id: ClientId) {
        self.send_fut_pending.remove(&id);
    }

    #[inline]
    fn append_send_fut_waker(&mut self, waker: Waker, id: ClientId) {
        if !self.send_fut_waker_ids.insert(id) {
            return;
        }
        self.send_fut_wakers.push_back(Some((waker, id)));
    }

    #[inline]
    fn append_send_sync_waker(&mut self) {
        // use condvar
        self.send_fut_wakers.push_back(None);
    }

    // receivers

    #[inline]
    fn notify_data_received(&mut self) {
        self.wake_next_send();
    }

    #[inline]
    fn wake_next_recv(&mut self) {
        if let Some(w) = self.recv_fut_wakers.pop_front() {
            if let Some((waker, id)) = w {
                self.recv_fut_pending.insert(id);
                self.recv_fut_waker_ids.remove(&id);
                waker.wake();
            } else {
                self.data_available.notify_one();
            }
        }
    }
    #[inline]
    fn wake_all_recvs(&mut self) {
        for (waker, _) in mem::take(&mut self.recv_fut_wakers).into_iter().flatten() {
            waker.wake();
        }
        self.recv_fut_waker_ids.clear();
        self.data_available.notify_all();
    }

    #[inline]
    fn notify_recv_fut_drop(&mut self, id: ClientId) {
        if let Some(pos) = self
            .recv_fut_wakers
            .iter()
            .position(|w| w.as_ref().is_some_and(|(_, i)| *i == id))
        {
            self.recv_fut_wakers.remove(pos);
            self.recv_fut_waker_ids.remove(&id);
        }
        if self.recv_fut_pending.remove(&id) {
            self.wake_next_recv();
        }
    }

    #[inline]
    fn confirm_recv_fut_waked(&mut self, id: ClientId) {
        // the resource is taken, remove from pending
        self.recv_fut_pending.remove(&id);
    }

    #[inline]
    fn append_recv_fut_waker(&mut self, waker: Waker, id: ClientId) {
        if !self.recv_fut_waker_ids.insert(id) {
            return;
        }
        self.recv_fut_wakers.push_back(Some((waker, id)));
    }

    #[inline]
    fn append_recv_sync_waker(&mut self) {
        // use condvar
        self.recv_fut_wakers.push_back(None);
    }
}

#[pin_project(PinnedDrop)]
struct Send<'a, T: Sized, S: ChannelStorage<T>> {
    id: usize,
    channel: &'a BaseChannelAsync<T, S>,
    queued: bool,
    value: Option<T>,
}

#[pinned_drop]
#[allow(clippy::needless_lifetimes)]
impl<'a, T: Sized, S: ChannelStorage<T>> PinnedDrop for Send<'a, T, S> {
    fn drop(self: Pin<&mut Self>) {
        if self.queued {
            self.channel.0.data.lock().notify_send_fut_drop(self.id);
        }
    }
}

impl<T, S> Future for Send<'_, T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    type Output = Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut pc = self.channel.0.data.lock();
        if self.queued {
            pc.confirm_send_fut_waked(self.id);
        }
        if pc.receivers == 0 {
            self.queued = false;
            return Poll::Ready(Err(Error::ChannelClosed));
        }
        if pc.send_fut_wakers.is_empty() || self.queued {
            let push_result = pc.queue.try_push(self.value.take().unwrap());
            if let StorageTryPushOutput::Full(val) = push_result {
                self.value = Some(val);
            } else {
                self.queued = false;
                return Poll::Ready(match push_result {
                    StorageTryPushOutput::Pushed => {
                        pc.notify_data_sent();
                        Ok(())
                    }
                    StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
                    StorageTryPushOutput::Full(_) => unreachable!(),
                });
            }
        }
        self.queued = true;
        pc.append_send_fut_waker(cx.waker().clone(), self.id);
        Poll::Pending
    }
}

/// Base async sender
#[derive(Eq, PartialEq)]
pub struct BaseSenderAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    channel: BaseChannelAsync<T, S>,
}

impl<T, S> BaseSenderAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    /// Sends a value to the channel
    #[inline]
    pub fn send(&self, value: T) -> impl Future<Output = Result<()>> + '_ {
        Send {
            id: self.channel.op_id(),
            channel: &self.channel,
            queued: false,
            value: Some(value),
        }
    }
    /// Tries to send a value to the channel
    pub fn try_send(&self, value: T) -> Result<()> {
        let mut pc = self.channel.0.data.lock();
        if pc.receivers == 0 {
            return Err(Error::ChannelClosed);
        }
        match pc.queue.try_push(value) {
            StorageTryPushOutput::Pushed => {
                pc.notify_data_sent();
                Ok(())
            }
            StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
            StorageTryPushOutput::Full(_) => Err(Error::ChannelFull),
        }
    }
    /// Sends a value to the channel in a blocking (synchronous) way
    pub fn send_blocking(&self, mut value: T) -> Result<()> {
        let mut pc = self.channel.0.data.lock();
        let pushed = loop {
            if pc.receivers == 0 {
                return Err(Error::ChannelClosed);
            }
            let push_result = pc.queue.try_push(value);
            let StorageTryPushOutput::Full(val) = push_result else {
                break push_result;
            };
            value = val;
            pc.append_send_sync_waker();
            self.channel.0.space_available.wait(&mut pc);
        };
        match pushed {
            StorageTryPushOutput::Pushed => {
                pc.notify_data_sent();
                Ok(())
            }
            StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
            StorageTryPushOutput::Full(_) => unreachable!(),
        }
    }
    /// Sends a value to the channel in a blocking (synchronous) way with a given tiemout
    pub fn send_blocking_timeout(&self, mut value: T, timeout: Duration) -> Result<()> {
        let mut pc = self.channel.0.data.lock();
        let pushed = loop {
            if pc.receivers == 0 {
                return Err(Error::ChannelClosed);
            }
            let push_result = pc.queue.try_push(value);
            let StorageTryPushOutput::Full(val) = push_result else {
                break push_result;
            };
            value = val;
            pc.append_send_sync_waker();
            if self
                .channel
                .0
                .space_available
                .wait_for(&mut pc, timeout)
                .timed_out()
            {
                return Err(Error::Timeout);
            }
        };
        pc.notify_data_sent();
        match pushed {
            StorageTryPushOutput::Pushed => Ok(()),
            StorageTryPushOutput::Skipped => Err(Error::ChannelSkipped),
            StorageTryPushOutput::Full(_) => unreachable!(),
        }
    }
    /// Returns the number of items in the channel
    #[inline]
    pub fn len(&self) -> usize {
        self.channel.0.data.lock().queue.len()
    }
    /// Returns true if the channel is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.channel.0.data.lock().queue.is_full()
    }
    /// Returns true if the channel is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.channel.0.data.lock().queue.is_empty()
    }
    /// Returns true if the channel is still alive
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.channel.0.data.lock().receivers > 0
    }
}

impl<T, S> Clone for BaseSenderAsync<T, S>
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

impl<T, S> Drop for BaseSenderAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn drop(&mut self) {
        let mut pc = self.channel.0.data.lock();
        pc.senders -= 1;
        if pc.senders == 0 {
            pc.wake_all_recvs();
        }
    }
}

struct Recv<'a, T: Sized, S: ChannelStorage<T>> {
    id: usize,
    channel: &'a BaseChannelAsync<T, S>,
    queued: bool,
}

impl<T: Sized, S: ChannelStorage<T>> Drop for Recv<'_, T, S> {
    fn drop(&mut self) {
        if self.queued {
            self.channel.0.data.lock().notify_recv_fut_drop(self.id);
        }
    }
}

impl<T, S> Future for Recv<'_, T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    type Output = Result<T>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut pc = self.channel.0.data.lock();
        if self.queued {
            pc.confirm_recv_fut_waked(self.id);
        }
        if pc.recv_fut_wakers.is_empty() || self.queued {
            if let Some(val) = pc.queue.get() {
                pc.notify_data_received();
                self.queued = false;
                return Poll::Ready(Ok(val));
            } else if pc.senders == 0 {
                self.queued = false;
                return Poll::Ready(Err(Error::ChannelClosed));
            }
        }
        self.queued = true;
        pc.append_recv_fut_waker(cx.waker().clone(), self.id);
        Poll::Pending
    }
}

/// Base async receiver
#[derive(Eq, PartialEq)]
pub struct BaseReceiverAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    pub(crate) channel: BaseChannelAsync<T, S>,
}

impl<T, S> BaseReceiverAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    /// Receives a value from the channel
    #[inline]
    pub fn recv(&self) -> impl Future<Output = Result<T>> + '_ {
        Recv {
            id: self.channel.op_id(),
            channel: &self.channel,
            queued: false,
        }
    }
    /// Tries to receive a value from the channel
    pub fn try_recv(&self) -> Result<T> {
        let mut pc = self.channel.0.data.lock();
        if let Some(val) = pc.queue.get() {
            pc.notify_data_received();
            Ok(val)
        } else if pc.senders == 0 {
            Err(Error::ChannelClosed)
        } else {
            Err(Error::ChannelEmpty)
        }
    }
    /// Receives a value from the channel in a blocking (synchronous) way
    pub fn recv_blocking(&self) -> Result<T> {
        let mut pc = self.channel.0.data.lock();
        loop {
            if let Some(val) = pc.queue.get() {
                pc.notify_data_received();
                return Ok(val);
            } else if pc.senders == 0 {
                return Err(Error::ChannelClosed);
            }
            pc.append_recv_sync_waker();
            self.channel.0.data_available.wait(&mut pc);
        }
    }
    /// Receives a value from the channel in a blocking (synchronous) way with a given timeout
    pub fn recv_blocking_timeout(&self, timeout: Duration) -> Result<T> {
        let mut pc = self.channel.0.data.lock();
        loop {
            if let Some(val) = pc.queue.get() {
                pc.notify_data_received();
                return Ok(val);
            } else if pc.senders == 0 {
                return Err(Error::ChannelClosed);
            }
            pc.append_recv_sync_waker();
            if self
                .channel
                .0
                .data_available
                .wait_for(&mut pc, timeout)
                .timed_out()
            {
                return Err(Error::Timeout);
            }
        }
    }
    /// Returns the number of items in the channel
    #[inline]
    pub fn len(&self) -> usize {
        self.channel.0.data.lock().queue.len()
    }
    /// Returns true if the channel is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.channel.0.data.lock().queue.is_full()
    }
    /// Returns true if the channel is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.channel.0.data.lock().queue.is_empty()
    }
    /// Returns true if the channel is still alive
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.channel.0.data.lock().senders > 0
    }
}

impl<T, S> Clone for BaseReceiverAsync<T, S>
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

impl<T, S> Drop for BaseReceiverAsync<T, S>
where
    T: Sized,
    S: ChannelStorage<T>,
{
    fn drop(&mut self) {
        let mut pc = self.channel.0.data.lock();
        pc.receivers -= 1;
        if pc.receivers == 0 {
            pc.wake_all_sends();
        }
    }
}

pub(crate) fn make_channel<T: Sized, S: ChannelStorage<T>>(
    ch: BaseChannelAsync<T, S>,
) -> (BaseSenderAsync<T, S>, BaseReceiverAsync<T, S>) {
    let tx = BaseSenderAsync {
        channel: ch.clone(),
    };
    let rx = BaseReceiverAsync { channel: ch };
    (tx, rx)
}
