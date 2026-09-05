// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::mem::{replace, swap};
use std::ops::{Range, RangeBounds};
use std::ptr::null;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::nodes::chunk::Chunk;
use crate::sync::Lock;
use crate::util::{to_range, PoolRef, Ref};
use crate::vector::{
    Iter, IterMut, RRBPool, Rrb, Vector,
    VectorInner::{Full, Inline, Single},
};

/// Focused indexing over a [`Vector`][Vector].
///
/// By remembering the last tree node accessed through an index lookup and the
/// path we took to get there, we can speed up lookups for adjacent indices
/// tremendously. Lookups on indices in the same node are instantaneous, and
/// lookups on sibling nodes are also very fast.
///
/// A `Focus` can also be used as a restricted view into a vector, using the
/// [`narrow`][narrow] and [`split_at`][split_at] methods.
///
/// # When should I use a `Focus` for better performance?
///
/// `Focus` is useful when you need to perform a large number of index lookups
/// that are more likely than not to be close to each other. It's usually worth
/// using a `Focus` in any situation where you're batching a lot of index
/// lookups together, even if they're not obviously adjacent - there's likely
/// to be some performance gain for even completely random access.
///
/// If you're just iterating forwards or backwards over the [`Vector`][Vector]
/// in order, you're better off with a regular iterator, which, in fact, is
/// implemented using a `Focus`, but provides a simpler interface.
///
/// If you're just doing a very small number of index lookups, the setup cost
/// for the `Focus` is probably not worth it.
///
/// A `Focus` is never faster than an index lookup on a small [`Vector`][Vector]
/// with a length below the internal RRB tree's branching factor of 64.
///
/// # Examples
///
/// This example is contrived, as the better way to iterate forwards or
/// backwards over a vector is with an actual iterator. Even so, the version
/// using a `Focus` should run nearly an order of magnitude faster than the
/// version using index lookups at a length of 1000. It should also be noted
/// that [`vector::Iter`][Iter] is actually implemented using a `Focus` behind
/// the scenes, so the performance of the two should be identical.
///
/// ```rust
/// # #[macro_use] extern crate im;
/// # use im::vector::Vector;
/// # use std::iter::FromIterator;
/// let mut vec: Vector<i64> = Vector::from_iter(0..1000);
///
/// // Summing a vector, the slow way:
/// let mut sum = 0;
/// for i in 0..1000 {
///     sum += *vec.get(i).unwrap();
/// }
/// assert_eq!(499500, sum);
///
/// // Summing a vector faster using a Focus:
/// let mut sum = 0;
/// let mut focus = vec.focus();
/// for i in 0..1000 {
///     sum += *focus.get(i).unwrap();
/// }
/// assert_eq!(499500, sum);
///
/// // And the easy way, for completeness:
/// let sum: i64 = vec.iter().sum();
/// assert_eq!(499500, sum);
/// ```
///
/// [Vector]: enum.Vector.html
/// [Iter]: struct.Iter.html
/// [narrow]: #method.narrow
/// [split_at]: #method.split_at
pub enum Focus<'a, A> {
    #[doc(hidden)]
    Single(&'a [A]),
    #[doc(hidden)]
    Full(TreeFocus<A>),
}

impl<'a, A> Focus<'a, A>
where
    A: Clone + 'a,
{
    /// Construct a `Focus` for a [`Vector`][Vector].
    ///
    /// [Vector]: enum.Vector.html
    pub fn new(vector: &'a Vector<A>) -> Self {
        match &vector.vector {
            Inline(_, chunk) => Focus::Single(chunk),
            Single(_, chunk) => Focus::Single(chunk),
            Full(_, tree) => Focus::Full(TreeFocus::new(tree)),
        }
    }

    /// Get the length of the focused [`Vector`][Vector].
    ///
    /// [Vector]: enum.Vector.html
    pub fn len(&self) -> usize {
        match self {
            Focus::Single(chunk) => chunk.len(),
            Focus::Full(tree) => tree.len(),
        }
    }

    /// Test if the focused [`Vector`][Vector] is empty.
    ///
    /// [Vector]: enum.Vector.html
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the value at a given index.
    pub fn get(&mut self, index: usize) -> Option<&A> {
        match self {
            Focus::Single(chunk) => chunk.get(index),
            Focus::Full(tree) => tree.get(index),
        }
    }

    /// Get a reference to the value at a given index.
    ///
    /// Panics if the index is out of bounds.
    pub fn index(&mut self, index: usize) -> &A {
        self.get(index).expect("index out of bounds")
    }

    /// Get the chunk for the given index.
    ///
    /// This gives you a reference to the leaf node that contains the index,
    /// along with its start and end indices.
    pub fn chunk_at(&mut self, index: usize) -> (Range<usize>, &[A]) {
        let len = self.len();
        if index >= len {
            panic!("vector::Focus::chunk_at: index out of bounds");
        }
        match self {
            Focus::Single(chunk) => (0..len, chunk),
            Focus::Full(tree) => tree.get_chunk(index),
        }
    }

    /// Narrow the focus onto a subslice of the vector.
    ///
    /// `Focus::narrow(range)` has the same effect as `&slice[range]`, without
    /// actually modifying the underlying vector.
    ///
    /// Panics if the range isn't fully inside the current focus.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate im;
    /// # use im::vector::Vector;
    /// # use std::iter::FromIterator;
    /// let vec = Vector::from_iter(0..1000);
    /// let narrowed = vec.focus().narrow(100..200);
    /// let narrowed_vec = narrowed.into_iter().cloned().collect();
    /// assert_eq!(Vector::from_iter(100..200), narrowed_vec);
    /// ```
    ///
    /// [slice::split_at]: https://doc.rust-lang.org/std/primitive.slice.html#method.split_at
    /// [Vector::split_at]: enum.Vector.html#method.split_at
    pub fn narrow<R>(self, range: R) -> Self
    where
        R: RangeBounds<usize>,
    {
        let r = to_range(&range, self.len());
        if r.start >= r.end || r.start >= self.len() {
            panic!("vector::Focus::narrow: range out of bounds");
        }
        match self {
            Focus::Single(chunk) => Focus::Single(&chunk[r]),
            Focus::Full(tree) => Focus::Full(tree.narrow(r)),
        }
    }

    /// Split the focus into two.
    ///
    /// Given an index `index`, consume the focus and produce two new foci, the
    /// left onto indices `0..index`, and the right onto indices `index..N`
    /// where `N` is the length of the current focus.
    ///
    /// Panics if the index is out of bounds.
    ///
    /// This is the moral equivalent of [`slice::split_at`][slice::split_at], in
    /// that it leaves the underlying data structure unchanged, unlike
    /// [`Vector::split_at`][Vector::split_at].
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate im;
    /// # use im::vector::Vector;
    /// # use std::iter::FromIterator;
    /// let vec = Vector::from_iter(0..1000);
    /// let (left, right) = vec.focus().split_at(500);
    /// let left_vec = left.into_iter().cloned().collect();
    /// let right_vec = right.into_iter().cloned().collect();
    /// assert_eq!(Vector::from_iter(0..500), left_vec);
    /// assert_eq!(Vector::from_iter(500..1000), right_vec);
    /// ```
    ///
    /// [slice::split_at]: https://doc.rust-lang.org/std/primitive.slice.html#method.split_at
    /// [Vector::split_at]: enum.Vector.html#method.split_at
    pub fn split_at(self, index: usize) -> (Self, Self) {
        if index >= self.len() {
            panic!("vector::Focus::split_at: index out of bounds");
        }
        match self {
            Focus::Single(chunk) => {
                let (left, right) = chunk.split_at(index);
                (Focus::Single(left), Focus::Single(right))
            }
            Focus::Full(tree) => {
                let (left, right) = tree.split_at(index);
                (Focus::Full(left), Focus::Full(right))
            }
        }
    }
}

impl<'a, A> IntoIterator for Focus<'a, A>
where
    A: Clone + 'a,
{
    type Item = &'a A;
    type IntoIter = Iter<'a, A>;

    fn into_iter(self) -> Self::IntoIter {
        Iter::from_focus(self)
    }
}

impl<'a, A> Clone for Focus<'a, A>
where
    A: Clone + 'a,
{
    fn clone(&self) -> Self {
        match self {
            Focus::Single(chunk) => Focus::Single(chunk),
            Focus::Full(tree) => Focus::Full(tree.clone()),
        }
    }
}

pub struct TreeFocus<A> {
    tree: Rrb<A>,
    view: Range<usize>,
    middle_range: Range<usize>,
    target_range: Range<usize>,
    target_ptr: *const Chunk<A>,
}

impl<A> Clone for TreeFocus<A> {
    fn clone(&self) -> Self {
        let tree = self.tree.clone();
        TreeFocus {
            view: self.view.clone(),
            middle_range: self.middle_range.clone(),
            target_range: 0..0,
            target_ptr: null(),
            tree,
        }
    }
}

#[allow(unsafe_code)]
#[cfg(threadsafe)]
unsafe impl<A: Send> Send for TreeFocus<A> {}
#[allow(unsafe_code)]
#[cfg(threadsafe)]
unsafe impl<A: Sync> Sync for TreeFocus<A> {}

#[inline]
fn contains<A: Ord>(range: &Range<A>, index: &A) -> bool {
    *index >= range.start && *index < range.end
}

impl<A> TreeFocus<A>
where
    A: Clone,
{
    fn new(tree: &Rrb<A>) -> Self {
        let middle_start = tree.outer_f.len() + tree.inner_f.len();
        let middle_end = middle_start + tree.middle.len();
        TreeFocus {
            tree: tree.clone(),
            view: 0..tree.length,
            middle_range: middle_start..middle_end,
            target_range: 0..0,
            target_ptr: null(),
        }
    }

    fn len(&self) -> usize {
        self.view.end - self.view.start
    }

    fn narrow(self, mut view: Range<usize>) -> Self {
        view.start += self.view.start;
        view.end += self.view.start;
        TreeFocus {
            view,
            middle_range: self.middle_range.clone(),
            target_range: 0..0,
            target_ptr: null(),
            tree: self.tree,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let len = self.len();
        let left = self.clone().narrow(0..index);
        let right = self.narrow(index..len);
        (left, right)
    }

    fn physical_index(&self, index: usize) -> usize {
        debug_assert!(index < self.view.end);
        self.view.start + index
    }

    fn logical_range(&self, range: &Range<usize>) -> Range<usize> {
        (range.start - self.view.start)..(range.end - self.view.start)
    }

    fn set_focus(&mut self, index: usize) {
        if index < self.middle_range.start {
            let outer_len = self.tree.outer_f.len();
            if index < outer_len {
                self.target_range = 0..outer_len;
                self.target_ptr = &*self.tree.outer_f;
            } else {
                self.target_range = outer_len..self.middle_range.start;
                self.target_ptr = &*self.tree.inner_f;
            }
        } else if index >= self.middle_range.end {
            let outer_start = self.middle_range.end + self.tree.inner_b.len();
            if index < outer_start {
                self.target_range = self.middle_range.end..outer_start;
                self.target_ptr = &*self.tree.inner_b;
            } else {
                self.target_range = outer_start..self.tree.length;
                self.target_ptr = &*self.tree.outer_b;
            }
        } else {
            let tree_index = index - self.middle_range.start;
            let (range, ptr) = self
                .tree
                .middle
                .lookup_chunk(self.tree.middle_level, 0, tree_index);
            self.target_range =
                (range.start + self.middle_range.start)..(range.end + self.middle_range.start);
            self.target_ptr = ptr;
        }
    }

    #[allow(unsafe_code)]
    fn get_focus(&self) -> &Chunk<A> {
        unsafe { &*self.target_ptr }
    }

    pub fn get(&mut self, index: usize) -> Option<&A> {
        if index >= self.len() {
            return None;
        }
        let phys_index = self.physical_index(index);
        if !contains(&self.target_range, &phys_index) {
            self.set_focus(phys_index);
        }
        let target_phys_index = phys_index - self.target_range.start;
        Some(&self.get_focus()[target_phys_index])
    }

    pub fn get_chunk(&mut self, index: usize) -> (Range<usize>, &[A]) {
        let phys_index = self.physical_index(index);
        if !contains(&self.target_range, &phys_index) {
            self.set_focus(phys_index);
        }
        let mut slice: &[A] = self.get_focus().as_slice();
        let mut left = 0;
        let mut right = 0;
        if self.target_range.start < self.view.start {
            left = self.view.start - self.target_range.start;
        }
        if self.target_range.end > self.view.end {
            right = self.target_range.end - self.view.end;
        }
        slice = &slice[left..(slice.len() - right)];
        let phys_range = (self.target_range.start + left)..(self.target_range.end - right);
        (self.logical_range(&phys_range), slice)
    }
}

/// A mutable version of [`Focus`][Focus].
///
/// See [`Focus`][Focus] for more details.
///
/// You can only build one `FocusMut` at a time for a vector, effectively
/// keeping a lock on the vector until you're done with the focus, which relies
/// on the structure of the vector not changing while it exists.
///
/// ```rust,compile_fail
/// # #[macro_use] extern crate im;
/// # use im::vector::Vector;
/// # use std::iter::FromIterator;
/// let mut vec = Vector::from_iter(0..1000);
/// let focus1 = vec.focus_mut();
/// // Fails here in 2015 edition because you're creating
/// // two mutable references to the same thing.
/// let focus2 = vec.focus_mut();
/// // Fails here in 2018 edition because creating focus2
/// // made focus1's lifetime go out of scope.
/// assert_eq!(Some(&0), focus1.get(0));
/// ```
///
/// On the other hand, you can split that one focus into multiple sub-focuses,
/// which is safe because they can't overlap:
///
/// ```rust
/// # #[macro_use] extern crate im;
/// # use im::vector::Vector;
/// # use std::iter::FromIterator;
/// let mut vec = Vector::from_iter(0..1000);
/// let focus = vec.focus_mut();
/// let (mut left, mut right) = focus.split_at(500);
/// assert_eq!(Some(&0), left.get(0));
/// assert_eq!(Some(&500), right.get(0));
/// ```
///
/// These sub-foci also work as a lock on the vector, even if the focus they
/// were created from goes out of scope.
///
/// ```rust,compile_fail
/// # #[macro_use] extern crate im;
/// # use im::vector::Vector;
/// # use std::iter::FromIterator;
/// let mut vec = Vector::from_iter(0..1000);
/// let (left, right) = {
///     let focus = vec.focus_mut();
///     focus.split_at(500)
/// };
/// // `left` and `right` are still in scope even if `focus` isn't, so we can't
/// // create another focus:
/// let focus2 = vec.focus_mut();
/// assert_eq!(Some(&0), left.get(0));
/// ```
///
/// [Focus]: enum.Focus.html
pub enum FocusMut<'a, A> {
    #[doc(hidden)]
    Single(RRBPool<A>, &'a mut [A]),
    #[doc(hidden)]
    Full(RRBPool<A>, TreeFocusMut<'a, A>),
}

impl<'a, A> FocusMut<'a, A>
where
    A: Clone + 'a,
{
    /// Construct a `FocusMut` for a `Vector`.
    pub fn new(vector: &'a mut Vector<A>) -> Self {
        match &mut vector.vector {
            Inline(pool, chunk) => FocusMut::Single(pool.clone(), chunk),
            Single(pool, chunk) => FocusMut::Single(
                pool.clone(),
                PoolRef::make_mut(&pool.value_pool, chunk).as_mut_slice(),
            ),
            Full(pool, tree) => FocusMut::Full(pool.clone(), TreeFocusMut::new(tree)),
        }
    }

    /// Get the length of the focused `Vector`.
    pub fn len(&self) -> usize {
        match self {
            FocusMut::Single(_, chunk) => chunk.len(),
            FocusMut::Full(_, tree) => tree.len(),
        }
    }

    /// Test if the focused `Vector` is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the value at a given index.
    pub fn get(&mut self, index: usize) -> Option<&A> {
        self.get_mut(index).map(|r| &*r)
    }

    /// Get a mutable reference to the value at a given index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut A> {
        match self {
            FocusMut::Single(_, chunk) => chunk.get_mut(index),
            FocusMut::Full(pool, tree) => tree.get(pool, index),
        }
    }

    /// Get a reference to the value at a given index.
    ///
    /// Panics if the index is out of bounds.
    pub fn index(&mut self, index: usize) -> &A {
        &*self.index_mut(index)
    }

    /// Get a mutable reference to the value at a given index.
    ///
    /// Panics if the index is out of bounds.
    #[allow(clippy::should_implement_trait)] // would if I could
    pub fn index_mut(&mut self, index: usize) -> &mut A {
        self.get_mut(index).expect("index out of bounds")
    }

    /// Update the value at a given index.
    ///
    /// Returns `None` if the index is out of bounds, or the replaced value
    /// otherwise.
    pub fn set(&mut self, index: usize, value: A) -> Option<A> {
        self.get_mut(index).map(|pos| replace(pos, value))
    }

    /// Swap the values at two given indices.
    ///
    /// Panics if either index is out of bounds.
    ///
    /// If the indices are equal, this function returns without doing anything.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        self.pair(a, b, |left, right| swap(left, right));
    }

    /// Lookup two indices simultaneously and run a function over them.
    ///
    /// Useful because the borrow checker won't let you have more than one
    /// mutable reference into the same data structure at any given time.
    ///
    /// Panics if either index is out of bounds, or if they are the same index.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate im;
    /// # use im::vector::Vector;
    /// # use std::iter::FromIterator;
    /// let mut vec = vector![1, 2, 3, 4, 5];
    /// vec.focus_mut().pair(1, 3, |a, b| *a += *b);
    /// assert_eq!(vector![1, 6, 3, 4, 5], vec);
    /// ```
    #[allow(unsafe_code)]
    pub fn pair<F, B>(&mut self, a: usize, b: usize, mut f: F) -> B
    where
        F: FnMut(&mut A, &mut A) -> B,
    {
        if a == b {
            panic!("vector::FocusMut::pair: indices cannot be equal!");
        }
        let pa: *mut A = self.index_mut(a);
        let pb: *mut A = self.index_mut(b);
        unsafe { f(&mut *pa, &mut *pb) }
    }

    /// Lookup three indices simultaneously and run a function over them.
    ///
    /// Useful because the borrow checker won't let you have more than one
    /// mutable reference into the same data structure at any given time.
    ///
    /// Panics if any index is out of bounds, or if any indices are equal.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate im;
    /// # use im::vector::Vector;
    /// # use std::iter::FromIterator;
    /// let mut vec = vector![1, 2, 3, 4, 5];
    /// vec.focus_mut().triplet(0, 2, 4, |a, b, c| *a += *b + *c);
    /// assert_eq!(vector![9, 2, 3, 4, 5], vec);
    /// ```
    #[allow(unsafe_code)]
    pub fn triplet<F, B>(&mut self, a: usize, b: usize, c: usize, mut f: F) -> B
    where
        F: FnMut(&mut A, &mut A, &mut A) -> B,
    {
        if a == b || b == c || a == c {
            panic!("vector::FocusMut::triplet: indices cannot be equal!");
        }
        let pa: *mut A = self.index_mut(a);
        let pb: *mut A = self.index_mut(b);
        let pc: *mut A = self.index_mut(c);
        unsafe { f(&mut *pa, &mut *pb, &mut *pc) }
    }

    /// Get the chunk for the given index.
    ///
    /// This gives you a reference to the leaf node that contains the index,
    /// along with its start and end indices.
    pub fn chunk_at(&mut self, index: usize) -> (Range<usize>, &mut [A]) {
        let len = self.len();
        if index >= len {
            panic!("vector::FocusMut::chunk_at: index out of bounds");
        }
        match self {
            FocusMut::Single(_, chunk) => (0..len, chunk),
            FocusMut::Full(pool, tree) => {
                let (range, chunk) = tree.get_chunk(pool, index);
                (range, chunk)
            }
        }
    }

    /// Narrow the focus onto a subslice of the vector.
    ///
    /// `FocusMut::narrow(range)` has the same effect as `&slice[range]`, without
    /// actually modifying the underlying vector.
    ///
    /// Panics if the range isn't fully inside the current focus.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate im;
    /// # use im::vector::Vector;
    /// # use std::iter::FromIterator;
    /// let mut vec = Vector::from_iter(0..1000);
    /// let narrowed = vec.focus_mut().narrow(100..200);
    /// let narrowed_vec = narrowed.unmut().into_iter().cloned().collect();
    /// assert_eq!(Vector::from_iter(100..200), narrowed_vec);
    /// ```
    ///
    /// [slice::split_at]: https://doc.rust-lang.org/std/primitive.slice.html#method.split_at
    /// [Vector::split_at]: enum.Vector.html#method.split_at
    pub fn narrow<R>(self, range: R) -> Self
    where
        R: RangeBounds<usize>,
    {
        let r = to_range(&range, self.len());
        if r.start > r.end || r.start > self.len() {
            panic!("vector::FocusMut::narrow: range out of bounds");
        }
        match self {
            FocusMut::Single(pool, chunk) => FocusMut::Single(pool, &mut chunk[r]),
            FocusMut::Full(pool, tree) => FocusMut::Full(pool, tree.narrow(r)),
        }
    }

    /// Split the focus into two.
    ///
    /// Given an index `index`, consume the focus and produce two new foci, the
    /// left onto indices `0..index`, and the right onto indices `index..N`
    /// where `N` is the length of the current focus.
    ///
    /// Panics if the index is out of bounds.
    ///
    /// This is the moral equivalent of [`slice::split_at`][slice::split_at], in
    /// that it leaves the underlying data structure unchanged, unlike
    /// [`Vector::split_at`][Vector::split_at].
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate im;
    /// # use im::vector::Vector;
    /// # use std::iter::FromIterator;
    /// let mut vec = Vector::from_iter(0..1000);
    /// {
    ///     let (left, right) = vec.focus_mut().split_at(500);
    ///     for ptr in left {
    ///         *ptr += 100;
    ///     }
    ///     for ptr in right {
    ///         *ptr -= 100;
    ///     }
    /// }
    /// let expected = Vector::from_iter(100..600)
    ///              + Vector::from_iter(400..900);
    /// assert_eq!(expected, vec);
    /// ```
    ///
    /// [slice::split_at]: https://doc.rust-lang.org/std/primitive.slice.html#method.split_at
    /// [Vector::split_at]: enum.Vector.html#method.split_at
    #[allow(clippy::redundant_clone)]
    pub fn split_at(self, index: usize) -> (Self, Self) {
        if index > self.len() {
            panic!("vector::FocusMut::split_at: index out of bounds");
        }
        match self {
            FocusMut::Single(pool, chunk) => {
                let (left, right) = chunk.split_at_mut(index);
                (
                    FocusMut::Single(pool.clone(), left),
                    FocusMut::Single(pool, right),
                )
            }
            FocusMut::Full(pool, tree) => {
                let (left, right) = tree.split_at(index);
                (
                    FocusMut::Full(pool.clone(), left),
                    FocusMut::Full(pool, right),
                )
            }
        }
    }

    /// Convert a `FocusMut` into a `Focus`.
    pub fn unmut(self) -> Focus<'a, A> {
        match self {
            FocusMut::Single(_, chunk) => Focus::Single(chunk),
            FocusMut::Full(_, mut tree) => Focus::Full(TreeFocus {
                tree: {
                    let t = tree.tree.lock().unwrap();
                    (*t).clone()
                },
                view: tree.view.clone(),
                middle_range: tree.middle_range.clone(),
                target_range: 0..0,
                target_ptr: null(),
            }),
        }
    }
}

impl<'a, A> IntoIterator for FocusMut<'a, A>
where
    A: Clone + 'a,
{
    type Item = &'a mut A;
    type IntoIter = IterMut<'a, A>;

    fn into_iter(self) -> Self::IntoIter {
        IterMut::from_focus(self)
    }
}

impl<'a, A> From<FocusMut<'a, A>> for Focus<'a, A>
where
    A: Clone + 'a,
{
    fn from(f: FocusMut<'a, A>) -> Self {
        f.unmut()
    }
}

pub struct TreeFocusMut<'a, A> {
    tree: Lock<&'a mut Rrb<A>>,
    view: Range<usize>,
    middle_range: Range<usize>,
    target_range: Range<usize>,
    target_ptr: AtomicPtr<Chunk<A>>,
}

impl<'a, A> TreeFocusMut<'a, A>
where
    A: Clone + 'a,
{
    fn new(tree: &'a mut Rrb<A>) -> Self {
        let middle_start = tree.outer_f.len() + tree.inner_f.len();
        let middle_end = middle_start + tree.middle.len();
        TreeFocusMut {
            view: 0..tree.length,
            tree: Lock::new(tree),
            middle_range: middle_start..middle_end,
            target_range: 0..0,
            target_ptr: AtomicPtr::default(),
        }
    }

    fn len(&self) -> usize {
        self.view.end - self.view.start
    }

    fn narrow(self, mut view: Range<usize>) -> Self {
        view.start += self.view.start;
        view.end += self.view.start;
        TreeFocusMut {
            view,
            middle_range: self.middle_range.clone(),
            target_range: 0..0,
            target_ptr: AtomicPtr::default(),
            tree: self.tree,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let len = self.len();
        debug_assert!(index <= len);
        #[allow(unsafe_code)]
        let left = TreeFocusMut {
            view: self.view.start..(self.view.start + index),
            middle_range: self.middle_range.clone(),
            target_range: 0..0,
            target_ptr: AtomicPtr::default(),
            tree: self.tree.clone(),
        };
        let right = TreeFocusMut {
            view: (self.view.start + index)..(self.view.start + len),
            middle_range: self.middle_range.clone(),
            target_range: 0..0,
            target_ptr: AtomicPtr::default(),
            tree: self.tree,
        };
        (left, right)
    }

    fn physical_index(&self, index: usize) -> usize {
        debug_assert!(index < self.view.end);
        self.view.start + index
    }

    fn logical_range(&self, range: &Range<usize>) -> Range<usize> {
        (range.start - self.view.start)..(range.end - self.view.start)
    }

    fn set_focus(&mut self, pool: &RRBPool<A>, index: usize) {
        let mut tree = self
            .tree
            .lock()
            .expect("im::vector::Focus::set_focus: unable to acquire exclusive lock on Vector");
        if index < self.middle_range.start {
            let outer_len = tree.outer_f.len();
            if index < outer_len {
                self.target_range = 0..outer_len;
                self.target_ptr.store(
                    PoolRef::make_mut(&pool.value_pool, &mut tree.outer_f),
                    Ordering::Relaxed,
                );
            } else {
                self.target_range = outer_len..self.middle_range.start;
                self.target_ptr.store(
                    PoolRef::make_mut(&pool.value_pool, &mut tree.inner_f),
                    Ordering::Relaxed,
                );
            }
        } else if index >= self.middle_range.end {
            let outer_start = self.middle_range.end + tree.inner_b.len();
            if index < outer_start {
                self.target_range = self.middle_range.end..outer_start;
                self.target_ptr.store(
                    PoolRef::make_mut(&pool.value_pool, &mut tree.inner_b),
                    Ordering::Relaxed,
                );
            } else {
                self.target_range = outer_start..tree.length;
                self.target_ptr.store(
                    PoolRef::make_mut(&pool.value_pool, &mut tree.outer_b),
                    Ordering::Relaxed,
                );
            }
        } else {
            let tree_index = index - self.middle_range.start;
            let level = tree.middle_level;
            let middle = Ref::make_mut(&mut tree.middle);
            let (range, ptr) = middle.lookup_chunk_mut(pool, level, 0, tree_index);
            self.target_range =
                (range.start + self.middle_range.start)..(range.end + self.middle_range.start);
            self.target_ptr.store(ptr, Ordering::Relaxed);
        }
    }

    #[allow(unsafe_code)]
    fn get_focus(&mut self) -> &mut Chunk<A> {
        unsafe { &mut *self.target_ptr.load(Ordering::Relaxed) }
    }

    pub fn get(&mut self, pool: &RRBPool<A>, index: usize) -> Option<&mut A> {
        if index >= self.len() {
            return None;
        }
        let phys_index = self.physical_index(index);
        if !contains(&self.target_range, &phys_index) {
            self.set_focus(pool, phys_index);
        }
        let target_phys_index = phys_index - self.target_range.start;
        Some(&mut self.get_focus()[target_phys_index])
    }

    pub fn get_chunk(&mut self, pool: &RRBPool<A>, index: usize) -> (Range<usize>, &mut [A]) {
        let phys_index = self.physical_index(index);
        if !contains(&self.target_range, &phys_index) {
            self.set_focus(pool, phys_index);
        }
        let mut left = 0;
        let mut right = 0;
        if self.target_range.start < self.view.start {
            left = self.view.start - self.target_range.start;
        }
        if self.target_range.end > self.view.end {
            right = self.target_range.end - self.view.end;
        }
        let phys_range = (self.target_range.start + left)..(self.target_range.end - right);
        let log_range = self.logical_range(&phys_range);
        let slice_len = self.get_focus().len();
        let slice = &mut (self.get_focus().as_mut_slice())[left..(slice_len - right)];
        (log_range, slice)
    }
}
