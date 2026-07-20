// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A fixed capacity smart array.
//!
//! See [`Chunk`](struct.Chunk.html)

use crate::inline_array::InlineArray;
use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::{Debug, Error, Formatter};
use core::hash::{Hash, Hasher};
use core::iter::FromIterator;
use core::mem::{replace, MaybeUninit};
use core::ops::{Deref, DerefMut, Index, IndexMut};
use core::ptr;
use core::slice::{
    from_raw_parts, from_raw_parts_mut, Iter as SliceIter, IterMut as SliceIterMut, SliceIndex,
};

#[cfg(feature = "std")]
use std::io;

use typenum::U64;

use crate::types::ChunkLength;

mod iter;
pub use self::iter::{Drain, Iter};

#[cfg(feature = "refpool")]
mod refpool;

/// A fixed capacity smart array.
///
/// An inline array of items with a variable length but a fixed, preallocated
/// capacity given by the `N` type, which must be an [`Unsigned`][Unsigned] type
/// level numeral.
///
/// It's 'smart' because it's able to reorganise its contents based on expected
/// behaviour. If you construct one using `push_back`, it will be laid out like
/// a `Vec` with space at the end. If you `push_front` it will start filling in
/// values from the back instead of the front, so that you still get linear time
/// push as long as you don't reverse direction. If you do, and there's no room
/// at the end you're pushing to, it'll shift its contents over to the other
/// side, creating more space to push into. This technique is tuned for
/// `Chunk`'s expected use case in [im::Vector]: usually, chunks always see
/// either `push_front` or `push_back`, but not both unless they move around
/// inside the tree, in which case they're able to reorganise themselves with
/// reasonable efficiency to suit their new usage patterns.
///
/// It maintains a `left` index and a `right` index instead of a simple length
/// counter in order to accomplish this, much like a ring buffer would, except
/// that the `Chunk` keeps all its items sequentially in memory so that you can
/// always get a `&[A]` slice for them, at the price of the occasional
/// reordering operation. The allocated size of a `Chunk` is thus `usize` * 2 +
/// `A` * `N`.
///
/// This technique also lets us choose to shift the shortest side to account for
/// the inserted or removed element when performing insert and remove
/// operations, unlike `Vec` where you always need to shift the right hand side.
///
/// Unlike a `Vec`, the `Chunk` has a fixed capacity and cannot grow beyond it.
/// Being intended for low level use, it expects you to know or test whether
/// you're pushing to a full array, and has an API more geared towards panics
/// than returning `Option`s, on the assumption that you know what you're doing.
/// Of course, if you don't, you can expect it to panic immediately rather than
/// do something undefined and usually bad.
///
/// ## Isn't this just a less efficient ring buffer?
///
/// You might be wondering why you would want to use this data structure rather
/// than a [`RingBuffer`][RingBuffer], which is similar but doesn't need to
/// shift its content around when it hits the sides of the allocated buffer. The
/// answer is that `Chunk` can be dereferenced into a slice, while a ring buffer
/// can not. You'll also save a few cycles on index lookups, as a `Chunk`'s data
/// is guaranteed to be contiguous in memory, so there's no need to remap logical
/// indices to a ring buffer's physical layout.
///
/// # Examples
///
/// ```rust
/// # #[macro_use] extern crate sized_chunks;
/// # extern crate typenum;
/// # use sized_chunks::Chunk;
/// # use typenum::U64;
/// // Construct a chunk with a 64 item capacity
/// let mut chunk = Chunk::<i32, U64>::new();
/// // Fill it with descending numbers
/// chunk.extend((0..64).rev());
/// // It derefs to a slice so we can use standard slice methods
/// chunk.sort();
/// // It's got all the amenities like `FromIterator` and `Eq`
/// let expected: Chunk<i32, U64> = (0..64).collect();
/// assert_eq!(expected, chunk);
/// ```
///
/// [Unsigned]: https://docs.rs/typenum/1.10.0/typenum/marker_traits/trait.Unsigned.html
/// [im::Vector]: https://docs.rs/im/latest/im/vector/enum.Vector.html
/// [RingBuffer]: ../ring_buffer/struct.RingBuffer.html
pub struct Chunk<A, N = U64>
where
    N: ChunkLength<A>,
{
    left: usize,
    right: usize,
    data: MaybeUninit<N::SizedType>,
}

impl<A, N> Drop for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.as_mut_slice()) }
    }
}

impl<A, N> Clone for Chunk<A, N>
where
    A: Clone,
    N: ChunkLength<A>,
{
    fn clone(&self) -> Self {
        let mut out = Self::new();
        out.left = self.left;
        out.right = self.left;
        for index in self.left..self.right {
            unsafe { Chunk::force_write(index, (*self.ptr(index)).clone(), &mut out) }
            // Panic safety, move the right index to cover only the really initialized things. This
            // way we don't try to drop uninitialized, but also don't leak if we panic in the
            // middle.
            out.right = index + 1;
        }
        out
    }
}

impl<A, N> Chunk<A, N>
where
    N: ChunkLength<A>,
{
    /// The maximum number of elements this `Chunk` can contain.
    pub const CAPACITY: usize = N::USIZE;

    /// Construct a new empty chunk.
    pub fn new() -> Self {
        Self {
            left: 0,
            right: 0,
            data: MaybeUninit::uninit(),
        }
    }

    /// Construct a new chunk with one item.
    pub fn unit(value: A) -> Self {
        assert!(Self::CAPACITY >= 1);
        let mut chunk = Self {
            left: 0,
            right: 1,
            data: MaybeUninit::uninit(),
        };
        unsafe {
            Chunk::force_write(0, value, &mut chunk);
        }
        chunk
    }

    /// Construct a new chunk with two items.
    pub fn pair(left: A, right: A) -> Self {
        assert!(Self::CAPACITY >= 2);
        let mut chunk = Self {
            left: 0,
            right: 2,
            data: MaybeUninit::uninit(),
        };
        unsafe {
            Chunk::force_write(0, left, &mut chunk);
            Chunk::force_write(1, right, &mut chunk);
        }
        chunk
    }

    /// Construct a new chunk and move every item from `other` into the new
    /// chunk.
    ///
    /// Time: O(n)
    pub fn drain_from(other: &mut Self) -> Self {
        let other_len = other.len();
        Self::from_front(other, other_len)
    }

    /// Construct a new chunk and populate it by taking `count` items from the
    /// iterator `iter`.
    ///
    /// Panics if the iterator contains less than `count` items.
    ///
    /// Time: O(n)
    pub fn collect_from<I>(iter: &mut I, mut count: usize) -> Self
    where
        I: Iterator<Item = A>,
    {
        let mut chunk = Self::new();
        while count > 0 {
            count -= 1;
            chunk.push_back(
                iter.next()
                    .expect("Chunk::collect_from: underfull iterator"),
            );
        }
        chunk
    }

    /// Construct a new chunk and populate it by taking `count` items from the
    /// front of `other`.
    ///
    /// Time: O(n) for the number of items moved
    pub fn from_front(other: &mut Self, count: usize) -> Self {
        let other_len = other.len();
        debug_assert!(count <= other_len);
        let mut chunk = Self::new();
        unsafe { Chunk::force_copy_to(other.left, 0, count, other, &mut chunk) };
        chunk.right = count;
        other.left += count;
        chunk
    }

    /// Construct a new chunk and populate it by taking `count` items from the
    /// back of `other`.
    ///
    /// Time: O(n) for the number of items moved
    pub fn from_back(other: &mut Self, count: usize) -> Self {
        let other_len = other.len();
        debug_assert!(count <= other_len);
        let mut chunk = Self::new();
        unsafe { Chunk::force_copy_to(other.right - count, 0, count, other, &mut chunk) };
        chunk.right = count;
        other.right -= count;
        chunk
    }

    /// Get the length of the chunk.
    #[inline]
    pub fn len(&self) -> usize {
        self.right - self.left
    }

    /// Test if the chunk is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.left == self.right
    }

    /// Test if the chunk is at capacity.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.left == 0 && self.right == Self::CAPACITY
    }

    #[inline]
    unsafe fn ptr(&self, index: usize) -> *const A {
        (&self.data as *const _ as *const A).add(index)
    }

    /// It has no bounds checks
    #[inline]
    unsafe fn mut_ptr(&mut self, index: usize) -> *mut A {
        (&mut self.data as *mut _ as *mut A).add(index)
    }

    /// Copy the value at an index, discarding ownership of the copied value
    #[inline]
    unsafe fn force_read(index: usize, chunk: &mut Self) -> A {
        chunk.ptr(index).read()
    }

    /// Write a value at an index without trying to drop what's already there.
    /// It has no bounds checks.
    #[inline]
    unsafe fn force_write(index: usize, value: A, chunk: &mut Self) {
        chunk.mut_ptr(index).write(value)
    }

    /// Copy a range within a chunk
    #[inline]
    unsafe fn force_copy(from: usize, to: usize, count: usize, chunk: &mut Self) {
        if count > 0 {
            ptr::copy(chunk.ptr(from), chunk.mut_ptr(to), count)
        }
    }

    /// Write values from iterator into range starting at write_index.
    ///
    /// Will overwrite values at the relevant range without dropping even in case the values were
    /// already initialized (it is expected they are empty). Does not update the left or right
    /// index.
    ///
    /// # Safety
    ///
    /// Range checks must already have been performed.
    ///
    /// # Panics
    ///
    /// If the iterator panics, the chunk becomes conceptually empty and will leak any previous
    /// elements (even the ones outside the range).
    #[inline]
    unsafe fn write_from_iter<I>(mut write_index: usize, iter: I, chunk: &mut Self)
    where
        I: ExactSizeIterator<Item = A>,
    {
        // Panic safety. We make the array conceptually empty, so we never ever drop anything that
        // is unitialized. We do so because we expect to be called when there's a potential "hole"
        // in the array that makes the space for the new elements to be written. We return it back
        // to original when everything goes fine, but leak any elements on panic. This is bad, but
        // better than dropping non-existing stuff.
        //
        // Should we worry about some better panic recovery than this?
        let left = replace(&mut chunk.left, 0);
        let right = replace(&mut chunk.right, 0);
        let len = iter.len();
        let expected_end = write_index + len;
        for value in iter.take(len) {
            Chunk::force_write(write_index, value, chunk);
            write_index += 1;
        }
        // Oops, we have a hole in here now. That would be bad, give up.
        assert_eq!(
            expected_end, write_index,
            "ExactSizeIterator yielded fewer values than advertised",
        );
        chunk.left = left;
        chunk.right = right;
    }

    /// Copy a range between chunks
    #[inline]
    unsafe fn force_copy_to(
        from: usize,
        to: usize,
        count: usize,
        chunk: &mut Self,
        other: &mut Self,
    ) {
        if count > 0 {
            ptr::copy_nonoverlapping(chunk.ptr(from), other.mut_ptr(to), count)
        }
    }

    /// Push an item to the front of the chunk.
    ///
    /// Panics if the capacity of the chunk is exceeded.
    ///
    /// Time: O(1) if there's room at the front, O(n) otherwise
    pub fn push_front(&mut self, value: A) {
        if self.is_full() {
            panic!("Chunk::push_front: can't push to full chunk");
        }
        if self.is_empty() {
            self.left = N::USIZE;
            self.right = N::USIZE;
        } else if self.left == 0 {
            self.left = N::USIZE - self.right;
            unsafe { Chunk::force_copy(0, self.left, self.right, self) };
            self.right = N::USIZE;
        }
        self.left -= 1;
        unsafe { Chunk::force_write(self.left, value, self) }
    }

    /// Push an item to the back of the chunk.
    ///
    /// Panics if the capacity of the chunk is exceeded.
    ///
    /// Time: O(1) if there's room at the back, O(n) otherwise
    pub fn push_back(&mut self, value: A) {
        if self.is_full() {
            panic!("Chunk::push_back: can't push to full chunk");
        }
        if self.is_empty() {
            self.left = 0;
            self.right = 0;
        } else if self.right == N::USIZE {
            unsafe { Chunk::force_copy(self.left, 0, self.len(), self) };
            self.right = N::USIZE - self.left;
            self.left = 0;
        }
        unsafe { Chunk::force_write(self.right, value, self) }
        self.right += 1;
    }

    /// Pop an item off the front of the chunk.
    ///
    /// Panics if the chunk is empty.
    ///
    /// Time: O(1)
    pub fn pop_front(&mut self) -> A {
        if self.is_empty() {
            panic!("Chunk::pop_front: can't pop from empty chunk");
        } else {
            let value = unsafe { Chunk::force_read(self.left, self) };
            self.left += 1;
            value
        }
    }

    /// Pop an item off the back of the chunk.
    ///
    /// Panics if the chunk is empty.
    ///
    /// Time: O(1)
    pub fn pop_back(&mut self) -> A {
        if self.is_empty() {
            panic!("Chunk::pop_back: can't pop from empty chunk");
        } else {
            self.right -= 1;
            unsafe { Chunk::force_read(self.right, self) }
        }
    }

    /// Discard all items up to but not including `index`.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items dropped
    pub fn drop_left(&mut self, index: usize) {
        if index > 0 {
            unsafe { ptr::drop_in_place(&mut self[..index]) }
            self.left += index;
        }
    }

    /// Discard all items from `index` onward.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items dropped
    pub fn drop_right(&mut self, index: usize) {
        if index != self.len() {
            unsafe { ptr::drop_in_place(&mut self[index..]) }
            self.right = self.left + index;
        }
    }

    /// Split a chunk into two, the original chunk containing
    /// everything up to `index` and the returned chunk containing
    /// everything from `index` onwards.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items in the new chunk
    pub fn split_off(&mut self, index: usize) -> Self {
        if index > self.len() {
            panic!("Chunk::split_off: index out of bounds");
        }
        if index == self.len() {
            return Self::new();
        }
        let mut right_chunk = Self::new();
        let start = self.left + index;
        let len = self.right - start;
        unsafe { Chunk::force_copy_to(start, 0, len, self, &mut right_chunk) };
        right_chunk.right = len;
        self.right = start;
        right_chunk
    }

    /// Remove all items from `other` and append them to the back of `self`.
    ///
    /// Panics if the capacity of the chunk is exceeded.
    ///
    /// Time: O(n) for the number of items moved
    pub fn append(&mut self, other: &mut Self) {
        let self_len = self.len();
        let other_len = other.len();
        if self_len + other_len > N::USIZE {
            panic!("Chunk::append: chunk size overflow");
        }
        if self.right + other_len > N::USIZE {
            unsafe { Chunk::force_copy(self.left, 0, self_len, self) };
            self.right -= self.left;
            self.left = 0;
        }
        unsafe { Chunk::force_copy_to(other.left, self.right, other_len, other, self) };
        self.right += other_len;
        other.left = 0;
        other.right = 0;
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
        assert!(self_len + count <= N::USIZE);
        assert!(other_len >= count);
        if self.right + count > N::USIZE {
            unsafe { Chunk::force_copy(self.left, 0, self_len, self) };
            self.right -= self.left;
            self.left = 0;
        }
        unsafe { Chunk::force_copy_to(other.left, self.right, count, other, self) };
        self.right += count;
        other.left += count;
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
        assert!(self_len + count <= N::USIZE);
        assert!(other_len >= count);
        if self.left < count {
            unsafe { Chunk::force_copy(self.left, N::USIZE - self_len, self_len, self) };
            self.left = N::USIZE - self_len;
            self.right = N::USIZE;
        }
        unsafe { Chunk::force_copy_to(other.right - count, self.left - count, count, other, self) };
        self.left -= count;
        other.right -= count;
    }

    /// Update the value at index `index`, returning the old value.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(1)
    pub fn set(&mut self, index: usize, value: A) -> A {
        replace(&mut self[index], value)
    }

    /// Insert a new value at index `index`, shifting all the following values
    /// to the right.
    ///
    /// Panics if the index is out of bounds or the chunk is full.
    ///
    /// Time: O(n) for the number of elements shifted
    pub fn insert(&mut self, index: usize, value: A) {
        if self.is_full() {
            panic!("Chunk::insert: chunk is full");
        }
        if index > self.len() {
            panic!("Chunk::insert: index out of bounds");
        }
        let real_index = index + self.left;
        let left_size = index;
        let right_size = self.right - real_index;
        if self.right == N::USIZE || (self.left > 0 && left_size < right_size) {
            unsafe {
                Chunk::force_copy(self.left, self.left - 1, left_size, self);
                Chunk::force_write(real_index - 1, value, self);
            }
            self.left -= 1;
        } else {
            unsafe {
                Chunk::force_copy(real_index, real_index + 1, right_size, self);
                Chunk::force_write(real_index, value, self);
            }
            self.right += 1;
        }
    }

    /// Insert a new value into the chunk in sorted order.
    ///
    /// This assumes every element of the chunk is already in sorted order.
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
        match self.binary_search(&value) {
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
        let real_index = index + self.left;
        let left_size = index;
        let right_size = self.right - real_index;
        if self.right == N::USIZE || (self.left >= insert_size && left_size < right_size) {
            unsafe {
                Chunk::force_copy(self.left, self.left - insert_size, left_size, self);
                let write_index = real_index - insert_size;
                Chunk::write_from_iter(write_index, iter, self);
            }
            self.left -= insert_size;
        } else if self.left == 0 || (self.right + insert_size <= Self::CAPACITY) {
            unsafe {
                Chunk::force_copy(real_index, real_index + insert_size, right_size, self);
                let write_index = real_index;
                Chunk::write_from_iter(write_index, iter, self);
            }
            self.right += insert_size;
        } else {
            unsafe {
                Chunk::force_copy(self.left, 0, left_size, self);
                Chunk::force_copy(real_index, left_size + insert_size, right_size, self);
                let write_index = left_size;
                Chunk::write_from_iter(write_index, iter, self);
            }
            self.right -= self.left;
            self.right += insert_size;
            self.left = 0;
        }
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
            panic!("Chunk::remove: index out of bounds");
        }
        let real_index = index + self.left;
        let value = unsafe { Chunk::force_read(real_index, self) };
        let left_size = index;
        let right_size = self.right - real_index - 1;
        if left_size < right_size {
            unsafe { Chunk::force_copy(self.left, self.left + 1, left_size, self) };
            self.left += 1;
        } else {
            unsafe { Chunk::force_copy(real_index + 1, real_index, right_size, self) };
            self.right -= 1;
        }
        value
    }

    /// Construct an iterator that drains values from the front of the chunk.
    pub fn drain(&mut self) -> Drain<'_, A, N> {
        Drain { chunk: self }
    }

    /// Discard the contents of the chunk.
    ///
    /// Time: O(n)
    pub fn clear(&mut self) {
        unsafe { ptr::drop_in_place(self.as_mut_slice()) }
        self.left = 0;
        self.right = 0;
    }

    /// Get a reference to the contents of the chunk as a slice.
    pub fn as_slice(&self) -> &[A] {
        unsafe {
            from_raw_parts(
                (&self.data as *const MaybeUninit<N::SizedType> as *const A).add(self.left),
                self.len(),
            )
        }
    }

    /// Get a reference to the contents of the chunk as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [A] {
        unsafe {
            from_raw_parts_mut(
                (&mut self.data as *mut MaybeUninit<N::SizedType> as *mut A).add(self.left),
                self.len(),
            )
        }
    }
}

impl<A, N> Default for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, N, I> Index<I> for Chunk<A, N>
where
    I: SliceIndex<[A]>,
    N: ChunkLength<A>,
{
    type Output = I::Output;
    fn index(&self, index: I) -> &Self::Output {
        self.as_slice().index(index)
    }
}

impl<A, N, I> IndexMut<I> for Chunk<A, N>
where
    I: SliceIndex<[A]>,
    N: ChunkLength<A>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.as_mut_slice().index_mut(index)
    }
}

impl<A, N> Debug for Chunk<A, N>
where
    A: Debug,
    N: ChunkLength<A>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("Chunk")?;
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<A, N> Hash for Chunk<A, N>
where
    A: Hash,
    N: ChunkLength<A>,
{
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        for item in self {
            item.hash(hasher)
        }
    }
}

impl<A, N, Slice> PartialEq<Slice> for Chunk<A, N>
where
    Slice: Borrow<[A]>,
    A: PartialEq,
    N: ChunkLength<A>,
{
    fn eq(&self, other: &Slice) -> bool {
        self.as_slice() == other.borrow()
    }
}

impl<A, N> Eq for Chunk<A, N>
where
    A: Eq,
    N: ChunkLength<A>,
{
}

impl<A, N> PartialOrd for Chunk<A, N>
where
    A: PartialOrd,
    N: ChunkLength<A>,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<A, N> Ord for Chunk<A, N>
where
    A: Ord,
    N: ChunkLength<A>,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

#[cfg(feature = "std")]
impl<N> io::Write for Chunk<u8, N>
where
    N: ChunkLength<u8>,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let old_len = self.len();
        self.extend(buf.iter().cloned().take(N::USIZE - old_len));
        Ok(self.len() - old_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<N: ChunkLength<u8>> std::io::Read for Chunk<u8, N> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_size = buf.len().min(self.len());
        if read_size == 0 {
            Ok(0)
        } else {
            for p in buf.iter_mut().take(read_size) {
                *p = self.pop_front();
            }
            Ok(read_size)
        }
    }
}

impl<A, N, T> From<InlineArray<A, T>> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    #[inline]
    fn from(mut array: InlineArray<A, T>) -> Self {
        Self::from(&mut array)
    }
}

impl<'a, A, N, T> From<&'a mut InlineArray<A, T>> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn from(array: &mut InlineArray<A, T>) -> Self {
        // The first capacity comparison is to help optimize it out
        assert!(
            InlineArray::<A, T>::CAPACITY <= Self::CAPACITY || array.len() <= Self::CAPACITY,
            "CAPACITY too small"
        );
        let mut out = Self::new();
        out.left = 0;
        out.right = array.len();
        unsafe {
            ptr::copy_nonoverlapping(array.data(), out.mut_ptr(0), out.right);
            *array.len_mut() = 0;
        }
        out
    }
}

impl<A, N> Borrow<[A]> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn borrow(&self) -> &[A] {
        self.as_slice()
    }
}

impl<A, N> BorrowMut<[A]> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn borrow_mut(&mut self) -> &mut [A] {
        self.as_mut_slice()
    }
}

impl<A, N> AsRef<[A]> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn as_ref(&self) -> &[A] {
        self.as_slice()
    }
}

impl<A, N> AsMut<[A]> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn as_mut(&mut self) -> &mut [A] {
        self.as_mut_slice()
    }
}

impl<A, N> Deref for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    type Target = [A];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<A, N> DerefMut for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<A, N> FromIterator<A> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    fn from_iter<I>(it: I) -> Self
    where
        I: IntoIterator<Item = A>,
    {
        let mut chunk = Self::new();
        for item in it {
            chunk.push_back(item);
        }
        chunk
    }
}

impl<'a, A, N> IntoIterator for &'a Chunk<A, N>
where
    N: ChunkLength<A>,
{
    type Item = &'a A;
    type IntoIter = SliceIter<'a, A>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A, N> IntoIterator for &'a mut Chunk<A, N>
where
    N: ChunkLength<A>,
{
    type Item = &'a mut A;
    type IntoIter = SliceIterMut<'a, A>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<A, N> Extend<A> for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    /// Append the contents of the iterator to the back of the chunk.
    ///
    /// Panics if the chunk exceeds its capacity.
    ///
    /// Time: O(n) for the length of the iterator
    fn extend<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = A>,
    {
        for item in it {
            self.push_back(item);
        }
    }
}

impl<'a, A, N> Extend<&'a A> for Chunk<A, N>
where
    A: 'a + Copy,
    N: ChunkLength<A>,
{
    /// Append the contents of the iterator to the back of the chunk.
    ///
    /// Panics if the chunk exceeds its capacity.
    ///
    /// Time: O(n) for the length of the iterator
    fn extend<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = &'a A>,
    {
        for item in it {
            self.push_back(*item);
        }
    }
}

impl<A, N> IntoIterator for Chunk<A, N>
where
    N: ChunkLength<A>,
{
    type Item = A;
    type IntoIter = Iter<A, N>;

    fn into_iter(self) -> Self::IntoIter {
        Iter { chunk: self }
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod test {
    use super::*;
    use typenum::{U0, U1, U2, U3, U5};

    #[test]
    #[should_panic(expected = "Chunk::push_back: can't push to full chunk")]
    fn issue_11_testcase1d() {
        let mut chunk = Chunk::<usize, U2>::pair(123, 456);
        chunk.push_back(789);
    }

    #[test]
    #[should_panic(expected = "CAPACITY too small")]
    fn issue_11_testcase2a() {
        let mut from = InlineArray::<u8, [u8; 256]>::new();
        from.push(1);

        let _ = Chunk::<u8, U0>::from(from);
    }

    #[test]
    fn issue_11_testcase2b() {
        let mut from = InlineArray::<u8, [u8; 256]>::new();
        from.push(1);

        let _ = Chunk::<u8, U1>::from(from);
    }

    struct DropDetector(u32);

    impl DropDetector {
        fn new(num: u32) -> Self {
            DropDetector(num)
        }
    }

    impl Drop for DropDetector {
        fn drop(&mut self) {
            assert!(self.0 == 42 || self.0 == 43);
        }
    }

    impl Clone for DropDetector {
        fn clone(&self) -> Self {
            if self.0 == 42 {
                panic!("panic on clone")
            }
            DropDetector::new(self.0)
        }
    }

    /// This is for miri to catch
    #[test]
    fn issue_11_testcase3a() {
        let mut chunk = Chunk::<DropDetector, U3>::new();
        chunk.push_back(DropDetector::new(42));
        chunk.push_back(DropDetector::new(42));
        chunk.push_back(DropDetector::new(43));
        let _ = chunk.pop_front();

        let _ = std::panic::catch_unwind(|| {
            let _ = chunk.clone();
        });
    }

    struct PanickingIterator {
        current: u32,
        panic_at: u32,
        len: usize,
    }

    impl Iterator for PanickingIterator {
        type Item = DropDetector;

        fn next(&mut self) -> Option<Self::Item> {
            let num = self.current;

            if num == self.panic_at {
                panic!("panicking index")
            }

            self.current += 1;
            Some(DropDetector::new(num))
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.len, Some(self.len))
        }
    }

    impl ExactSizeIterator for PanickingIterator {}

    #[test]
    fn issue_11_testcase3b() {
        let _ = std::panic::catch_unwind(|| {
            let mut chunk = Chunk::<DropDetector, U5>::new();
            chunk.push_back(DropDetector::new(1));
            chunk.push_back(DropDetector::new(2));
            chunk.push_back(DropDetector::new(3));

            chunk.insert_from(
                1,
                PanickingIterator {
                    current: 1,
                    panic_at: 1,
                    len: 1,
                },
            );
        });
    }

    struct FakeSizeIterator { reported: usize, actual: usize }
    impl Iterator for FakeSizeIterator {
        type Item = u8;
        fn next(&mut self) -> Option<Self::Item> {
            if self.actual == 0 {
                None
            } else {
                self.actual -= 1;
                Some(1)
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.reported, Some(self.reported))
        }
    }

    impl ExactSizeIterator for FakeSizeIterator {
        fn len(&self) -> usize {
            self.reported
        }
    }

    #[test]
    fn iterator_too_long() {
        let mut chunk = Chunk::<u8, U5>::new();
        chunk.push_back(0);
        chunk.push_back(1);
        chunk.push_back(2);
        chunk.insert_from(1, FakeSizeIterator { reported: 1, actual: 10 });

        let mut chunk = Chunk::<u8, U5>::new();
        chunk.push_back(1);
        chunk.insert_from(0, FakeSizeIterator { reported: 1, actual: 10 });

        let mut chunk = Chunk::<u8, U5>::new();
        chunk.insert_from(0, FakeSizeIterator { reported: 1, actual: 10 });
    }

    #[test]
    #[should_panic(expected = "ExactSizeIterator yielded fewer values than advertised")]
    fn iterator_too_short1() {
        let mut chunk = Chunk::<u8, U5>::new();
        chunk.push_back(0);
        chunk.push_back(1);
        chunk.push_back(2);
        chunk.insert_from(1, FakeSizeIterator { reported: 2, actual: 0 });
    }

    #[test]
    #[should_panic(expected = "ExactSizeIterator yielded fewer values than advertised")]
    fn iterator_too_short2() {
        let mut chunk = Chunk::<u8, U5>::new();
        chunk.push_back(1);
        chunk.insert_from(1, FakeSizeIterator { reported: 4, actual: 2 });
    }

    #[test]
    fn is_full() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..64 {
            assert_eq!(false, chunk.is_full());
            chunk.push_back(i);
        }
        assert_eq!(true, chunk.is_full());
    }

    #[test]
    fn push_back_front() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 12..20 {
            chunk.push_back(i);
        }
        assert_eq!(8, chunk.len());
        for i in (0..12).rev() {
            chunk.push_front(i);
        }
        assert_eq!(20, chunk.len());
        for i in 20..32 {
            chunk.push_back(i);
        }
        assert_eq!(32, chunk.len());
        let right: Vec<i32> = chunk.into_iter().collect();
        let left: Vec<i32> = (0..32).collect();
        assert_eq!(left, right);
    }

    #[test]
    fn push_and_pop() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..64 {
            chunk.push_back(i);
        }
        for i in 0..64 {
            assert_eq!(i, chunk.pop_front());
        }
        for i in 0..64 {
            chunk.push_front(i);
        }
        for i in 0..64 {
            assert_eq!(i, chunk.pop_back());
        }
    }

    #[test]
    fn drop_left() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..6 {
            chunk.push_back(i);
        }
        chunk.drop_left(3);
        let vec: Vec<i32> = chunk.into_iter().collect();
        assert_eq!(vec![3, 4, 5], vec);
    }

    #[test]
    fn drop_right() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..6 {
            chunk.push_back(i);
        }
        chunk.drop_right(3);
        let vec: Vec<i32> = chunk.into_iter().collect();
        assert_eq!(vec![0, 1, 2], vec);
    }

    #[test]
    fn split_off() {
        let mut left = Chunk::<_, U64>::new();
        for i in 0..6 {
            left.push_back(i);
        }
        let right = left.split_off(3);
        let left_vec: Vec<i32> = left.into_iter().collect();
        let right_vec: Vec<i32> = right.into_iter().collect();
        assert_eq!(vec![0, 1, 2], left_vec);
        assert_eq!(vec![3, 4, 5], right_vec);
    }

    #[test]
    fn append() {
        let mut left = Chunk::<_, U64>::new();
        for i in 0..32 {
            left.push_back(i);
        }
        let mut right = Chunk::<_, U64>::new();
        for i in (32..64).rev() {
            right.push_front(i);
        }
        left.append(&mut right);
        let out_vec: Vec<i32> = left.into_iter().collect();
        let should_vec: Vec<i32> = (0..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn ref_iter() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..64 {
            chunk.push_back(i);
        }
        let out_vec: Vec<&i32> = chunk.iter().collect();
        let should_vec_p: Vec<i32> = (0..64).collect();
        let should_vec: Vec<&i32> = should_vec_p.iter().collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn mut_ref_iter() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..64 {
            chunk.push_back(i);
        }
        let out_vec: Vec<&mut i32> = chunk.iter_mut().collect();
        let mut should_vec_p: Vec<i32> = (0..64).collect();
        let should_vec: Vec<&mut i32> = should_vec_p.iter_mut().collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn consuming_iter() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..64 {
            chunk.push_back(i);
        }
        let out_vec: Vec<i32> = chunk.into_iter().collect();
        let should_vec: Vec<i32> = (0..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn insert_middle() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..32 {
            chunk.push_back(i);
        }
        for i in 33..64 {
            chunk.push_back(i);
        }
        chunk.insert(32, 32);
        let out_vec: Vec<i32> = chunk.into_iter().collect();
        let should_vec: Vec<i32> = (0..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn insert_back() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..63 {
            chunk.push_back(i);
        }
        chunk.insert(63, 63);
        let out_vec: Vec<i32> = chunk.into_iter().collect();
        let should_vec: Vec<i32> = (0..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn insert_front() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 1..64 {
            chunk.push_front(64 - i);
        }
        chunk.insert(0, 0);
        let out_vec: Vec<i32> = chunk.into_iter().collect();
        let should_vec: Vec<i32> = (0..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    #[test]
    fn remove_value() {
        let mut chunk = Chunk::<_, U64>::new();
        for i in 0..64 {
            chunk.push_back(i);
        }
        chunk.remove(32);
        let out_vec: Vec<i32> = chunk.into_iter().collect();
        let should_vec: Vec<i32> = (0..32).chain(33..64).collect();
        assert_eq!(should_vec, out_vec);
    }

    use crate::tests::DropTest;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn dropping() {
        let counter = AtomicUsize::new(0);
        {
            let mut chunk: Chunk<DropTest<'_>> = Chunk::new();
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
        Chunk::<usize, U0>::unit(1);
    }

    #[test]
    #[should_panic(expected = "assertion failed: Self::CAPACITY >= 2")]
    fn pair_on_empty() {
        Chunk::<usize, U0>::pair(1, 2);
    }
}
