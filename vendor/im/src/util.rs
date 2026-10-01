// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Every codebase needs a `util` module.

use std::cmp::Ordering;
use std::ops::{Bound, IndexMut, Range, RangeBounds};
use std::ptr;

#[cfg(feature = "pool")]
pub(crate) use refpool::{PoolClone, PoolDefault};

// The `Ref` type is an alias for either `Rc` or `Arc`, user's choice.

// `Arc` without refpool
#[cfg(all(threadsafe))]
pub(crate) use crate::fakepool::{Arc as PoolRef, Pool, PoolClone, PoolDefault};

// `Ref` == `Arc` when threadsafe
#[cfg(threadsafe)]
pub(crate) type Ref<A> = std::sync::Arc<A>;

// `Rc` without refpool
#[cfg(all(not(threadsafe), not(feature = "pool")))]
pub(crate) use crate::fakepool::{Pool, PoolClone, PoolDefault, Rc as PoolRef};

// `Rc` with refpool
#[cfg(all(not(threadsafe), feature = "pool"))]
pub(crate) type PoolRef<A> = refpool::PoolRef<A>;
#[cfg(all(not(threadsafe), feature = "pool"))]
pub(crate) type Pool<A> = refpool::Pool<A>;

// `Ref` == `Rc` when not threadsafe
#[cfg(not(threadsafe))]
pub(crate) type Ref<A> = std::rc::Rc<A>;

pub(crate) fn clone_ref<A>(r: Ref<A>) -> A
where
    A: Clone,
{
    Ref::try_unwrap(r).unwrap_or_else(|r| (*r).clone())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Side {
    Left,
    Right,
}

/// Swap two values of anything implementing `IndexMut`.
///
/// Like `slice::swap`, but more generic.
#[allow(unsafe_code)]
pub(crate) fn swap_indices<V>(vector: &mut V, a: usize, b: usize)
where
    V: IndexMut<usize>,
    V::Output: Sized,
{
    if a == b {
        return;
    }
    // so sorry, but there's no implementation for this in std that's
    // sufficiently generic
    let pa: *mut V::Output = &mut vector[a];
    let pb: *mut V::Output = &mut vector[b];
    unsafe {
        ptr::swap(pa, pb);
    }
}

#[allow(dead_code)]
pub(crate) fn linear_search_by<'a, A, I, F>(iterable: I, mut cmp: F) -> Result<usize, usize>
where
    A: 'a,
    I: IntoIterator<Item = &'a A>,
    F: FnMut(&A) -> Ordering,
{
    let mut pos = 0;
    for value in iterable {
        match cmp(value) {
            Ordering::Equal => return Ok(pos),
            Ordering::Greater => return Err(pos),
            Ordering::Less => {}
        }
        pos += 1;
    }
    Err(pos)
}

pub(crate) fn to_range<R>(range: &R, right_unbounded: usize) -> Range<usize>
where
    R: RangeBounds<usize>,
{
    let start_index = match range.start_bound() {
        Bound::Included(i) => *i,
        Bound::Excluded(i) => *i + 1,
        Bound::Unbounded => 0,
    };
    let end_index = match range.end_bound() {
        Bound::Included(i) => *i + 1,
        Bound::Excluded(i) => *i,
        Bound::Unbounded => right_unbounded,
    };
    start_index..end_index
}

macro_rules! def_pool {
    ($name:ident<$($arg:tt),*>, $pooltype:ty) => {
        /// A memory pool for the appropriate node type.
        pub struct $name<$($arg,)*>(Pool<$pooltype>);

        impl<$($arg,)*> $name<$($arg,)*> {
            /// Create a new pool with the given size.
            pub fn new(size: usize) -> Self {
                Self(Pool::new(size))
            }

            /// Fill the pool with preallocated chunks.
            pub fn fill(&self) {
                self.0.fill();
            }

            ///Get the current size of the pool.
            pub fn pool_size(&self) -> usize {
                self.0.get_pool_size()
            }
        }

        impl<$($arg,)*> Default for $name<$($arg,)*> {
            fn default() -> Self {
                Self::new($crate::config::POOL_SIZE)
            }
        }

        impl<$($arg,)*> Clone for $name<$($arg,)*> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
    };
}
