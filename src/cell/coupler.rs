use crate::{Error, Result};
use std::sync::Arc;

use parking_lot_rt::{Condvar, Mutex};

struct CellValue<P, S> {
    primary: Option<P>,
    second: Option<S>,
    closed: bool,
}

impl<P, S> Default for CellValue<P, S> {
    fn default() -> Self {
        Self {
            primary: None,
            second: None,
            closed: false,
        }
    }
}

struct CouplerInner<P, S> {
    value: Mutex<CellValue<P, S>>,
    data_available: Condvar,
}

/// Data coupler, which combines [`crate::cell::DataCell`] functionality with a secondary data
/// value. The primary value is combined with the secondary, the secondary may not be present.
pub struct Coupler<P, S> {
    inner: Arc<CouplerInner<P, S>>,
}

impl<P, S> Default for Coupler<P, S> {
    fn default() -> Self {
        Self {
            inner: Arc::new(CouplerInner {
                value: Mutex::new(CellValue::default()),
                data_available: Condvar::new(),
            }),
        }
    }
}

impl<P, S> Clone for Coupler<P, S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<P, S> Coupler<P, S> {
    /// Creates a new coupler
    pub fn new() -> Self {
        Self::default()
    }
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
    pub fn get(&self) -> Result<(P, Option<S>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        loop {
            if let Some(primary) = value.primary.take() {
                return Ok((primary, value.second.take()));
            }
            self.inner.data_available.wait(&mut value);
        }
    }
    /// Tries to retrieve the data from the cell (non-blocking)
    pub fn try_get(&self) -> Result<(P, Option<S>)> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        let primary = value.primary.take().ok_or(Error::ChannelEmpty)?;
        Ok((primary, value.second.take()))
    }
}

impl<P, S> Iterator for Coupler<P, S> {
    type Item = (P, Option<S>);
    fn next(&mut self) -> Option<Self::Item> {
        self.get().ok()
    }
}
