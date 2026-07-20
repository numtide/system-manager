// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! An unordered set.
//!
//! An immutable hash set using [hash array mapped tries] [1].
//!
//! Most operations on this set are O(log<sub>x</sub> n) for a
//! suitably high *x* that it should be nearly O(1) for most sets.
//! Because of this, it's a great choice for a generic set as long as
//! you don't mind that values will need to implement
//! [`Hash`][std::hash::Hash] and [`Eq`][std::cmp::Eq].
//!
//! Values will have a predictable order based on the hasher
//! being used. Unless otherwise specified, this will be the standard
//! [`RandomState`][std::collections::hash_map::RandomState] hasher.
//!
//! [1]: https://en.wikipedia.org/wiki/Hash_array_mapped_trie
//! [std::cmp::Eq]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
//! [std::hash::Hash]: https://doc.rust-lang.org/std/hash/trait.Hash.html
//! [std::collections::hash_map::RandomState]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::hash_map::RandomState;
use std::collections::{self, BTreeSet};
use std::fmt::{Debug, Error, Formatter};
use std::hash::{BuildHasher, Hash, Hasher};
use std::iter::FusedIterator;
use std::iter::{FromIterator, IntoIterator, Sum};
use std::ops::{Add, Deref, Mul};

use crate::nodes::hamt::{hash_key, Drain as NodeDrain, HashValue, Iter as NodeIter, Node};
use crate::ordset::OrdSet;
use crate::util::{Pool, PoolRef, Ref};
use crate::Vector;

/// Construct a set from a sequence of values.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate im;
/// # use im::hashset::HashSet;
/// # fn main() {
/// assert_eq!(
///   hashset![1, 2, 3],
///   HashSet::from(vec![1, 2, 3])
/// );
/// # }
/// ```
#[macro_export]
macro_rules! hashset {
    () => { $crate::hashset::HashSet::new() };

    ( $($x:expr),* ) => {{
        let mut l = $crate::hashset::HashSet::new();
        $(
            l.insert($x);
        )*
            l
    }};

    ( $($x:expr ,)* ) => {{
        let mut l = $crate::hashset::HashSet::new();
        $(
            l.insert($x);
        )*
            l
    }};
}

def_pool!(HashSetPool<A>, Node<Value<A>>);

/// An unordered set.
///
/// An immutable hash set using [hash array mapped tries] [1].
///
/// Most operations on this set are O(log<sub>x</sub> n) for a
/// suitably high *x* that it should be nearly O(1) for most sets.
/// Because of this, it's a great choice for a generic set as long as
/// you don't mind that values will need to implement
/// [`Hash`][std::hash::Hash] and [`Eq`][std::cmp::Eq].
///
/// Values will have a predictable order based on the hasher
/// being used. Unless otherwise specified, this will be the standard
/// [`RandomState`][std::collections::hash_map::RandomState] hasher.
///
/// [1]: https://en.wikipedia.org/wiki/Hash_array_mapped_trie
/// [std::cmp::Eq]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
/// [std::hash::Hash]: https://doc.rust-lang.org/std/hash/trait.Hash.html
/// [std::collections::hash_map::RandomState]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
pub struct HashSet<A, S = RandomState> {
    hasher: Ref<S>,
    pool: HashSetPool<A>,
    root: PoolRef<Node<Value<A>>>,
    size: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Value<A>(A);

impl<A> Deref for Value<A> {
    type Target = A;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// FIXME lacking specialisation, we can't simply implement `HashValue`
// for `A`, we have to use the `Value<A>` indirection.
impl<A> HashValue for Value<A>
where
    A: Hash + Eq,
{
    type Key = A;

    fn extract_key(&self) -> &Self::Key {
        &self.0
    }

    fn ptr_eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<A> HashSet<A, RandomState> {
    /// Construct an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct an empty set using a specific memory pool.
    #[cfg(feature = "pool")]
    #[must_use]
    pub fn with_pool(pool: &HashSetPool<A>) -> Self {
        Self {
            pool: pool.clone(),
            hasher: Default::default(),
            size: 0,
            root: PoolRef::default(&pool.0),
        }
    }
}

impl<A> HashSet<A, RandomState>
where
    A: Hash + Eq + Clone,
{
    /// Construct a set with a single value.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// # use std::sync::Arc;
    /// let set = HashSet::unit(123);
    /// assert!(set.contains(&123));
    /// ```
    #[inline]
    #[must_use]
    pub fn unit(a: A) -> Self {
        HashSet::new().update(a)
    }
}

impl<A, S> HashSet<A, S> {
    /// Test whether a set is empty.
    ///
    /// Time: O(1)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// assert!(
    ///   !hashset![1, 2, 3].is_empty()
    /// );
    /// assert!(
    ///   HashSet::<i32>::new().is_empty()
    /// );
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the size of a set.
    ///
    /// Time: O(1)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// assert_eq!(3, hashset![1, 2, 3].len());
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.size
    }

    /// Test whether two sets refer to the same content in memory.
    ///
    /// This is true if the two sides are references to the same set,
    /// or if the two sets refer to the same root node.
    ///
    /// This would return true if you're comparing a set to itself, or
    /// if you're comparing a set to a fresh clone of itself.
    ///
    /// Time: O(1)
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || PoolRef::ptr_eq(&self.root, &other.root)
    }

    /// Get a reference to the memory pool used by this set.
    ///
    /// Note that if you didn't specifically construct it with a pool, you'll
    /// get back a reference to a pool of size 0.
    #[cfg(feature = "pool")]
    pub fn pool(&self) -> &HashSetPool<A> {
        &self.pool
    }

    /// Construct an empty hash set using the provided hasher.
    #[inline]
    #[must_use]
    pub fn with_hasher<RS>(hasher: RS) -> Self
    where
        Ref<S>: From<RS>,
    {
        let pool = HashSetPool::default();
        let root = PoolRef::default(&pool.0);
        HashSet {
            size: 0,
            pool,
            root,
            hasher: From::from(hasher),
        }
    }

    /// Construct an empty hash set using the provided memory pool and hasher.
    #[cfg(feature = "pool")]
    #[inline]
    #[must_use]
    pub fn with_pool_hasher<RS>(pool: &HashSetPool<A>, hasher: RS) -> Self
    where
        Ref<S>: From<RS>,
    {
        let root = PoolRef::default(&pool.0);
        HashSet {
            size: 0,
            pool: pool.clone(),
            root,
            hasher: From::from(hasher),
        }
    }

    /// Get a reference to the set's [`BuildHasher`][BuildHasher].
    ///
    /// [BuildHasher]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    #[must_use]
    pub fn hasher(&self) -> &Ref<S> {
        &self.hasher
    }

    /// Construct an empty hash set using the same hasher as the current hash set.
    #[inline]
    #[must_use]
    pub fn new_from<A1>(&self) -> HashSet<A1, S>
    where
        A1: Hash + Eq + Clone,
    {
        let pool = HashSetPool::default();
        let root = PoolRef::default(&pool.0);
        HashSet {
            size: 0,
            pool,
            root,
            hasher: self.hasher.clone(),
        }
    }

    /// Discard all elements from the set.
    ///
    /// This leaves you with an empty set, and all elements that
    /// were previously inside it are dropped.
    ///
    /// Time: O(n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::HashSet;
    /// let mut set = hashset![1, 2, 3];
    /// set.clear();
    /// assert!(set.is_empty());
    /// ```
    pub fn clear(&mut self) {
        if !self.is_empty() {
            self.root = PoolRef::default(&self.pool.0);
            self.size = 0;
        }
    }

    /// Get an iterator over the values in a hash set.
    ///
    /// Please note that the order is consistent between sets using
    /// the same hasher, but no other ordering guarantee is offered.
    /// Items will not come out in insertion order or sort order.
    /// They will, however, come out in the same order every time for
    /// the same set.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, A> {
        Iter {
            it: NodeIter::new(&self.root, self.size),
        }
    }
}

impl<A, S> HashSet<A, S>
where
    A: Hash + Eq,
    S: BuildHasher,
{
    fn test_eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let mut seen = collections::HashSet::new();
        for value in self.iter() {
            if !other.contains(value) {
                return false;
            }
            seen.insert(value);
        }
        for value in other.iter() {
            if !seen.contains(&value) {
                return false;
            }
        }
        true
    }

    /// Test if a value is part of a set.
    ///
    /// Time: O(log n)
    #[must_use]
    pub fn contains<BA>(&self, a: &BA) -> bool
    where
        BA: Hash + Eq + ?Sized,
        A: Borrow<BA>,
    {
        self.root.get(hash_key(&*self.hasher, a), 0, a).is_some()
    }

    /// Test whether a set is a subset of another set, meaning that
    /// all values in our set must also be in the other set.
    ///
    /// Time: O(n log n)
    #[must_use]
    pub fn is_subset<RS>(&self, other: RS) -> bool
    where
        RS: Borrow<Self>,
    {
        let o = other.borrow();
        self.iter().all(|a| o.contains(a))
    }

    /// Test whether a set is a proper subset of another set, meaning
    /// that all values in our set must also be in the other set. A
    /// proper subset must also be smaller than the other set.
    ///
    /// Time: O(n log n)
    #[must_use]
    pub fn is_proper_subset<RS>(&self, other: RS) -> bool
    where
        RS: Borrow<Self>,
    {
        self.len() != other.borrow().len() && self.is_subset(other)
    }
}

impl<A, S> HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher,
{
    /// Insert a value into a set.
    ///
    /// Time: O(log n)
    #[inline]
    pub fn insert(&mut self, a: A) -> Option<A> {
        let hash = hash_key(&*self.hasher, &a);
        let root = PoolRef::make_mut(&self.pool.0, &mut self.root);
        match root.insert(&self.pool.0, hash, 0, Value(a)) {
            None => {
                self.size += 1;
                None
            }
            Some(Value(old_value)) => Some(old_value),
        }
    }

    /// Remove a value from a set if it exists.
    ///
    /// Time: O(log n)
    pub fn remove<BA>(&mut self, a: &BA) -> Option<A>
    where
        BA: Hash + Eq + ?Sized,
        A: Borrow<BA>,
    {
        let root = PoolRef::make_mut(&self.pool.0, &mut self.root);
        let result = root.remove(&self.pool.0, hash_key(&*self.hasher, a), 0, a);
        if result.is_some() {
            self.size -= 1;
        }
        result.map(|v| v.0)
    }

    /// Construct a new set from the current set with the given value
    /// added.
    ///
    /// Time: O(log n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// # use std::sync::Arc;
    /// let set = hashset![123];
    /// assert_eq!(
    ///   set.update(456),
    ///   hashset![123, 456]
    /// );
    /// ```
    #[must_use]
    pub fn update(&self, a: A) -> Self {
        let mut out = self.clone();
        out.insert(a);
        out
    }

    /// Construct a new set with the given value removed if it's in
    /// the set.
    ///
    /// Time: O(log n)
    #[must_use]
    pub fn without<BA>(&self, a: &BA) -> Self
    where
        BA: Hash + Eq + ?Sized,
        A: Borrow<BA>,
    {
        let mut out = self.clone();
        out.remove(a);
        out
    }

    /// Filter out values from a set which don't satisfy a predicate.
    ///
    /// This is slightly more efficient than filtering using an
    /// iterator, in that it doesn't need to rehash the retained
    /// values, but it still needs to reconstruct the entire tree
    /// structure of the set.
    ///
    /// Time: O(n log n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::HashSet;
    /// let mut set = hashset![1, 2, 3];
    /// set.retain(|v| *v > 1);
    /// let expected = hashset![2, 3];
    /// assert_eq!(expected, set);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&A) -> bool,
    {
        let old_root = self.root.clone();
        let root = PoolRef::make_mut(&self.pool.0, &mut self.root);
        for (value, hash) in NodeIter::new(&old_root, self.size) {
            if !f(value) && root.remove(&self.pool.0, hash, 0, value).is_some() {
                self.size -= 1;
            }
        }
    }

    /// Construct the union of two sets.
    ///
    /// Time: O(n log n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// let set1 = hashset!{1, 2};
    /// let set2 = hashset!{2, 3};
    /// let expected = hashset!{1, 2, 3};
    /// assert_eq!(expected, set1.union(set2));
    /// ```
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let (mut to_mutate, to_consume) = if self.len() >= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        for value in to_consume {
            to_mutate.insert(value);
        }
        to_mutate
    }

    /// Construct the union of multiple sets.
    ///
    /// Time: O(n log n)
    #[must_use]
    pub fn unions<I>(i: I) -> Self
    where
        I: IntoIterator<Item = Self>,
        S: Default,
    {
        i.into_iter().fold(Self::default(), Self::union)
    }

    /// Construct the symmetric difference between two sets.
    ///
    /// This is an alias for the
    /// [`symmetric_difference`][symmetric_difference] method.
    ///
    /// Time: O(n log n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// let set1 = hashset!{1, 2};
    /// let set2 = hashset!{2, 3};
    /// let expected = hashset!{1, 3};
    /// assert_eq!(expected, set1.difference(set2));
    /// ```
    ///
    /// [symmetric_difference]: #method.symmetric_difference
    #[must_use]
    pub fn difference(self, other: Self) -> Self {
        self.symmetric_difference(other)
    }

    /// Construct the symmetric difference between two sets.
    ///
    /// Time: O(n log n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// let set1 = hashset!{1, 2};
    /// let set2 = hashset!{2, 3};
    /// let expected = hashset!{1, 3};
    /// assert_eq!(expected, set1.symmetric_difference(set2));
    /// ```
    #[must_use]
    pub fn symmetric_difference(mut self, other: Self) -> Self {
        for value in other {
            if self.remove(&value).is_none() {
                self.insert(value);
            }
        }
        self
    }

    /// Construct the relative complement between two sets, that is the set
    /// of values in `self` that do not occur in `other`.
    ///
    /// Time: O(m log n) where m is the size of the other set
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::ordset::OrdSet;
    /// let set1 = ordset!{1, 2};
    /// let set2 = ordset!{2, 3};
    /// let expected = ordset!{1};
    /// assert_eq!(expected, set1.relative_complement(set2));
    /// ```
    #[must_use]
    pub fn relative_complement(mut self, other: Self) -> Self {
        for value in other {
            let _ = self.remove(&value);
        }
        self
    }

    /// Construct the intersection of two sets.
    ///
    /// Time: O(n log n)
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im;
    /// # use im::hashset::HashSet;
    /// let set1 = hashset!{1, 2};
    /// let set2 = hashset!{2, 3};
    /// let expected = hashset!{2};
    /// assert_eq!(expected, set1.intersection(set2));
    /// ```
    #[must_use]
    pub fn intersection(self, other: Self) -> Self {
        let mut out = self.new_from();
        for value in other {
            if self.contains(&value) {
                out.insert(value);
            }
        }
        out
    }
}

// Core traits

impl<A, S> Clone for HashSet<A, S>
where
    A: Clone,
{
    /// Clone a set.
    ///
    /// Time: O(1)
    #[inline]
    fn clone(&self) -> Self {
        HashSet {
            hasher: self.hasher.clone(),
            pool: self.pool.clone(),
            root: self.root.clone(),
            size: self.size,
        }
    }
}

impl<A, S> PartialEq for HashSet<A, S>
where
    A: Hash + Eq,
    S: BuildHasher + Default,
{
    fn eq(&self, other: &Self) -> bool {
        self.test_eq(other)
    }
}

impl<A, S> Eq for HashSet<A, S>
where
    A: Hash + Eq,
    S: BuildHasher + Default,
{
}

impl<A, S> PartialOrd for HashSet<A, S>
where
    A: Hash + Eq + Clone + PartialOrd,
    S: BuildHasher + Default,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if Ref::ptr_eq(&self.hasher, &other.hasher) {
            return self.iter().partial_cmp(other.iter());
        }
        self.iter().partial_cmp(other.iter())
    }
}

impl<A, S> Ord for HashSet<A, S>
where
    A: Hash + Eq + Clone + Ord,
    S: BuildHasher + Default,
{
    fn cmp(&self, other: &Self) -> Ordering {
        if Ref::ptr_eq(&self.hasher, &other.hasher) {
            return self.iter().cmp(other.iter());
        }
        self.iter().cmp(other.iter())
    }
}

impl<A, S> Hash for HashSet<A, S>
where
    A: Hash + Eq,
    S: BuildHasher + Default,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        for i in self.iter() {
            i.hash(state);
        }
    }
}

impl<A, S> Default for HashSet<A, S>
where
    S: BuildHasher + Default,
{
    fn default() -> Self {
        let pool = HashSetPool::default();
        let root = PoolRef::default(&pool.0);
        HashSet {
            hasher: Ref::<S>::default(),
            pool,
            root,
            size: 0,
        }
    }
}

impl<A, S> Add for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher,
{
    type Output = HashSet<A, S>;

    fn add(self, other: Self) -> Self::Output {
        self.union(other)
    }
}

impl<A, S> Mul for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher,
{
    type Output = HashSet<A, S>;

    fn mul(self, other: Self) -> Self::Output {
        self.intersection(other)
    }
}

impl<'a, A, S> Add for &'a HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher,
{
    type Output = HashSet<A, S>;

    fn add(self, other: Self) -> Self::Output {
        self.clone().union(other.clone())
    }
}

impl<'a, A, S> Mul for &'a HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher,
{
    type Output = HashSet<A, S>;

    fn mul(self, other: Self) -> Self::Output {
        self.clone().intersection(other.clone())
    }
}

impl<A, S> Sum for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn sum<I>(it: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        it.fold(Self::default(), |a, b| a + b)
    }
}

impl<A, S, R> Extend<R> for HashSet<A, S>
where
    A: Hash + Eq + Clone + From<R>,
    S: BuildHasher,
{
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = R>,
    {
        for value in iter {
            self.insert(From::from(value));
        }
    }
}

#[cfg(not(has_specialisation))]
impl<A, S> Debug for HashSet<A, S>
where
    A: Hash + Eq + Debug,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(has_specialisation)]
impl<A, S> Debug for HashSet<A, S>
where
    A: Hash + Eq + Debug,
    S: BuildHasher,
{
    default fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(has_specialisation)]
impl<A, S> Debug for HashSet<A, S>
where
    A: Hash + Eq + Debug + Ord,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_set().entries(self.iter()).finish()
    }
}

// Iterators

/// An iterator over the elements of a set.
pub struct Iter<'a, A> {
    it: NodeIter<'a, Value<A>>,
}

impl<'a, A> Iterator for Iter<'a, A>
where
    A: 'a,
{
    type Item = &'a A;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|(v, _)| &v.0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<'a, A> ExactSizeIterator for Iter<'a, A> {}

impl<'a, A> FusedIterator for Iter<'a, A> {}

/// A consuming iterator over the elements of a set.
pub struct ConsumingIter<A>
where
    A: Hash + Eq + Clone,
{
    it: NodeDrain<Value<A>>,
}

impl<A> Iterator for ConsumingIter<A>
where
    A: Hash + Eq + Clone,
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|(v, _)| v.0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<A> ExactSizeIterator for ConsumingIter<A> where A: Hash + Eq + Clone {}

impl<A> FusedIterator for ConsumingIter<A> where A: Hash + Eq + Clone {}

// Iterator conversions

impl<A, RA, S> FromIterator<RA> for HashSet<A, S>
where
    A: Hash + Eq + Clone + From<RA>,
    S: BuildHasher + Default,
{
    fn from_iter<T>(i: T) -> Self
    where
        T: IntoIterator<Item = RA>,
    {
        let mut set = Self::default();
        for value in i {
            set.insert(From::from(value));
        }
        set
    }
}

impl<'a, A, S> IntoIterator for &'a HashSet<A, S>
where
    A: Hash + Eq,
    S: BuildHasher,
{
    type Item = &'a A;
    type IntoIter = Iter<'a, A>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<A, S> IntoIterator for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher,
{
    type Item = A;
    type IntoIter = ConsumingIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        ConsumingIter {
            it: NodeDrain::new(&self.pool.0, self.root, self.size),
        }
    }
}

// Conversions

impl<'s, 'a, A, OA, SA, SB> From<&'s HashSet<&'a A, SA>> for HashSet<OA, SB>
where
    A: ToOwned<Owned = OA> + Hash + Eq + ?Sized,
    OA: Borrow<A> + Hash + Eq + Clone,
    SA: BuildHasher,
    SB: BuildHasher + Default,
{
    fn from(set: &HashSet<&A, SA>) -> Self {
        set.iter().map(|a| (*a).to_owned()).collect()
    }
}

impl<'a, A, S> From<&'a [A]> for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(slice: &'a [A]) -> Self {
        slice.iter().cloned().collect()
    }
}

impl<A, S> From<Vec<A>> for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(vec: Vec<A>) -> Self {
        vec.into_iter().collect()
    }
}

impl<'a, A, S> From<&'a Vec<A>> for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(vec: &Vec<A>) -> Self {
        vec.iter().cloned().collect()
    }
}

impl<A, S> From<Vector<A>> for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(vector: Vector<A>) -> Self {
        vector.into_iter().collect()
    }
}

impl<'a, A, S> From<&'a Vector<A>> for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(vector: &Vector<A>) -> Self {
        vector.iter().cloned().collect()
    }
}

impl<A, S> From<collections::HashSet<A>> for HashSet<A, S>
where
    A: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    fn from(hash_set: collections::HashSet<A>) -> Self {
        hash_set.into_iter().collect()
    }
}

impl<'a, A, S> From<&'a collections::HashSet<A>> for HashSet<A, S>
where
    A: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    fn from(hash_set: &collections::HashSet<A>) -> Self {
        hash_set.iter().cloned().collect()
    }
}

impl<'a, A, S> From<&'a BTreeSet<A>> for HashSet<A, S>
where
    A: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(btree_set: &BTreeSet<A>) -> Self {
        btree_set.iter().cloned().collect()
    }
}

impl<A, S> From<OrdSet<A>> for HashSet<A, S>
where
    A: Ord + Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(ordset: OrdSet<A>) -> Self {
        ordset.into_iter().collect()
    }
}

impl<'a, A, S> From<&'a OrdSet<A>> for HashSet<A, S>
where
    A: Ord + Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn from(ordset: &OrdSet<A>) -> Self {
        ordset.into_iter().cloned().collect()
    }
}

// Proptest
#[cfg(any(test, feature = "proptest"))]
#[doc(hidden)]
pub mod proptest {
    #[deprecated(
        since = "14.3.0",
        note = "proptest strategies have moved to im::proptest"
    )]
    pub use crate::proptest::hash_set;
}

#[cfg(test)]
mod test {
    use super::proptest::*;
    use super::*;
    use crate::test::LolHasher;
    use ::proptest::num::i16;
    use ::proptest::proptest;
    use std::hash::BuildHasherDefault;

    #[test]
    fn insert_failing() {
        let mut set: HashSet<i16, BuildHasherDefault<LolHasher>> = Default::default();
        set.insert(14658);
        assert_eq!(1, set.len());
        set.insert(-19198);
        assert_eq!(2, set.len());
    }

    #[test]
    fn match_strings_with_string_slices() {
        let mut set: HashSet<String> = From::from(&hashset!["foo", "bar"]);
        set = set.without("bar");
        assert!(!set.contains("bar"));
        set.remove("foo");
        assert!(!set.contains("foo"));
    }

    #[test]
    fn macro_allows_trailing_comma() {
        let set1 = hashset! {"foo", "bar"};
        let set2 = hashset! {
            "foo",
            "bar",
        };
        assert_eq!(set1, set2);
    }

    #[test]
    fn issue_60_drain_iterator_memory_corruption() {
        use crate::test::MetroHashBuilder;
        for i in 0..1000 {
            let mut lhs = vec![0, 1, 2];
            lhs.sort_unstable();

            let hasher = Ref::from(MetroHashBuilder::new(i));
            let mut iset: HashSet<_, MetroHashBuilder> = HashSet::with_hasher(hasher.clone());
            for &i in &lhs {
                iset.insert(i);
            }

            let mut rhs: Vec<_> = iset.clone().into_iter().collect();
            rhs.sort_unstable();

            if lhs != rhs {
                println!("iteration: {}", i);
                println!("seed: {}", hasher.seed());
                println!("lhs: {}: {:?}", lhs.len(), &lhs);
                println!("rhs: {}: {:?}", rhs.len(), &rhs);
                panic!();
            }
        }
    }

    proptest! {
        #[test]
        fn proptest_a_set(ref s in hash_set(".*", 10..100)) {
            assert!(s.len() < 100);
            assert!(s.len() >= 10);
        }
    }
}
