// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use core::iter::FusedIterator;
use core::marker::PhantomData;

use crate::types::ChunkLength;

use super::{index::RawIndex, RingBuffer};
use array_ops::HasLength;

/// A reference iterator over a `RingBuffer`.
pub struct Iter<'a, A, N>
where
    N: ChunkLength<A>,
{
    pub(crate) buffer: &'a RingBuffer<A, N>,
    pub(crate) left_index: RawIndex<N>,
    pub(crate) right_index: RawIndex<N>,
    pub(crate) remaining: usize,
}

impl<'a, A, N> Iterator for Iter<'a, A, N>
where
    N: ChunkLength<A>,
{
    type Item = &'a A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some(unsafe { &*self.buffer.ptr(self.left_index.inc()) })
        }
    }

    #[inline]
    #[must_use]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, A, N> DoubleEndedIterator for Iter<'a, A, N>
where
    N: ChunkLength<A>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some(unsafe { &*self.buffer.ptr(self.right_index.dec()) })
        }
    }
}

impl<'a, A, N> ExactSizeIterator for Iter<'a, A, N> where N: ChunkLength<A> {}

impl<'a, A, N> FusedIterator for Iter<'a, A, N> where N: ChunkLength<A> {}

/// A mutable reference iterator over a `RingBuffer`.
pub struct IterMut<'a, A, N>
where
    N: ChunkLength<A>,
{
    data: *mut A,
    left_index: RawIndex<N>,
    right_index: RawIndex<N>,
    remaining: usize,
    phantom: PhantomData<&'a ()>,
}

impl<'a, A, N> IterMut<'a, A, N>
where
    N: ChunkLength<A>,
    A: 'a,
{
    pub(crate) fn new(buffer: &mut RingBuffer<A, N>) -> Self {
        Self::new_slice(buffer, buffer.origin, buffer.len())
    }

    pub(crate) fn new_slice(
        buffer: &mut RingBuffer<A, N>,
        origin: RawIndex<N>,
        len: usize,
    ) -> Self {
        Self {
            left_index: origin,
            right_index: origin + len,
            remaining: len,
            phantom: PhantomData,
            data: buffer.data.as_mut_ptr().cast(),
        }
    }

    unsafe fn mut_ptr(&mut self, index: RawIndex<N>) -> *mut A {
        self.data.add(index.to_usize())
    }
}

impl<'a, A, N> Iterator for IterMut<'a, A, N>
where
    N: ChunkLength<A>,
    A: 'a,
{
    type Item = &'a mut A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            let index = self.left_index.inc();
            Some(unsafe { &mut *self.mut_ptr(index) })
        }
    }

    #[inline]
    #[must_use]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, A, N> DoubleEndedIterator for IterMut<'a, A, N>
where
    N: ChunkLength<A>,
    A: 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            let index = self.right_index.dec();
            Some(unsafe { &mut *self.mut_ptr(index) })
        }
    }
}

impl<'a, A, N> ExactSizeIterator for IterMut<'a, A, N>
where
    N: ChunkLength<A>,
    A: 'a,
{
}

impl<'a, A, N> FusedIterator for IterMut<'a, A, N>
where
    N: ChunkLength<A>,
    A: 'a,
{
}

/// A draining iterator over a `RingBuffer`.
pub struct Drain<'a, A, N: ChunkLength<A>> {
    pub(crate) buffer: &'a mut RingBuffer<A, N>,
}

impl<'a, A: 'a, N: ChunkLength<A> + 'a> Iterator for Drain<'a, A, N> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.buffer.pop_front()
    }

    #[inline]
    #[must_use]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.buffer.len(), Some(self.buffer.len()))
    }
}

impl<'a, A: 'a, N: ChunkLength<A> + 'a> DoubleEndedIterator for Drain<'a, A, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.buffer.pop_back()
    }
}

impl<'a, A: 'a, N: ChunkLength<A> + 'a> ExactSizeIterator for Drain<'a, A, N> {}

impl<'a, A: 'a, N: ChunkLength<A> + 'a> FusedIterator for Drain<'a, A, N> {}

/// A consuming iterator over a `RingBuffer`.
pub struct OwnedIter<A, N: ChunkLength<A>> {
    pub(crate) buffer: RingBuffer<A, N>,
}

impl<A, N: ChunkLength<A>> Iterator for OwnedIter<A, N> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.buffer.pop_front()
    }

    #[inline]
    #[must_use]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.buffer.len(), Some(self.buffer.len()))
    }
}

impl<A, N: ChunkLength<A>> DoubleEndedIterator for OwnedIter<A, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.buffer.pop_back()
    }
}

impl<A, N: ChunkLength<A>> ExactSizeIterator for OwnedIter<A, N> {}

impl<A, N: ChunkLength<A>> FusedIterator for OwnedIter<A, N> {}
