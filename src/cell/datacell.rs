use crate::{Error, Result};
use std::sync::Arc;

use parking_lot_rt::{Condvar, Mutex};

struct CellValue<P> {
    current: Option<P>,
    closed: bool,
}

struct DataCellInner<P> {
    value: Mutex<CellValue<P>>,
    data_available: Condvar,
}

/// A simple data cell that can be used to pass data between threads. Acts similarly to a
/// ring-buffer channel with a capacity of 1.
pub struct DataCell<P> {
    inner: Arc<DataCellInner<P>>,
}

impl<P> Default for DataCell<P> {
    fn default() -> Self {
        Self {
            inner: Arc::new(DataCellInner {
                value: Mutex::new(CellValue {
                    current: None,
                    closed: false,
                }),
                data_available: Condvar::new(),
            }),
        }
    }
}

impl<P> Clone for DataCell<P> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<P> DataCell<P> {
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
            self.inner.data_available.wait(&mut value);
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

impl<P> Iterator for DataCell<P> {
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
        let cell = DataCell::new();
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
        let cell = DataCell::new();
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
        let cell = DataCell::new();
        assert_eq!(cell.try_get().unwrap_err(), Error::ChannelEmpty);
        let cell2 = cell.clone();
        let handle = thread::spawn(move || {
            cell2.set(42);
        });
        handle.join().unwrap();
        assert_eq!(cell.try_get().unwrap(), 42);
    }
}
