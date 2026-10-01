// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A fixed capacity array sized to match some other type `T`.
//!
//! See [`InlineArray`](struct.InlineArray.html)

use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::{Debug, Error, Formatter};
use core::hash::{Hash, Hasher};
use core::iter::FromIterator;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::ptr::NonNull;
use core::slice::{from_raw_parts, from_raw_parts_mut, Iter as SliceIter, IterMut as SliceIterMut};

mod iter;
pub use self::iter::{Drain, Iter};

/// A fixed capacity array sized to match some other type `T`.
///
/// This works like a vector, but allocated on the stack (and thus marginally
/// faster than `Vec`), with the allocated space exactly matching the size of
/// the given type `T`. The vector consists of a `usize` tracking its current
/// length and zero or more elements of type `A`. The capacity is thus
/// `( size_of::<T>() - size_of::<usize>() ) / size_of::<A>()`. This could lead
/// to situations where the capacity is zero, if `size_of::<A>()` is greater
/// than `size_of::<T>() - size_of::<usize>()`, which is not an error and
/// handled properly by the data structure.
///
/// If `size_of::<T>()` is less than `size_of::<usize>()`, meaning the vector
/// has no space to store its length, `InlineArray::new()` will panic.
///
/// This is meant to facilitate optimisations where a list data structure
/// allocates a fairly large struct for itself, allowing you to replace it with
/// an `InlineArray` until it grows beyond its capacity. This not only gives you
/// a performance boost at very small sizes, it also saves you from having to
/// allocate anything on the heap until absolutely necessary.
///
/// For instance, `im::Vector<A>` in its final form currently looks like this
/// (approximately):
///
/// ```rust, ignore
/// struct RRB<A> {
///     length: usize,
///     tree_height: usize,
///     outer_head: Rc<Chunk<A>>,
///     inner_head: Rc<Chunk<A>>,
///     tree: Rc<TreeNode<A>>,
///     inner_tail: Rc<Chunk<A>>,
///     outer_tail: Rc<Chunk<A>>,
/// }
/// ```
///
/// That's two `usize`s and five `Rc`s, which comes in at 56 bytes on x86_64
/// architectures. With `InlineArray`, that leaves us with 56 -
/// `size_of::<usize>()` = 48 bytes we can use before having to expand into the
/// full data struture. If `A` is `u8`, that's 48 elements, and even if `A` is a
/// pointer we can still keep 6 of them inline before we run out of capacity.
///
/// We can declare an enum like this:
///
/// ```rust, ignore
/// enum VectorWrapper<A> {
///     Inline(InlineArray<A, RRB<A>>),
///     Full(RRB<A>),
/// }
/// ```
///
/// Both of these will have the same size, and we can swap the `Inline` case out
/// with the `Full` case once the `InlineArray` runs out of capacity.
#[repr(C)]
pub struct InlineArray<A, T> {
    // Alignment tricks
    //
    // We need both the `_header_align` and `data` to be properly aligned in memory. We do a few tricks
    // to handle that.
    //
    // * An alignment is always power of 2. Therefore, with a pair of alignments, one is always
    //   a multiple of the other (one way or the other).
    // * A struct is aligned to at least the max alignment of each of its fields.
    // * A `repr(C)` struct follows the order of fields and pushes each as close to the previous one
    //   as allowed by alignment.
    //
    // By placing two "fake" fields that have 0 size, but an alignment first, we make sure that all
    // 3 start at the beginning of the struct and that all of them are aligned to their maximum
    // alignment.
    //
    // Unfortunately, we can't use `[A; 0]` to align to actual alignment of the type `A`, because
    // it prevents use of `InlineArray` in recursive types.
    // We rely on alignment of `u64`/`usize` or `T` to be sufficient, and panic otherwise. We use
    // `u64` to handle all common types on 32-bit systems too.
    //
    // Furthermore, because we don't know if `u64` or `A` has bigger alignment, we decide on case by
    // case basis if the header or the elements go first. By placing the one with higher alignment
    // requirements first, we align that one and the other one will be aligned "automatically" when
    // placed just after it.
    //
    // To the best of our knowledge, this is all guaranteed by the compiler. But just to make sure,
    // we have bunch of asserts in the constructor to check; as these are invariants enforced by
    // the compiler, it should be trivial for it to remove the checks so they are for free (if we
    // are correct) or will save us (if we are not).
    _header_align: [(u64, usize); 0],
    _phantom: PhantomData<A>,
    data: MaybeUninit<T>,
}

const fn capacity(
    host_size: usize,
    header_size: usize,
    element_size: usize,
    element_align: usize,
    container_align: usize,
) -> usize {
    if element_size == 0 {
        usize::MAX
    } else if element_align <= container_align && host_size > header_size {
        (host_size - header_size) / element_size
    } else {
        0 // larger alignment can't be guaranteed, so it'd be unsafe to store any elements
    }
}

impl<A, T> InlineArray<A, T> {
    const HOST_SIZE: usize = mem::size_of::<T>();
    const ELEMENT_SIZE: usize = mem::size_of::<A>();
    const HEADER_SIZE: usize = mem::size_of::<usize>();
    // Do we place the header before the elements or the other way around?
    const HEADER_FIRST: bool = mem::align_of::<usize>() >= mem::align_of::<A>();
    // Note: one of the following is always 0
    // How many usizes to skip before the first element?
    const ELEMENT_SKIP: usize = Self::HEADER_FIRST as usize;
    // How many elements to skip before the header
    const HEADER_SKIP: usize = Self::CAPACITY * (1 - Self::ELEMENT_SKIP);

    /// The maximum number of elements the `InlineArray` can hold.
    pub const CAPACITY: usize = capacity(
        Self::HOST_SIZE,
        Self::HEADER_SIZE,
        Self::ELEMENT_SIZE,
        mem::align_of::<A>(),
        mem::align_of::<Self>(),
    );

    #[inline]
    #[must_use]
    unsafe fn len_const(&self) -> *const usize {
        let ptr = self
            .data
            .as_ptr()
            .cast::<A>()
            .add(Self::HEADER_SKIP)
            .cast::<usize>();
        debug_assert!(ptr as usize % mem::align_of::<usize>() == 0);
        ptr
    }

    #[inline]
    #[must_use]
    pub(crate) unsafe fn len_mut(&mut self) -> *mut usize {
        let ptr = self
            .data
            .as_mut_ptr()
            .cast::<A>()
            .add(Self::HEADER_SKIP)
            .cast::<usize>();
        debug_assert!(ptr as usize % mem::align_of::<usize>() == 0);
        ptr
    }

    #[inline]
    #[must_use]
    pub(crate) unsafe fn data(&self) -> *const A {
        if Self::CAPACITY == 0 {
            return NonNull::<A>::dangling().as_ptr();
        }
        let ptr = self
            .data
            .as_ptr()
            .cast::<usize>()
            .add(Self::ELEMENT_SKIP)
            .cast::<A>();
        debug_assert!(ptr as usize % mem::align_of::<A>() == 0);
        ptr
    }

    #[inline]
    #[must_use]
    unsafe fn data_mut(&mut self) -> *mut A {
        if Self::CAPACITY == 0 {
            return NonNull::<A>::dangling().as_ptr();
        }
        let ptr = self
            .data
            .as_mut_ptr()
            .cast::<usize>()
            .add(Self::ELEMENT_SKIP)
            .cast::<A>();
        debug_assert!(ptr as usize % mem::align_of::<A>() == 0);
        ptr
    }

    #[inline]
    #[must_use]
    unsafe fn ptr_at(&self, index: usize) -> *const A {
        debug_assert!(index < Self::CAPACITY);
        self.data().add(index)
    }

    #[inline]
    #[must_use]
    unsafe fn ptr_at_mut(&mut self, index: usize) -> *mut A {
        debug_assert!(index < Self::CAPACITY);
        self.data_mut().add(index)
    }

    #[inline]
    unsafe fn read_at(&self, index: usize) -> A {
        ptr::read(self.ptr_at(index))
    }

    #[inline]
    unsafe fn write_at(&mut self, index: usize, value: A) {
        ptr::write(self.ptr_at_mut(index), value);
    }

    /// Get the length of the array.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        unsafe { *self.len_const() }
    }

    /// Test if the array is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Test if the array is at capacity.
    #[inline]
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() >= Self::CAPACITY
    }

    /// Construct a new empty array.
    ///
    /// # Panics
    ///
    /// If the element type requires large alignment, which is larger than
    /// both alignment of `usize` and alignment of the type that provides the capacity.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        assert!(Self::HOST_SIZE > Self::HEADER_SIZE);
        assert!(
            (Self::CAPACITY == 0) || (mem::align_of::<Self>() % mem::align_of::<A>() == 0),
            "InlineArray can't satisfy alignment of {}",
            core::any::type_name::<A>()
        );

        let mut self_ = Self {
            _header_align: [],
            _phantom: PhantomData,
            data: MaybeUninit::uninit(),
        };
        // Sanity check our assumptions about what is guaranteed by the compiler. If we are right,
        // these should completely optimize out of the resulting binary.
        assert_eq!(
            &self_ as *const _ as usize,
            self_.data.as_ptr() as usize,
            "Padding at the start of struct",
        );
        assert_eq!(
            self_.data.as_ptr() as usize % mem::align_of::<usize>(),
            0,
            "Unaligned header"
        );
        assert!(mem::size_of::<Self>() == mem::size_of::<T>() || mem::align_of::<T>() < mem::align_of::<Self>());
        assert_eq!(0, unsafe { self_.data() } as usize % mem::align_of::<A>());
        assert_eq!(0, unsafe { self_.data_mut() } as usize % mem::align_of::<A>());
        assert!(Self::ELEMENT_SKIP == 0 || Self::HEADER_SKIP == 0);
        unsafe { ptr::write(self_.len_mut(), 0usize) };
        self_
    }

    /// Push an item to the back of the array.
    ///
    /// Panics if the capacity of the array is exceeded.
    ///
    /// Time: O(1)
    pub fn push(&mut self, value: A) {
        if self.is_full() {
            panic!("InlineArray::push: chunk size overflow");
        }
        unsafe {
            self.write_at(self.len(), value);
            *self.len_mut() += 1;
        }
    }

    /// Pop an item from the back of the array.
    ///
    /// Returns `None` if the array is empty.
    ///
    /// Time: O(1)
    pub fn pop(&mut self) -> Option<A> {
        if self.is_empty() {
            None
        } else {
            unsafe {
                *self.len_mut() -= 1;
            }
            Some(unsafe { self.read_at(self.len()) })
        }
    }

    /// Insert a new value at index `index`, shifting all the following values
    /// to the right.
    ///
    /// Panics if the index is out of bounds or the array is at capacity.
    ///
    /// Time: O(n) for the number of items shifted
    pub fn insert(&mut self, index: usize, value: A) {
        if self.is_full() {
            panic!("InlineArray::push: chunk size overflow");
        }
        if index > self.len() {
            panic!("InlineArray::insert: index out of bounds");
        }
        unsafe {
            let src = self.ptr_at_mut(index);
            ptr::copy(src, src.add(1), self.len() - index);
            ptr::write(src, value);
            *self.len_mut() += 1;
        }
    }

    /// Remove the value at index `index`, shifting all the following values to
    /// the left.
    ///
    /// Returns the removed value, or `None` if the array is empty or the index
    /// is out of bounds.
    ///
    /// Time: O(n) for the number of items shifted
    pub fn remove(&mut self, index: usize) -> Option<A> {
        if index >= self.len() {
            None
        } else {
            unsafe {
                let src = self.ptr_at_mut(index);
                let value = ptr::read(src);
                *self.len_mut() -= 1;
                ptr::copy(src.add(1), src, self.len() - index);
                Some(value)
            }
        }
    }

    /// Split an array into two, the original array containing
    /// everything up to `index` and the returned array containing
    /// everything from `index` onwards.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// Time: O(n) for the number of items in the new chunk
    pub fn split_off(&mut self, index: usize) -> Self {
        if index > self.len() {
            panic!("InlineArray::split_off: index out of bounds");
        }
        let mut out = Self::new();
        if index < self.len() {
            unsafe {
                ptr::copy(self.ptr_at(index), out.data_mut(), self.len() - index);
                *out.len_mut() = self.len() - index;
                *self.len_mut() = index;
            }
        }
        out
    }

    #[inline]
    unsafe fn drop_contents(&mut self) {
        ptr::drop_in_place::<[A]>(&mut **self) // uses DerefMut
    }

    /// Discard the contents of the array.
    ///
    /// Time: O(n)
    pub fn clear(&mut self) {
        unsafe {
            self.drop_contents();
            *self.len_mut() = 0;
        }
    }

    /// Construct an iterator that drains values from the front of the array.
    pub fn drain(&mut self) -> Drain<'_, A, T> {
        Drain { array: self }
    }
}

impl<A, T> Drop for InlineArray<A, T> {
    fn drop(&mut self) {
        unsafe { self.drop_contents() }
    }
}

impl<A, T> Default for InlineArray<A, T> {
    fn default() -> Self {
        Self::new()
    }
}

// WANT:
// impl<A, T> Copy for InlineArray<A, T> where A: Copy {}

impl<A, T> Clone for InlineArray<A, T>
where
    A: Clone,
{
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        for i in 0..self.len() {
            unsafe {
                copy.write_at(i, self.get_unchecked(i).clone());
            }
        }
        unsafe {
            *copy.len_mut() = self.len();
        }
        copy
    }
}

impl<A, T> Deref for InlineArray<A, T> {
    type Target = [A];
    fn deref(&self) -> &Self::Target {
        unsafe { from_raw_parts(self.data(), self.len()) }
    }
}

impl<A, T> DerefMut for InlineArray<A, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { from_raw_parts_mut(self.data_mut(), self.len()) }
    }
}

impl<A, T> Borrow<[A]> for InlineArray<A, T> {
    fn borrow(&self) -> &[A] {
        self.deref()
    }
}

impl<A, T> BorrowMut<[A]> for InlineArray<A, T> {
    fn borrow_mut(&mut self) -> &mut [A] {
        self.deref_mut()
    }
}

impl<A, T> AsRef<[A]> for InlineArray<A, T> {
    fn as_ref(&self) -> &[A] {
        self.deref()
    }
}

impl<A, T> AsMut<[A]> for InlineArray<A, T> {
    fn as_mut(&mut self) -> &mut [A] {
        self.deref_mut()
    }
}
impl<A, T, Slice> PartialEq<Slice> for InlineArray<A, T>
where
    Slice: Borrow<[A]>,
    A: PartialEq,
{
    fn eq(&self, other: &Slice) -> bool {
        self.deref() == other.borrow()
    }
}

impl<A, T> Eq for InlineArray<A, T> where A: Eq {}

impl<A, T> PartialOrd for InlineArray<A, T>
where
    A: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<A, T> Ord for InlineArray<A, T>
where
    A: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<A, T> Debug for InlineArray<A, T>
where
    A: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("Chunk")?;
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<A, T> Hash for InlineArray<A, T>
where
    A: Hash,
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

impl<A, T> IntoIterator for InlineArray<A, T> {
    type Item = A;
    type IntoIter = Iter<A, T>;
    fn into_iter(self) -> Self::IntoIter {
        Iter { array: self }
    }
}

impl<A, T> FromIterator<A> for InlineArray<A, T> {
    fn from_iter<I>(it: I) -> Self
    where
        I: IntoIterator<Item = A>,
    {
        let mut chunk = Self::new();
        for item in it {
            chunk.push(item);
        }
        chunk
    }
}

impl<'a, A, T> IntoIterator for &'a InlineArray<A, T> {
    type Item = &'a A;
    type IntoIter = SliceIter<'a, A>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A, T> IntoIterator for &'a mut InlineArray<A, T> {
    type Item = &'a mut A;
    type IntoIter = SliceIterMut<'a, A>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<A, T> Extend<A> for InlineArray<A, T> {
    /// Append the contents of the iterator to the back of the array.
    ///
    /// Panics if the array exceeds its capacity.
    ///
    /// Time: O(n) for the length of the iterator
    fn extend<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = A>,
    {
        for item in it {
            self.push(item);
        }
    }
}

impl<'a, A, T> Extend<&'a A> for InlineArray<A, T>
where
    A: 'a + Copy,
{
    /// Append the contents of the iterator to the back of the array.
    ///
    /// Panics if the array exceeds its capacity.
    ///
    /// Time: O(n) for the length of the iterator
    fn extend<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = &'a A>,
    {
        for item in it {
            self.push(*item);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::DropTest;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn dropping() {
        let counter = AtomicUsize::new(0);
        {
            let mut chunk: InlineArray<DropTest<'_>, [usize; 32]> = InlineArray::new();
            for _i in 0..16 {
                chunk.push(DropTest::new(&counter));
            }
            assert_eq!(16, counter.load(Ordering::Relaxed));
            for _i in 0..8 {
                chunk.pop();
            }
            assert_eq!(8, counter.load(Ordering::Relaxed));
        }
        assert_eq!(0, counter.load(Ordering::Relaxed));
    }

    #[test]
    fn zero_sized_values() {
        let mut chunk: InlineArray<(), [usize; 32]> = InlineArray::new();
        for _i in 0..65536 {
            chunk.push(());
        }
        assert_eq!(65536, chunk.len());
        assert_eq!(Some(()), chunk.pop());
    }

    #[test]
    fn low_align_base() {
        let mut chunk: InlineArray<String, [u8; 512]> = InlineArray::new();
        chunk.push("Hello".to_owned());
        assert_eq!(chunk[0], "Hello");

        let mut chunk: InlineArray<String, [u16; 512]> = InlineArray::new();
        chunk.push("Hello".to_owned());
        assert_eq!(chunk[0], "Hello");
    }

    #[test]
    fn float_align() {
        let mut chunk: InlineArray<f64, [u8; 16]> = InlineArray::new();
        chunk.push(1234.);
        assert_eq!(chunk[0], 1234.);

        let mut chunk: InlineArray<f64, [u8; 17]> = InlineArray::new();
        chunk.push(1234.);
        assert_eq!(chunk[0], 1234.);
    }

    #[test]
    fn recursive_types_compile() {
        #[allow(dead_code)]
        enum Recursive {
            A(InlineArray<Recursive, u64>),
            B,
        }
    }

    #[test]
    fn insufficient_alignment1() {
        #[repr(align(256))]
        struct BigAlign(u8);
        #[repr(align(32))]
        struct MediumAlign(u8);

        assert_eq!(0, InlineArray::<BigAlign, [usize; 256]>::CAPACITY);
        assert_eq!(0, InlineArray::<BigAlign, [u64; 256]>::CAPACITY);
        assert_eq!(0, InlineArray::<BigAlign, [f64; 256]>::CAPACITY);
        assert_eq!(0, InlineArray::<BigAlign, [MediumAlign; 256]>::CAPACITY);
    }

    #[test]
    fn insufficient_alignment2() {
        #[repr(align(256))]
        struct BigAlign(usize);

        let mut bad: InlineArray<BigAlign, [usize; 256]> = InlineArray::new();
        assert_eq!(0, InlineArray::<BigAlign, [usize; 256]>::CAPACITY);
        assert_eq!(0, bad.len());
        assert_eq!(0, bad[..].len());
        assert_eq!(true, bad.is_full());
        assert_eq!(0, bad.drain().count());
        assert!(bad.pop().is_none());
        assert!(bad.remove(0).is_none());
        assert!(bad.split_off(0).is_full());
        bad.clear();
    }

    #[test]
    fn sufficient_alignment1() {
        #[repr(align(256))]
        struct BigAlign(u8);

        assert_eq!(13, InlineArray::<BigAlign, [BigAlign; 14]>::CAPACITY);
        assert_eq!(1, InlineArray::<BigAlign, [BigAlign; 2]>::CAPACITY);
        assert_eq!(0, InlineArray::<BigAlign, [BigAlign; 1]>::CAPACITY);

        let mut chunk: InlineArray<BigAlign, [BigAlign; 2]> = InlineArray::new();
        chunk.push(BigAlign(42));
        assert_eq!(
            chunk.get(0).unwrap() as *const _ as usize % mem::align_of::<BigAlign>(),
            0
        );
    }

    #[test]
    fn sufficient_alignment2() {
        #[repr(align(128))]
        struct BigAlign([u8; 64]);
        #[repr(align(256))]
        struct BiggerAlign(u8);
        assert_eq!(128, mem::align_of::<BigAlign>());
        assert_eq!(256, mem::align_of::<BiggerAlign>());

        assert_eq!(199, InlineArray::<BigAlign, [BiggerAlign; 100]>::CAPACITY);
        assert_eq!(3, InlineArray::<BigAlign, [BiggerAlign; 2]>::CAPACITY);
        assert_eq!(1, InlineArray::<BigAlign, [BiggerAlign; 1]>::CAPACITY);
        assert_eq!(0, InlineArray::<BigAlign, [BiggerAlign; 0]>::CAPACITY);

        let mut chunk: InlineArray<BigAlign, [BiggerAlign; 1]> = InlineArray::new();
        chunk.push(BigAlign([0; 64]));
        assert_eq!(
            chunk.get(0).unwrap() as *const _ as usize % mem::align_of::<BigAlign>(),
            0
        );
    }
}
