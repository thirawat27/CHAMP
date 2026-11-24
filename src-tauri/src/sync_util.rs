//! Locking shared state that must outlive a panic.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Lock `mutex`, recovering it when a panic elsewhere poisoned it.
///
/// CHAMP's shared state (process manager, tunnel, add-on runtime, settings)
/// is reconciled from the real processes and files on the next status poll,
/// so a half-finished update is safer than a lock that fails forever: a
/// poisoned process manager would refuse every later start, stop, and the
/// shutdown that stops the database and PHP before CHAMP exits.
pub(crate) fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_mutex_still_locks() {
        let mutex = Mutex::new(1);
        let _ = std::panic::catch_unwind(|| {
            let _guard = mutex.lock().unwrap();
            panic!("poison the mutex");
        });
        assert!(mutex.is_poisoned());
        *lock_unpoisoned(&mutex) += 1;
        assert_eq!(*lock_unpoisoned(&mutex), 2);
    }
}
