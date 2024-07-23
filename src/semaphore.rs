use std::sync::Arc;

use crate::{
    condvar_api::RawCondvar,
    locking::{Condvar, RawMutex},
};

use lock_api::RawMutex as RawMutexTrait;

/// A lightweight real-time safe semaphore
pub struct Semaphore<M = RawMutex, CV = Condvar> {
    inner: Arc<SemaphoreInner<M, CV>>,
}

impl<M, CV> Semaphore<M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar + RawCondvar<RawMutex = M>,
{
    /// Creates a new semaphore with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(SemaphoreInner {
                permissions: <_>::default(),
                capacity,
                cv: CV::new(),
            }),
        }
    }
    /// Tries to acquire permission, returns None if failed
    pub fn try_acquire(&self) -> Option<SemaphoreGuard<M, CV>> {
        let mut count = self.inner.permissions.lock();
        if *count == self.inner.capacity {
            return None;
        }
        *count += 1;
        Some(SemaphoreGuard {
            inner: self.inner.clone(),
        })
    }
    /// Acquires permission, blocks until it is available
    pub fn acquire(&self) -> SemaphoreGuard<M, CV> {
        let mut count = self.inner.permissions.lock();
        while *count == self.inner.capacity {
            self.inner.cv.wait::<usize, M>(&mut count);
        }
        *count += 1;
        SemaphoreGuard {
            inner: self.inner.clone(),
        }
    }
    /// Returns the capacity of the semaphore
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }
    /// Returns the number of available permissions
    pub fn available(&self) -> usize {
        self.inner.capacity - *self.inner.permissions.lock()
    }
    /// Returns the number of used permissions
    pub fn used(&self) -> usize {
        *self.inner.permissions.lock()
    }
    /// For tests only
    #[allow(dead_code)]
    fn is_poisoned(&self) -> bool {
        *self.inner.permissions.lock() > self.inner.capacity
    }
}

struct SemaphoreInner<M, CV> {
    permissions: lock_api::Mutex<M, usize>,
    capacity: usize,
    cv: CV,
}

impl<M, CV> SemaphoreInner<M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    fn release(&self) {
        let mut count = self.permissions.lock();
        *count -= 1;
        self.cv.notify_one();
    }
}

#[allow(clippy::module_name_repetitions)]
/// A guard that releases the permission when dropped
pub struct SemaphoreGuard<M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    inner: Arc<SemaphoreInner<M, CV>>,
}

impl<M, CV> Drop for SemaphoreGuard<M, CV>
where
    M: RawMutexTrait,
    CV: RawCondvar,
{
    fn drop(&mut self) {
        self.inner.release();
    }
}

#[cfg(test)]
mod test {
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_semaphore() {
        let sem: Semaphore = Semaphore::new(2);
        assert_eq!(sem.capacity(), 2);
        assert_eq!(sem.available(), 2);
        assert_eq!(sem.used(), 0);
        let _g1 = sem.acquire();
        assert_eq!(sem.available(), 1);
        assert_eq!(sem.used(), 1);
        let _g2 = sem.acquire();
        assert_eq!(sem.available(), 0);
        assert_eq!(sem.used(), 2);
        let g3 = sem.try_acquire();
        assert!(g3.is_none());
        drop(_g1);
        assert_eq!(sem.available(), 1);
        assert_eq!(sem.used(), 1);
        let _g4 = sem.acquire();
        assert_eq!(sem.available(), 0);
        assert_eq!(sem.used(), 2);
    }
    #[test]
    fn test_semaphore_multithread() {
        let start = Instant::now();
        let sem: Semaphore = Semaphore::new(10);
        let mut tasks = Vec::new();
        for _ in 0..100 {
            let perm = sem.acquire();
            tasks.push(std::thread::spawn(move || {
                let _perm = perm;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }));
        }
        'outer: loop {
            for task in &tasks {
                std::hint::spin_loop();
                assert!(!sem.is_poisoned(), "Semaphore is poisoned");
                if !task.is_finished() {
                    continue 'outer;
                }
            }
            break 'outer;
        }
        assert!(start.elapsed().as_millis() > 10);
    }
    #[test]
    fn test_semaphore_other_mutex() {
        let sem: Semaphore<parking_lot_rt::RawMutex, parking_lot_rt::Condvar> = Semaphore::new(2);
        assert_eq!(sem.capacity(), 2);
    }
}
