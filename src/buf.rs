use std::collections::VecDeque;

use crate::locking::RawMutex;

use lock_api::RawMutex as RawMutexTrait;

/// A capacity-limited thread-safe deque-based data buffer
pub struct DataBuffer<T, M = RawMutex> {
    data: lock_api::Mutex<M, VecDeque<T>>,
    capacity: usize,
    preallocated: bool,
}

impl<T, M> DataBuffer<T, M>
where
    M: RawMutexTrait,
{
    /// Creates a new bounded data buffer. The buffer is always allocated dynamically
    ///
    /// # Panics
    ///
    /// Will panic if the capacity is zero
    pub const fn bounded(capacity: usize) -> Self {
        assert!(capacity > 0, "data buffer capacity MUST be > 0");
        Self {
            data: lock_api::Mutex::const_new(M::INIT, VecDeque::new()),
            capacity,
            preallocated: false,
        }
    }
    /// Creates a new bounded pre-allocated data buffer. the buffer is pre-allocated on creation
    /// and at taking the content
    ///
    /// # Panics
    ///
    /// Will panic if the capacity is zero
    pub fn bounded_prealloc(capacity: usize) -> Self {
        assert!(capacity > 0, "data buffer capacity MUST be > 0");
        Self {
            data: lock_api::Mutex::const_new(M::INIT, VecDeque::with_capacity(capacity)),
            capacity,
            preallocated: true,
        }
    }
    /// Tries to push the value
    /// returns the value back if not pushed
    pub fn try_push(&self, value: T) -> Option<T> {
        let mut buf = self.data.lock();
        if buf.len() >= self.capacity {
            return Some(value);
        }
        buf.push_back(value);
        None
    }
    /// Forcibly pushes the value, removing the first element if necessary
    ///
    /// returns true in case the buffer had enough capacity or false if the first element had been
    /// removed
    pub fn force_push(&self, value: T) -> bool {
        let mut buf = self.data.lock();
        let mut res = true;
        while buf.len() >= self.capacity {
            buf.pop_front();
            res = false;
        }
        buf.push_back(value);
        res
    }
    /// the current buffer length (number of elements)
    pub fn len(&self) -> usize {
        self.data.lock().len()
    }
    /// is the buffer empty
    pub fn is_empty(&self) -> bool {
        self.data.lock().is_empty()
    }
    /// takes the buffer content and keeps nothing inside
    pub fn take(&self) -> VecDeque<T> {
        std::mem::replace(
            &mut *self.data.lock(),
            if self.preallocated {
                VecDeque::with_capacity(self.capacity)
            } else {
                VecDeque::new()
            },
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_data_buffer() {
        let buf: DataBuffer<_> = DataBuffer::bounded(3);
        assert_eq!(buf.len(), 0);
        buf.try_push(1);
        assert_eq!(buf.len(), 1);
        buf.try_push(2);
        assert_eq!(buf.len(), 2);
        buf.try_push(3);
        assert_eq!(buf.len(), 3);
        buf.try_push(4);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.take(), vec![1, 2, 3]);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_data_buffer_other_mutex() {
        let buf: DataBuffer<i32, parking_lot_rt::RawMutex> = DataBuffer::bounded(3);
        assert_eq!(buf.len(), 0);
    }
}
