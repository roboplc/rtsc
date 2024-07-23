use crate::{
    condvar_api::RawCondvar,
    locking::{Condvar, RawMutex},
    Error, Result,
};
use lock_api::RawMutex as RawMutexTrait;
use std::{sync::Arc, time::Duration};

struct CellValue<P> {
    current: Option<P>,
    closed: bool,
}

impl<P> Default for CellValue<P> {
    fn default() -> Self {
        Self {
            current: None,
            closed: false,
        }
    }
}

struct DataCellInner<P, M, CV> {
    value: lock_api::Mutex<M, CellValue<P>>,
    data_available: CV,
}

/// A simple data cell that can be used to pass data between threads. Acts similarly to a
/// ring-buffer channel with a capacity of 1.
pub struct DataCell<P, M = RawMutex, CV = Condvar> {
    inner: Arc<DataCellInner<P, M, CV>>,
}

impl<P, M, CV> Default for DataCell<P, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    fn default() -> Self {
        Self {
            inner: Arc::new(DataCellInner {
                value: <_>::default(),
                data_available: CV::new(),
            }),
        }
    }
}

impl<P, M, CV> Clone for DataCell<P, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<P, M, CV> DataCell<P, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar + RawCondvar<RawMutex = M>,
{
    /// Creates a new data cell
    pub fn new() -> Self {
        Self::default()
    }
    /// Closes the cell, preventing any further data from being retrieved
    pub fn close(&self) {
        let mut value = self.inner.value.lock();
        value.closed = true;
        self.inner.data_available.notify_all();
    }
    /// Returns true if the data cell is closed
    pub fn is_closed(&self) -> bool {
        let value = self.inner.value.lock();
        value.closed
    }
    /// Sets the data in the cell
    pub fn set(&self, data: P) {
        let mut value = self.inner.value.lock();
        value.current = Some(data);
        self.inner.data_available.notify_one();
    }
    /// Replaces the value in the cell and returns the old one if any
    pub fn replace(&self, data: P) -> Option<P> {
        let mut value = self.inner.value.lock();
        let prev = value.current.replace(data);
        self.inner.data_available.notify_one();
        prev
    }
    /// Retrieves the data from the cell
    pub fn get(&self) -> Result<P> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        loop {
            if let Some(current) = value.current.take() {
                return Ok(current);
            }
            self.inner
                .data_available
                .wait::<CellValue<P>, M>(&mut value);
        }
    }
    /// Retrieves the data from the cell with the given timeout
    pub fn get_timeout(&self, timeout: Duration) -> Result<P> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        loop {
            if let Some(current) = value.current.take() {
                return Ok(current);
            }
            if self
                .inner
                .data_available
                .wait_for::<CellValue<P>, M>(&mut value, timeout)
                .timed_out()
            {
                return Err(Error::Timeout);
            }
        }
    }
    /// Tries to retrieve the data from the cell (non-blocking)
    pub fn try_get(&self) -> Result<P> {
        let mut value = self.inner.value.lock();
        if value.closed {
            return Err(Error::ChannelClosed);
        }
        value.current.take().ok_or(Error::ChannelEmpty)
    }
}

impl<P, M, CV> Iterator for DataCell<P, M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar + RawCondvar<RawMutex = M>,
{
    type Item = P;
    fn next(&mut self) -> Option<Self::Item> {
        self.get().ok()
    }
}

#[cfg(test)]
mod test {

    use std::{thread, time::Duration};

    use crate::Error;

    use super::DataCell;

    #[test]
    fn test_datacell() {
        let cell: DataCell<_> = DataCell::new();
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            cell2.set(42);
        });
        assert_eq!(cell.get().unwrap(), 42);
        handle.join().unwrap();
    }

    #[test]
    fn test_datacell_close() {
        let cell: DataCell<_> = DataCell::new();
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            cell2.set(42);
        });
        cell.close();
        assert_eq!(cell.get().unwrap_err(), Error::ChannelClosed);
        handle.join().unwrap();
    }

    #[test]
    fn test_datacell_try_get() {
        let cell: DataCell<_> = DataCell::new();
        assert_eq!(cell.try_get().unwrap_err(), Error::ChannelEmpty);
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            cell2.set(42);
        });
        handle.join().unwrap();
        assert_eq!(cell.try_get().unwrap(), 42);
    }

    #[test]
    fn test_datacell_other_mutex() {
        let cell: DataCell<_, parking_lot_rt::RawMutex, parking_lot_rt::Condvar> = DataCell::new();
        cell.set(42);
        assert_eq!(cell.get().unwrap(), 42);
    }
}
