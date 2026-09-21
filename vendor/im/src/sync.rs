// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub(crate) use self::lock::Lock;

#[cfg(threadsafe)]
mod lock {
    use std::sync::{Arc, Mutex, MutexGuard};

    /// Thread safe lock: just wraps a `Mutex`.
    pub(crate) struct Lock<A> {
        lock: Arc<Mutex<A>>,
    }

    impl<A> Lock<A> {
        pub(crate) fn new(value: A) -> Self {
            Lock {
                lock: Arc::new(Mutex::new(value)),
            }
        }

        #[inline]
        pub(crate) fn lock(&mut self) -> Option<MutexGuard<'_, A>> {
            self.lock.lock().ok()
        }
    }

    impl<A> Clone for Lock<A> {
        fn clone(&self) -> Self {
            Lock {
                lock: self.lock.clone(),
            }
        }
    }
}

#[cfg(not(threadsafe))]
mod lock {
    use std::cell::{RefCell, RefMut};
    use std::rc::Rc;

    /// Single threaded lock: a `RefCell` so we should safely panic if somehow
    /// trying to access the stored data twice from the same thread.
    pub(crate) struct Lock<A> {
        lock: Rc<RefCell<A>>,
    }

    impl<A> Lock<A> {
        pub(crate) fn new(value: A) -> Self {
            Lock {
                lock: Rc::new(RefCell::new(value)),
            }
        }

        #[inline]
        pub(crate) fn lock(&mut self) -> Option<RefMut<'_, A>> {
            self.lock.try_borrow_mut().ok()
        }
    }

    impl<A> Clone for Lock<A> {
        fn clone(&self) -> Self {
            Lock {
                lock: self.lock.clone(),
            }
        }
    }
}
