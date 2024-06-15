use crate::{Error, Result};
use std::sync::Arc;

use parking_lot_rt::{Condvar, Mutex};

#[derive(Default)]
struct CellValue<P, S, T> {
    primary: Option<P>,
    second: Option<S>,
    third: Option<T>,
    closed: bool,
}

#[derive(Default)]
struct TripleCouplerInner<P, S, T> {
    value: Mutex<CellValue<P, S, T>>,
    data_available: Condvar,
}

#[derive(Default)]
/// Data coupler, which combines [`crate::cell::DataCell`] functionality with two additional data
/// value. The primary value is combined with the secondary, the secondary may not be present.
pub struct TripleCoupler<P, S, T> {
    inner: Arc<TripleCouplerInner<P, S, T>>,
}

impl<P, S, T> Clone for TripleCoupler<P, S, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<P, S, T> TripleCoupler<P, S, T> {
    /// Closes the cell, preventing any further data from being retrieved
    pub fn close(&self) {
        let mut value = self.inner.value.lock();
        value.closed = true;
        self.inner.data_available.notify_all();
    }
    /// Returns true if the cell is closed
    pub fn is_closed(&self) -> bool {
        let value = self.inner.value.lock();
        value.closed
    }
    /// Sets the primary value
    pub fn set(&self, data: P) {
        let mut value = self.inner.value.lock();
        value.primary = Some(data);
        self.inner.data_available.notify_one();
    }
    /// Sets the second value
    pub fn set_second(&self, data: S) {
        let mut value = self.inner.value.lock();
        value.second = Some(data);
    }
    /// Retrieves the primary and secondary values from the cell
    pub fn set_third(&self, data: T) {
        let mut value = self.inner.value.lock();
        value.third = Some(data);
    }
    /// Retrieves the primary and secondary values from the cell
    pub fn get(&self) -> Result<(P, Option<S>, Option<T>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        loop {
            if let Some(primary) = value.primary.take() {
                return Ok((primary, value.second.take(), value.third.take()));
            }
            self.inner.data_available.wait(&mut value);
        }
    }
    /// Tries to retrieve the data from the cell (non-blocking)
    pub fn try_get(&self) -> Result<(P, Option<S>, Option<T>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        let primary = value.primary.take().ok_or(Error::ChannelEmpty)?;
        Ok((primary, value.second.take(), value.third.take()))
    }
}

impl<P, S, T> Iterator for TripleCoupler<P, S, T> {
    type Item = (P, Option<S>, Option<T>);
    fn next(&mut self) -> Option<Self::Item> {
        self.get().ok()
    }
}
