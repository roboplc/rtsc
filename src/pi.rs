use std::{
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
    thread,
    time::{Duration, Instant},
};

use linux_futex::{AsFutex as _, Futex, PiFutex, Private, TimedWaitError, WaitError};
use lock_api::{GuardSend, RawMutex, RawMutexTimed};

thread_local! {
    #[allow(clippy::cast_possible_truncation)]
    static TID: libc::pid_t = unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

#[inline]
fn tid() -> libc::pid_t {
    TID.with(|it| *it)
}

/// Result, returned by [`Condvar::wait_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitTimeoutResult {
    timed_out: bool,
}

impl WaitTimeoutResult {
    /// Returns `true` if the wait timed out.
    pub fn timed_out(&self) -> bool {
        self.timed_out
    }
}

/// Priority-inheritance based Condvar implementation for the priority-inheritance [`Mutex`].
#[derive(Default)]
pub struct Condvar {
    waiters: AtomicI32,
    fx: AtomicU32,
}

impl Condvar {
    /// Creates a new [`Condvar`]. Unlike traditional approach, a single Condvar can be used with
    /// any number of mutexes.
    pub const fn new() -> Self {
        Self {
            waiters: AtomicI32::new(0),
            fx: AtomicU32::new(0),
        }
    }

    /// Blocks the current thread until being notified. The mutex guard is unlocked before blocking
    #[inline]
    pub fn wait<T>(&self, mutex_guard: &mut MutexGuard<T>) {
        self.wait_with_timeout(mutex_guard, None);
    }

    /// Blocks the current thread until being notified or the timeout is reached. The mutex guard
    /// is unlocked before blocking
    #[inline]
    pub fn wait_for<T>(
        &self,
        mutex_guard: &mut MutexGuard<T>,
        timeout: Duration,
    ) -> WaitTimeoutResult {
        self.wait_with_timeout(mutex_guard, Some(timeout))
    }

    fn wait_with_timeout<T>(
        &self,
        mutex_guard: &mut MutexGuard<T>,
        timeout: Option<Duration>,
    ) -> WaitTimeoutResult {
        let mutex = unsafe { lock_api::MutexGuard::<'_, PiLock, T>::mutex(mutex_guard).raw() };
        macro_rules! unlock {
            () => {
                assert!(
                    self.waiters.fetch_add(1, Ordering::SeqCst) < i32::MAX,
                    "CRITICAL: too many waiters"
                );
                if mutex.is_locked() {
                    mutex.perform_unlock();
                }
            };
        }
        let fx: &Futex<Private> = self.fx.as_futex();
        let result = if let Some(timeout) = timeout {
            let now = Instant::now();
            loop {
                let Some(remaining) = timeout.checked_sub(now.elapsed()) else {
                    break WaitTimeoutResult { timed_out: true };
                };
                unlock!();
                match fx.wait_for(0, remaining) {
                    Ok(()) => break WaitTimeoutResult { timed_out: false },
                    Err(TimedWaitError::TimedOut) => break WaitTimeoutResult { timed_out: true },
                    Err(TimedWaitError::Interrupted) => continue,
                    Err(TimedWaitError::WrongValue) => unreachable!(),
                }
            }
        } else {
            loop {
                unlock!();
                match fx.wait(0) {
                    Ok(()) => break WaitTimeoutResult { timed_out: false },
                    Err(WaitError::Interrupted) => continue,
                    Err(WaitError::WrongValue) => unreachable!(),
                }
            }
        };
        self.waiters.fetch_sub(1, Ordering::SeqCst);
        mutex.perform_lock();
        result
    }

    /// Notifies one thread waiting on this condvar.
    pub fn notify_one(&self) {
        let fx: &Futex<Private> = self.fx.as_futex();
        let mut backoff = Backoff::new();
        while self.waiters.load(Ordering::SeqCst) > 0 && fx.wake(1) == 0 {
            // there is a chance that some waiter has not been entered into the futex yet, waiting
            // for it in a tiny spin loop
            backoff.backoff();
        }
    }

    /// Notifies all threads waiting on this condvar.
    pub fn notify_all(&self) {
        let fx: &Futex<Private> = self.fx.as_futex();
        let mut backoff = Backoff::new();
        loop {
            let to_wake = self.waiters.load(Ordering::SeqCst);
            if to_wake == 0 || fx.wake(to_wake) == to_wake {
                break;
            }
            // there is a chance that some waiter has not been entered into the futex yet, waiting
            // for it in a tiny spin loop
            backoff.backoff();
        }
    }
}

// Backoff strategy for the conditional variable.
//
// If the notify thread is running with higher priority, the waiter might be blocked
// between mutex.unlock() and futex.wait(). So, we need to backoff in notify_one() and
// notify_all().
//
// It is proven by tests that yelding is not enough, as futex.wake() is a relatively expensive
// operation so it should be called as less as possible (ideal case is <=2).
//
// The initial 50us quant has been chosen as a trade-off between performance and fairness. It is
// proven to be enough to let a waiter with sched=1 to enter the futex from the first time even if
// the notify thread is spinning with sched=99 (in case if both are on the same CPU).
//
// Tested on: ARM Cortex-A53, ARM Cortex-A72
//
// The backoff time is increased by 50us each time, to make sure the loop does not block
// the waiter on different (possibly slower) CPU models.
struct Backoff {
    n: u32,
}

impl Backoff {
    fn new() -> Self {
        Self { n: 50 }
    }

    fn backoff(&mut self) {
        thread::sleep(Duration::from_micros(self.n.into()));
        if self.n < 200 {
            // max sleep time = 200us
            self.n += 25;
        }
    }
}

/// The lock implementation for the priority-inheritance based mutex.
#[allow(clippy::module_name_repetitions)]
pub struct PiLock {
    futex: PiFutex<Private>,
}

impl PiLock {
    fn perform_lock(&self) {
        let tid = tid();
        #[allow(clippy::cast_sign_loss)]
        let locked =
            self.futex
                .value
                .compare_exchange(0, tid as u32, Ordering::SeqCst, Ordering::SeqCst);

        if locked.is_err() {
            while self.futex.lock_pi().is_err() {
                thread::yield_now();
            }
        }
    }
    fn perform_try_lock(&self) -> bool {
        let tid = tid();
        #[allow(clippy::cast_sign_loss)]
        let locked =
            self.futex
                .value
                .compare_exchange(0, tid as u32, Ordering::SeqCst, Ordering::SeqCst);

        if locked.is_ok() {
            true
        } else {
            self.futex.trylock_pi().is_ok()
        }
    }
    fn perform_unlock(&self) {
        let tid = tid();
        #[allow(clippy::cast_sign_loss)]
        let fast_unlocked =
            self.futex
                .value
                .compare_exchange(tid as u32, 0, Ordering::SeqCst, Ordering::SeqCst);

        if fast_unlocked.is_err() {
            self.futex.unlock_pi();
        }
    }
    #[inline]
    fn is_locked(&self) -> bool {
        self.futex.value.load(Ordering::SeqCst) != 0
    }
}

unsafe impl RawMutex for PiLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        futex: PiFutex::new(0),
    };

    type GuardMarker = GuardSend;

    #[inline]
    fn lock(&self) {
        self.perform_lock();
    }

    #[inline]
    fn try_lock(&self) -> bool {
        self.perform_try_lock()
    }

    #[inline]
    unsafe fn unlock(&self) {
        self.perform_unlock();
    }
}

unsafe impl RawMutexTimed for PiLock {
    type Duration = Duration;

    type Instant = Instant;

    #[inline]
    fn try_lock_for(&self, timeout: Self::Duration) -> bool {
        self.try_lock_until(Self::Instant::now() + timeout)
    }

    fn try_lock_until(&self, timeout: Self::Instant) -> bool {
        let tid = tid();
        #[allow(clippy::cast_sign_loss)]
        let locked =
            self.futex
                .value
                .compare_exchange(0, tid as u32, Ordering::SeqCst, Ordering::SeqCst);

        if locked.is_ok() {
            return true;
        }

        loop {
            match self.futex.lock_pi_until(timeout) {
                Ok(()) => return true,
                Err(linux_futex::TimedLockError::TryAgain) => (),
                Err(linux_futex::TimedLockError::TimedOut) => return false,
            }
        }
    }
}

/// Priority-inheritance based mutex implementation.
pub type Mutex<T> = lock_api::Mutex<PiLock, T>;
/// Priority-inheritance based mutex guard.
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, PiLock, T>;

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread, time::Duration};

    use super::{Condvar, Mutex};

    const NUM_THREADS: usize = 100;
    const ITERS: usize = 100;

    #[test]
    fn test_mutex_lock_loop() {
        for _ in 0..ITERS {
            let mutex = Arc::new(Mutex::new(0));
            let mut handles = vec![];

            for _ in 0..NUM_THREADS {
                let m = Arc::clone(&mutex);
                handles.push(thread::spawn(move || {
                    let mut num = m.lock();
                    *num += 1;
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            assert_eq!(*mutex.lock(), NUM_THREADS);
        }
    }

    #[test]
    fn test_mutex_try_lock_loop() {
        for _ in 0..ITERS {
            let mutex = Arc::new(Mutex::new(0));
            let mut handles = vec![];

            for _ in 0..NUM_THREADS {
                let m = Arc::clone(&mutex);
                handles.push(thread::spawn(move || {
                    if let Some(mut num) = m.try_lock() {
                        *num += 1;
                    }
                }));
                thread::sleep(Duration::from_micros(100));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            assert_eq!(*mutex.try_lock().unwrap(), NUM_THREADS);
        }
    }

    #[test]
    fn test_condvar_wait_notify_one_loop() {
        for _ in 0..ITERS {
            let pair = Arc::new((Mutex::new(false), Condvar::new()));
            let mut handles = vec![];

            for _ in 0..NUM_THREADS {
                let pair_clone = Arc::clone(&pair);
                handles.push(thread::spawn(move || {
                    let (lock, cvar) = &*pair_clone;
                    let mut started = lock.lock();
                    while !*started {
                        cvar.wait(&mut started);
                    }
                }));
            }

            thread::sleep(Duration::from_millis(10));
            for _ in 0..NUM_THREADS {
                let (lock, cvar) = &*pair;
                let mut started = lock.lock();
                *started = true;
                cvar.notify_one();
            }

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn test_condvar_wait_notify_all_loop() {
        for _ in 0..ITERS {
            let pair = Arc::new((Mutex::new(false), Condvar::new()));
            let mut handles = vec![];

            for _ in 0..NUM_THREADS {
                let pair_clone = Arc::clone(&pair);
                handles.push(thread::spawn(move || {
                    let (lock, cvar) = &*pair_clone;
                    let mut started = lock.lock();
                    while !*started {
                        cvar.wait(&mut started);
                    }
                }));
            }

            thread::sleep(Duration::from_millis(10));
            {
                let (lock, cvar) = &*pair;
                let mut started = lock.lock();
                *started = true;
                cvar.notify_all();
            }

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn test_condvar_timeout_wait_loop() {
        for _ in 0..ITERS {
            let pair = Arc::new((Mutex::new(false), Condvar::new()));
            let mut handles = vec![];

            for _ in 0..NUM_THREADS {
                let pair_clone = Arc::clone(&pair);
                handles.push(thread::spawn(move || {
                    let (lock, cvar) = &*pair_clone;
                    let mut mx = lock.lock();
                    if cvar
                        .wait_for(&mut mx, Duration::from_millis(100))
                        .timed_out()
                    {
                        panic!("timed out");
                    }
                }));
            }

            thread::sleep(Duration::from_millis(50));
            {
                let (lock, cvar) = &*pair;
                let mut started = lock.lock();
                *started = true;
                cvar.notify_all();
            }

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }
}
