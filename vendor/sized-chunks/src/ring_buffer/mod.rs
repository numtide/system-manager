// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A fixed capacity ring buffer.
//!
//! See [`RingBuffer`](struct.RingBuffer.html)

use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt::{Debug, Error, Formatter};
use core::hash::{Hash, Hasher};
use core::iter::FromIterator;
use core::mem::{replace, MaybeUninit};
use core::ops::{Bound, Range, RangeBounds};
use core::ops::{Index, IndexMut};

use typenum::U64;

pub use array_ops::{Array, ArrayMut, HasLength};

use crate::types::ChunkLength;

mod index;
use index::{IndexIter, RawIndex};

mod iter;
pub use iter::{Drain, Iter, IterMut, OwnedIter};

mod slice;
pub use slice::{Slice, SliceMut};

#[cfg(feature = "refpool")]
mod refpool;

/// A fixed capacity ring buffer.
///
/// A ring buffer is an array where the first logical index is at some arbitrary
/// location inside the array, and the indices wrap around to the start of the
/// array once they overflow its bounds.
///
/// This gives us the ability to push to either the front or the end of the
/// array in constant time, at the cost of losing the ability to get a single
/// contiguous slice reference to the contents.
///
/// It differs from the [`Chunk`][Chunk] in that the latter will have mostly
/// constant time pushes, but may occasionally need to shift its contents around
/// to make room. They both have constant time pop, and they both have linear
/// time insert and remove.
///
/// The `RingBuffer` offers its own [`Slice`][Slice] and [`SliceMut`][SliceMut]
/// types to compensate for the loss of being able to take a slice, but they're
/// somewhat less efficient, so the general rule should be that you shouldn't
/// choose a `RingBuffer` if you rely heavily on slices - but if you don't,
/// it's probably a marginally better choice overall than [`Chunk`][Chunk].
///
/// # Feature Flag
///
/// To use this data structure, you need to enable the `ringbuffer` feature.
///
/// [Chunk]: ../sized_chunk/struct.Chunk.html
/// [Slice]: struct.Slice.html
/// [SliceMut]: struct.SliceMut.html
pub struct RingBuffer<A, N = U64>
where
    N: ChunkLength<A>,
{
    origin: RawIndex<N>,
    length: usize,
    data: MaybeUninit<N::SizedType>,
}

impl<A, N: ChunkLength<A>> Drop for RingBuffer<A, N> {
    #[inline]
    fn drop(&mut self) {
        if core::mem::needs_drop::<A>() {
            for i in self.range() {
                unsafe { self.force_drop(i) }
            }
        }
    }
}

impl<A, N> HasLength for RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    /// Get the length of the ring buffer.
    #[inline]
    #[must_use]
    fn len(&self) -> usize {
        self.length
    }
}

impl<A, N> Array for RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    /// Get a reference to the value at a given index.
    #[must_use]
    fn get(&self, index: usize) -> Option<&A> {
        if index >= self.len() {
            None
        } else {
            Some(unsafe { self.get_unchecked(index) })
        }
    }
}

impl<A, N> ArrayMut for RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    /// Get a mutable reference to the value at a given index.
    #[must_use]
    fn get_mut(&mut self, index: usize) -> Option<&mut A> {
        if index >= self.len() {
            None
        } else {
            Some(unsafe { self.get_unchecked_mut(index) })
        }
    }
}

impl<A, N> RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    /// The capacity of this ring buffer, as a `usize`.
    pub const CAPACITY: usize = N::USIZE;

    /// Get the raw index for a logical index.
    #[inline]
    fn raw(&self, index: usize) -> RawIndex<N> {
        self.origin + index
    }

    #[inline]
    unsafe fn ptr(&self, index: RawIndex<N>) -> *const A {
        debug_assert!(index.to_usize() < Self::CAPACITY);
        (&self.data as *const _ as *const A).add(index.to_usize())
    }

    #[inline]
    unsafe fn mut_ptr(&mut self, index: RawIndex<N>) -> *mut A {
        debug_assert!(index.to_usize() < Self::CAPACITY);
        (&mut self.data as *mut _ as *mut A).add(index.to_usize())
    }

    /// Drop the value at a raw index.
    #[inline]
    unsafe fn force_drop(&mut self, index: RawIndex<N>) {
        core::ptr::drop_in_place(self.mut_ptr(index))
    }

    /// Copy the value at a raw index, discarding ownership of the copied value
    #[inline]
    unsafe fn force_read(&self, index: RawIndex<N>) -> A {
        core::ptr::read(self.ptr(index))
    }

    /// Write a value at a raw index without trying to drop what's already there
    #[inline]
    unsafe fn force_write(&mut self, index: RawIndex<N>, value: A) {
        core::ptr::write(self.mut_ptr(index), value)
    }

    /// Copy a range of raw indices from another buffer.
    unsafe fn copy_from(
        &mut self,
        source: &mut Self,
        from: RawIndex<N>,
        to: RawIndex<N>,
        count: usize,
    ) {
        #[inline]
        unsafe fn force_copy_to<A, N: ChunkLength<A>>(
            source: &mut RingBuffer<A, N>,
            from: RawIndex<N>,
            target: &mut RingBuffer<A, N>,
            to: RawIndex<N>,
            count: usize,
        ) {
            if count > 0 {
                debug_assert!(from.to_usize() + count <= RingBuffer::<A, N>::CAPACITY);
                debug_assert!(to.to_usize() + count <= RingBuffer::<A, N>::CAPACITY);
                core::ptr::copy_nonoverlapping(source.mut_ptr(from), target.mut_ptr(to), count)
            }
        }

        if from.to_usize() + count > Self::CAPACITY {
            let first_length = Self::CAPACITY - from.to_usize();
            let last_length = count - first_length;
            self.copy_from(source, from, to, first_length);
            self.copy_from(source, 0.into(), to + first_length, last_length);
        } else if to.to_usize() + count > Self::CAPACITY {
            let first_length = Self::CAPACITY - to.to_usize();
            let last_length = count - first_length;
            force_copy_to(source, from, self, to, first_length);
            force_copy_to(source, from + first_length, self, 0.into(), last_length);
        } else {
            force_copy_to(source, from, self, to, count);
        }
    }

    /// Copy values from a slice.
    #[allow(dead_code)]
    unsafe fn copy_from_slice(&mut self, source: &[A], to: RawIndex<N>) {
        let count = source.len();
        debug_assert!(to.to_usize() + count <= Self::CAPACITY);
        if to.to_usize() + count > Self::CAPACITY {
            let first_length = Self::CAPACITY - to.to_usize();
            let first_slice = &source[..first_length];
            let last_slice = &source[first_length..];
            core::ptr::copy_nonoverlapping(
                first_slice.as_ptr(),
                self.mut_ptr(to),
                first_slice.len(),
            );
            core::ptr::copy_nonoverlapping(
                last_slice.as_ptr(),
                self.mut_ptr(0.into()),
                last_slice.len(),
            );
        } else {
            core::ptr::copy_nonoverlapping(source.as_ptr(), self.mut_ptr(to), count)
        }
    }

    /// Get an iterator over the raw indices of the buffer from left to right.
    #[inline]
    fn range(&self) -> IndexIter<N> {
        IndexIter {
            remaining: self.len(),
            left_index: self.origin,
            right_index: self.origin + self.len(),
        }
    }

    /// Construct an empty ring buffer.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: 0.into(),
            length: 0,
            data: MaybeUninit::uninit(),
        }
    }

    /// Construct a ring buffer with a single item.
    #[inline]
    #[must_use]
    pub fn unit(value: A) -> Self {
        assert!(Self::CAPACITY >= 1);
        let mut buffer = Self {
            origin: 0.into(),
            length: 1,
            data: MaybeUninit::uninit(),
        };
        unsafe {
            buffer.force_write(0.into(), value);
        }
        buffer
    }

    /// Construct a ring buffer with two items.
    #[inline]
    #[must_use]
    pub fn pair(value1: A, value2: A) -> Self {
        assert!(Self::CAPACITY >= 2);
        let mut buffer = Self {
            origin: 0.into(),
            length: 2,
            data: MaybeUninit::uninit(),
        };
        unsafe {
            buffer.force_write(0.into(), value1);
            buffer.force_write(1.into(), value2);
        }
        buffer
    }

    /// Construct a new ring buffer and move every item from `other` into the
    /// new buffer.
    ///
    /// Time: O(n)
    #[inline]
    #[must_use]
    pub fn drain_from(other: &mut Self) -> Self {
        Self::from_front(other, other.len())
    }

    /// Construct a new ring buffer and populate it by taking `count` items from
    /// the iterator `iter`.
    ///
    /// Panics if the iterator contains less than `count` items.
    ///
    /// Time: O(n)
    #[must_use]
    pub fn collect_from<I>(iter: &mut I, count: usize) -> Self
    where
        I: Iterator<Item = A>,
    {
        let buffer = Self::from_iter(iter.take(count));
        if buffer.len() < count {
            panic!("RingBuffer::collect_from: underfull iterator");
        }
        buffer
    }

    /// Construct a new ring buffer and populate it by taking `count` items from
    /// the front of `other`.
    ///
    /// Time: O(n) for the number of items moved
    #[must_use]
    pub fn from_front(other: &mut Self, count: usize) -> Self {
        let mut buffer = Self::new();
        buffer.drain_from_front(other, count);
        buffer
    }

    /// Construct a new ring buffer and populate it by taking `count` items from
    /// the back of `other`.
    ///
    /// Time: O(n) for the number of items moved
    #[must_use]
    pub fn from_back(other: &mut Self, count: usize) -> Self {
        let mut buffer = Self::new();
        buffer.drain_from_back(other, count);
        buffer
    }

    /// Test if the ring buffer is full.
    #[inline]
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() == Self::CAPACITY
    }

    /// Get an iterator over references to the items in the ring buffer in
    /// order.
    #[inline]
    #[must_use]
    pub fn iter(&self) -> Iter<'_, A, N> {
        Iter {
            buffer: self,
            left_index: self.origin,
            right_index: self.origin + self.len(),
            remaining: self.len(),
        }
    }

    /// Get an iterator over mutable references to the items in the ring buffer
    /// in order.
    #[inline]
    #[must_use]
    pub fn iter_mut(&mut self) -> IterMut<'_, A, N> {
        IterMut::new(self)
    }

    #[must_use]
    fn parse_range<R: RangeBounds<usize>>(&self, range: R) -> Range<usize> {
        let new_range = Range {
            start: match range.start_bound() {
                Bound::Unbounded => 0,
                Bound::Included(index) => *index,
                Bound::Excluded(_) => unimplemented!(),
            },
            end: match range.end_bound() {
                Bound::Unbounded => self.len(),
                Bound::Included(index) => *index + 1,
                Bound::Excluded(index) => *index,
            },
        };
        if new_range.end > self.len() || new_range.start > new_range.end {
            panic!("Slice::parse_range: index out of bounds");
        }
        new_range
    }

    /// Get a `Slice` for a subset of the ring buffer.
    #[must_use]
    pub fn slice<R: RangeBounds<usize>>(&self, range: R) -> Slice<'_, A, N> {
        Slice {
            buffer: self,
            range: self.parse_range(range),
        }
    }

    /// Get a `SliceMut` for a subset of the ring buffer.
    #[must_use]
    pub fn slice_mut<R: RangeBounds<usize>>(&mut self, range: R) -> SliceMut<'_, A, N> {
        SliceMut {
            range: self.parse_range(range),
            buffer: self,
        }
    }

    /// Get an unchecked reference to the value at the given index.
    ///
    /// # Safety
    ///
    /// You must ensure the index is not out of bounds.
    #[must_use]
    pub unsafe fn get_unchecked(&self, index: usize) -> &A {
        &*self.ptr(self.raw(index))
    }

    /// Get an unchecked mutable reference to the value at the given index.
    ///
    /// # Safety
    ///
    /// You must ensure the index is not out of bounds.
    #[must_use]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut A {
        &mut *self.mut_ptr(self.raw(index))
    }

    /// Push a value to the back of the buffer.
    ///
    /// Panics if the capacity of the buffer is exceeded.
    ///
    /// Time: O(1)
    pub fn push_back(&mut self, value: A) {
        if self.is_full() {
            panic!("RingBuffer::push_back: can't push to a full buffer")
        } else {
            unsafe { self.force_write(self.raw(self.length), value) }
            self.length += 1;
        }
    }

    /// Push a value to the front of the buffer.
    ///
    /// Panics if the capacity of the buffer is exceeded.
    ///
    /// Time: O(1)
    pub fn push_front(&mut self, value: A) {
        if self.is_full() {
            panic!("RingBuffer::push_front: can't push to a full buffer")
        } else {
            let origin = self.origin.dec();
            self.length += 1;
            unsafe { self.force_write(origin, value) }
        }
    }

    /// Pop a value from the back of the buffer.
    ///
    /// Returns `None` if the buffer is empty.
    ///
    /// Time: O(1)
    pub fn pop_back(&mut self) -> Option<A> {
        if self.is_empty() {
            None
        } else {
            self.length -= 1;
            Some(unsafe { self.force_read(self.raw(self.length)) })
        }
    }

    /// Pop a value from the front of the buffer.
    ///
    /// Returns `None` if the buffer is empty.
    ///
    /// Time: O(1)
    pub fn pop_front(&mut self) -> Option<A> {
        if self.is_empty() {
            None
        } else {
            self.length -= 1;
            let index = self.origin.inc();
            Some(unsafe { self.force_read(index) })
        }
    }

    /// Discard all items up to but not including `index`.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items dropped
    pub fn drop_left(&mut self, index: usize) {
        if index > 0 {
            if index > self.len() {
                panic!("RingBuffer::drop_left: index out of bounds");
            }
            for i in self.range().take(index) {
                unsafe { self.force_drop(i) }
            }
            self.origin += index;
            self.length -= index;
        }
    }

    /// Discard all items from `index` onward.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items dropped
    pub fn drop_right(&mut self, index: usize) {
        if index > self.len() {
            panic!("RingBuffer::drop_right: index out of bounds");
        }
        if index == self.len() {
            return;
        }
        for i in self.range().skip(index) {
            unsafe { self.force_drop(i) }
        }
        self.length = index;
    }

    /// Split a buffer into two, the original buffer containing
    /// everything up to `index` and the returned buffer containing
    /// everything from `index` onwards.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items in the new buffer
    #[must_use]
    pub fn split_off(&mut self, index: usize) -> Self {
        if index > self.len() {
            panic!("RingBuffer::split: index out of bounds");
        }
        if index == self.len() {
            return Self::new();
        }
        let mut right = Self::new();
        let length = self.length - index;
        unsafe { right.copy_from(self, self.raw(index), 0.into(), length) };
        self.length = index;
        right.length = length;
        right
    }

    /// Remove all items from `other` and append them to the back of `self`.
    ///
    /// Panics if the capacity of `self` is exceeded.
    ///
    /// `other` will be an empty buffer after this operation.
    ///
    /// Time: O(n) for the number of items moved
    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.drain_from_front(other, other.len());
    }

    /// Remove `count` items from the front of `other` and append them to the
    /// back of `self`.
    ///
    /// Panics if `self` doesn't have `count` items left, or if `other` has
    /// fewer than `count` items.
    ///
    /// Time: O(n) for the number of items moved
    pub fn drain_from_front(&mut self, other: &mut Self, count: usize) {
        let self_len = self.len();
        let other_len = other.len();
        if self_len + count > Self::CAPACITY {
            panic!("RingBuffer::drain_from_front: chunk size overflow");
        }
        if other_len < count {
            panic!("RingBuffer::drain_from_front: index out of bounds");
        }
        unsafe { self.copy_from(other, other.origin, self.raw(self.len()), count) };
        other.origin += count;
        other.length -= count;
        self.length += count;
    }

    /// Remove `count` items from the back of `other` and append them to the
    /// front of `self`.
    ///
    /// Panics if `self` doesn't have `count` items left, or if `other` has
    /// fewer than `count` items.
    ///
    /// Time: O(n) for the number of items moved
    pub fn drain_from_back(&mut self, other: &mut Self, count: usize) {
        let self_len = self.len();
        let other_len = other.len();
        if self_len + count > Self::CAPACITY {
            panic!("RingBuffer::drain_from_back: chunk size overflow");
        }
        if other_len < count {
            panic!("RingBuffer::drain_from_back: index out of bounds");
        }
        self.origin -= count;
        let source_index = other.origin + (other.len() - count);
        unsafe { self.copy_from(other, source_index, self.origin, count) };
        other.length -= count;
        self.length += count;
    }

    /// Insert a new value at index `index`, shifting all the following values
    /// to the right.
    ///
    /// Panics if the index is out of bounds.
    ///
    /// Time: O(n) for the number of items shifted
    pub fn insert(&mut self, index: usize, value: A) {
        if self.is_full() {
            panic!("RingBuffer::insert: chunk size overflow");
        }
        if index > self.len() {
            panic!("RingBuffer::insert: index out of bounds");
        }
        if index == 0 {
            return self.push_front(value);
        }
        if index == self.len() {
            return self.push_back(value);
        }
        let right_count = self.len() - index;
        // Check which side has fewer elements to shift.
        if right_count < index {
            // Shift to the right.
            let mut i = self.raw(self.len() - 1);
            let target = self.raw(index);
            while i != target {
                unsafe { self.force_write(i + 1, self.force_read(i)) };
                i -= 1;
            }
            unsafe { self.force_write(target + 1, self.force_read(target)) };
            self.length += 1;
        } else {
            // Shift to the left.
            self.origin -= 1;
            self.length += 1;
            for i in self.range().take(index) {
                unsafe { self.force_write(i, self.force_read(i + 1)) };
            }
        }
        unsafe { self.force_write(self.raw(index), value) };
    }

    /// Insert a new value into the buffer in sorted order.
    ///
    /// This assumes every element of the buffer is already in sorted order.
    /// If not, the value will still be inserted but the ordering is not
    /// guaranteed.
    ///
    /// Time: O(log n) to find the insert position, then O(n) for the number
    /// of elements shifted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::iter::FromIterator;
    /// # use sized_chunks::Chunk;
    /// # use typenum::U64;
    /// let mut chunk = Chunk::<i32, U64>::from_iter(0..5);
    /// chunk.insert_ordered(3);
    /// assert_eq!(&[0, 1, 2, 3, 3, 4], chunk.as_slice());
    /// ```
    pub fn insert_ordered(&mut self, value: A)
    where
        A: Ord,
    {
        if self.is_full() {
            panic!("Chunk::insert: chunk is full");
        }
        match self.slice(..).binary_search(&value) {
            Ok(index) => self.insert(index, value),
            Err(index) => self.insert(index, value),
        }
    }

    /// Insert multiple values at index `index`, shifting all the following values
    /// to the right.
    ///
    /// Panics if the index is out of bounds or the chunk doesn't have room for
    /// all the values.
    ///
    /// Time: O(m+n) where m is the number of elements inserted and n is the number
    /// of elements following the insertion index. Calling `insert`
    /// repeatedly would be O(m*n).
    pub fn insert_from<Iterable, I>(&mut self, index: usize, iter: Iterable)
    where
        Iterable: IntoIterator<Item = A, IntoIter = I>,
        I: ExactSizeIterator<Item = A>,
    {
        let iter = iter.into_iter();
        let insert_size = iter.len();
        if self.len() + insert_size > Self::CAPACITY {
            panic!(
                "Chunk::insert_from: chunk cannot fit {} elements",
                insert_size
            );
        }
        if index > self.len() {
            panic!("Chunk::insert_from: index out of bounds");
        }
        if index == self.len() {
            self.extend(iter);
            return;
        }
        let right_count = self.len() - index;
        // Check which side has fewer elements to shift.
        if right_count < index {
            // Shift to the right.
            let mut i = self.raw(self.len() - 1);
            let target = self.raw(index);
            while i != target {
                unsafe { self.force_write(i + insert_size, self.force_read(i)) };
                i -= 1;
            }
            unsafe { self.force_write(target + insert_size, self.force_read(target)) };
            self.length += insert_size;
        } else {
            // Shift to the left.
            self.origin -= insert_size;
            self.length += insert_size;
            for i in self.range().take(index) {
                unsafe { self.force_write(i, self.force_read(i + insert_size)) };
            }
        }
        let mut index = self.raw(index);
        // Panic safety: unless and until we fill it fully, there's a hole somewhere in the middle
        // and the destructor would drop non-existing elements. Therefore we pretend to be empty
        // for a while (and leak the elements instead in case something bad happens).
        let mut inserted = 0;
        let length = replace(&mut self.length, 0);
        for value in iter.take(insert_size) {
            unsafe { self.force_write(index, value) };
            index += 1;
            inserted += 1;
        }
        // This would/could create a hole in the middle if it was less
        assert_eq!(
            inserted, insert_size,
            "Iterator has fewer elements than advertised",
        );
        self.length = length;
    }

    /// Remove the value at index `index`, shifting all the following values to
    /// the left.
    ///
    /// Returns the removed value.
    ///
    /// Panics if the index is out of bounds.
    ///
    /// Time: O(n) for the number of items shifted
    pub fn remove(&mut self, index: usize) -> A {
        if index >= self.len() {
            panic!("RingBuffer::remove: index out of bounds");
        }
        let value = unsafe { self.force_read(self.raw(index)) };
        let right_count = self.len() - index;
        // Check which side has fewer elements to shift.
        if right_count < index {
            // Shift from the right.
            self.length -= 1;
            let mut i = self.raw(index);
            let target = self.raw(self.len());
            while i != target {
                unsafe { self.force_write(i, self.force_read(i + 1)) };
                i += 1;
            }
        } else {
            // Shift from the left.
            let mut i = self.raw(index);
            while i != self.origin {
                unsafe { self.force_write(i, self.force_read(i - 1)) };
                i -= 1;
            }
            self.origin += 1;
            self.length -= 1;
        }
        value
    }

    /// Construct an iterator that drains values from the front of the buffer.
    pub fn drain(&mut self) -> Drain<'_, A, N> {
        Drain { buffer: self }
    }

    /// Discard the contents of the buffer.
    ///
    /// Time: O(n)
    pub fn clear(&mut self) {
        for i in self.range() {
            unsafe { self.force_drop(i) };
        }
        self.origin = 0.into();
        self.length = 0;
    }
}

impl<A, N: ChunkLength<A>> Default for RingBuffer<A, N> {
    #[inline]
    #[must_use]
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Clone, N: ChunkLength<A>> Clone for RingBuffer<A, N> {
    fn clone(&self) -> Self {
        let mut out = Self::new();
        out.origin = self.origin;
        out.length = self.length;
        let range = self.range();
        // Panic safety. If we panic, we don't want to drop more than we have initialized.
        out.length = 0;
        for index in range {
            unsafe { out.force_write(index, (&*self.ptr(index)).clone()) };
            out.length += 1;
        }
        out
    }
}

impl<A, N> Index<usize> for RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    type Output = A;

    #[must_use]
    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len() {
            panic!(
                "RingBuffer::index: index out of bounds {} >= {}",
                index,
                self.len()
            );
        }
        unsafe { &*self.ptr(self.raw(index)) }
    }
}

impl<A, N> IndexMut<usize> for RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    #[must_use]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len() {
            panic!(
                "RingBuffer::index_mut: index out of bounds {} >= {}",
                index,
                self.len()
            );
        }
        unsafe { &mut *self.mut_ptr(self.raw(index)) }
    }
}

impl<A: PartialEq, N: ChunkLength<A>> PartialEq for RingBuffer<A, N> {
    #[inline]
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<A, N, PrimSlice> PartialEq<PrimSlice> for RingBuffer<A, N>
where
    PrimSlice: Borrow<[A]>,
    A: PartialEq,
    N: ChunkLength<A>,
{
    #[inline]
    #[must_use]
    fn eq(&self, other: &PrimSlice) -> bool {
        let other = other.borrow();
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<A, N> PartialEq<Slice<'_, A, N>> for RingBuffer<A, N>
where
    A: PartialEq,
    N: ChunkLength<A>,
{
    fn eq(&self, other: &Slice<'_, A, N>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<A, N> PartialEq<SliceMut<'_, A, N>> for RingBuffer<A, N>
where
    A: PartialEq,
    N: ChunkLength<A>,
{
    fn eq(&self, other: &SliceMut<'_, A, N>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<A: Eq, N: ChunkLength<A>> Eq for RingBuffer<A, N> {}

impl<A: PartialOrd, N: ChunkLength<A>> PartialOrd for RingBuffer<A, N> {
    #[inline]
    #[must_use]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<A: Ord, N: ChunkLength<A>> Ord for RingBuffer<A, N> {
    #[inline]
    #[must_use]
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<A, N: ChunkLength<A>> Extend<A> for RingBuffer<A, N> {
    #[inline]
    fn extend<I: IntoIterator<Item = A>>(&mut self, iter: I) {
        for item in iter {
            self.push_back(item);
        }
    }
}

impl<'a, A: Clone + 'a, N: ChunkLength<A>> Extend<&'a A> for RingBuffer<A, N> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a A>>(&mut self, iter: I) {
        for item in iter {
            self.push_back(item.clone());
        }
    }
}

impl<A: Debug, N: ChunkLength<A>> Debug for RingBuffer<A, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("RingBuffer")?;
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<A: Hash, N: ChunkLength<A>> Hash for RingBuffer<A, N> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        for item in self {
            item.hash(hasher)
        }
    }
}

#[cfg(feature = "std")]
impl<N: ChunkLength<u8>> std::io::Write for RingBuffer<u8, N> {
    fn write(&mut self, mut buf: &[u8]) -> std::io::Result<usize> {
        let max_new = Self::CAPACITY - self.len();
        if buf.len() > max_new {
            buf = &buf[..max_new];
        }
        unsafe { self.copy_from_slice(buf, self.origin + self.len()) };
        self.length += buf.len();
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<N: ChunkLength<u8>> std::io::Read for RingBuffer<u8, N> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_size = buf.len().min(self.len());
        if read_size == 0 {
            Ok(0)
        } else {
            for p in buf.iter_mut().take(read_size) {
                *p = self.pop_front().unwrap();
            }
            Ok(read_size)
        }
    }
}

impl<A, N: ChunkLength<A>> FromIterator<A> for RingBuffer<A, N> {
    #[must_use]
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> Self {
        let mut buffer = RingBuffer::new();
        buffer.extend(iter);
        buffer
    }
}

impl<A, N: ChunkLength<A>> IntoIterator for RingBuffer<A, N> {
    type Item = A;
    type IntoIter = OwnedIter<A, N>;

    #[inline]
    #[must_use]
    fn into_iter(self) -> Self::IntoIter {
        OwnedIter { buffer: self }
    }
}

impl<'a, A, N: ChunkLength<A>> IntoIterator for &'a RingBuffer<A, N> {
    type Item = &'a A;
    type IntoIter = Iter<'a, A, N>;

    #[inline]
    #[must_use]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A, N: ChunkLength<A>> IntoIterator for &'a mut RingBuffer<A, N> {
    type Item = &'a mut A;
    type IntoIter = IterMut<'a, A, N>;

    #[inline]
    #[must_use]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// Tests

#[cfg(test)]
mod test {
    use typenum::U0;

    use super::*;

    #[test]
    fn validity_invariant() {
        assert!(Some(RingBuffer::<Box<()>>::new()).is_some());
    }

    #[test]
    fn is_full() {
        let mut chunk = RingBuffer::<_, U64>::new();
        for i in 0..64 {
            assert_eq!(false, chunk.is_full());
            chunk.push_back(i);
        }
        assert_eq!(true, chunk.is_full());
    }

    #[test]
    fn ref_iter() {
        let chunk: RingBuffer<i32> = (0..64).collect();
        let out_vec: Vec<&i32> = chunk.iter().collect();
        let should_vec_p: Vec<i32> = (0..64).collect();
        let should_vec: Vec<&i32> = should_vec_p.iter().collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn mut_ref_iter() {
        let mut chunk: RingBuffer<i32> = (0..64).collect();
        let out_vec: Vec<&mut i32> = chunk.iter_mut().collect();
        let mut should_vec_p: Vec<i32> = (0..64).collect();
        let should_vec: Vec<&mut i32> = should_vec_p.iter_mut().collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn consuming_iter() {
        let chunk: RingBuffer<i32> = (0..64).collect();
        let out_vec: Vec<i32> = chunk.into_iter().collect();
        let should_vec: Vec<i32> = (0..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn draining_iter() {
        let mut chunk: RingBuffer<i32> = (0..64).collect();
        let mut half: RingBuffer<i32> = chunk.drain().take(16).collect();
        half.extend(chunk.drain().rev().take(16));
        let should: Vec<i32> = (16..48).collect();
        assert_eq!(chunk, should);
        let should: Vec<i32> = (0..16).chain((48..64).rev()).collect();
        assert_eq!(half, should);
    }

    #[cfg(feature = "std")]
    #[test]
    fn io_write() {
        use std::io::Write;
        let mut buffer: RingBuffer<u8> = (0..32).collect();
        let to_write: Vec<u8> = (32..128).collect();
        assert_eq!(32, buffer.write(&to_write).unwrap());
        assert_eq!(buffer, (0..64).collect::<Vec<u8>>());
    }

    #[cfg(feature = "std")]
    #[test]
    fn io_read() {
        use std::io::Read;
        let mut buffer: RingBuffer<u8> = (16..48).collect();
        let mut read_buf: Vec<u8> = (0..16).collect();
        assert_eq!(16, buffer.read(&mut read_buf).unwrap());
        assert_eq!(read_buf, (16..32).collect::<Vec<u8>>());
        assert_eq!(buffer, (32..48).collect::<Vec<u8>>());
        assert_eq!(16, buffer.read(&mut read_buf).unwrap());
        assert_eq!(read_buf, (32..48).collect::<Vec<u8>>());
        assert_eq!(buffer, vec![]);
        assert_eq!(0, buffer.read(&mut read_buf).unwrap());
    }

    #[test]
    fn clone() {
        let buffer: RingBuffer<u32> = (0..50).collect();
        assert_eq!(buffer, buffer.clone());
    }

    #[test]
    fn failing() {
        let mut buffer: RingBuffer<u32> = RingBuffer::new();
        buffer.push_front(0);
        let mut add: RingBuffer<u32> = vec![1, 0, 0, 0, 0, 0].into_iter().collect();
        buffer.append(&mut add);
        assert_eq!(1, buffer.remove(1));
        let expected = vec![0, 0, 0, 0, 0, 0];
        assert_eq!(buffer, expected);
    }

    use crate::tests::DropTest;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn dropping() {
        let counter = AtomicUsize::new(0);
        {
            let mut chunk: RingBuffer<DropTest<'_>> = RingBuffer::new();
            for _i in 0..20 {
                chunk.push_back(DropTest::new(&counter))
            }
            for _i in 0..20 {
                chunk.push_front(DropTest::new(&counter))
            }
            assert_eq!(40, counter.load(Ordering::Relaxed));
            for _i in 0..10 {
                chunk.pop_back();
            }
            assert_eq!(30, counter.load(Ordering::Relaxed));
        }
        assert_eq!(0, counter.load(Ordering::Relaxed));
    }

    #[test]
    #[should_panic(expected = "assertion failed: Self::CAPACITY >= 1")]
    fn unit_on_empty() {
        let _ = RingBuffer::<usize, U0>::unit(1);
    }

    #[test]
    #[should_panic(expected = "assertion failed: Self::CAPACITY >= 2")]
    fn pair_on_empty() {
        let _ = RingBuffer::<usize, U0>::pair(1, 2);
    }
}
