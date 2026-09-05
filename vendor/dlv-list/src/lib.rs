//! Crate that implements a semi-doubly linked list via a vector.
//!
//! See [`VecList`] for more information.
//!
//! # Features
//!
//! By default, this crate uses the Rust standard library. To disable this, disable the default
//! `no_std` feature. Without this feature, certain methods will not be available.

#![allow(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

use alloc::{collections::LinkedList, vec::Vec};
use core::{
  cmp::Ordering,
  fmt::{self, Debug, Formatter},
  hash::{Hash, Hasher},
  hint::unreachable_unchecked,
  iter::{FromIterator, FusedIterator},
  marker::PhantomData,
  mem,
  num::NonZeroUsize,
  ops,
};

#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(feature = "serde")]
mod serde;

/// Number type that's capable of representing [0, usize::MAX - 1]
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct NonMaxUsize(NonZeroUsize);

impl Debug for NonMaxUsize {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.get())
  }
}

impl NonMaxUsize {
  /// Convert an index to a usize
  #[cfg_attr(mutants, mutants::skip)]
  #[inline]
  const fn get(&self) -> usize {
    self.0.get() - 1
  }

  /// Create a new index from a usize, if `index` is `usize::MAX` then `None` is returned
  #[inline]
  const fn new(index: usize) -> Option<Self> {
    match NonZeroUsize::new(index.wrapping_add(1)) {
      Some(index) => Some(Self(index)),
      None => None,
    }
  }

  /// Create a new index from a usize, without checking if `index` is `usize::MAX`.
  ///
  /// # Safety
  ///
  /// `index` must not be `usize::MAX`
  #[cfg(feature = "std")]
  #[inline]
  const unsafe fn new_unchecked(index: usize) -> Self {
    Self(unsafe { NonZeroUsize::new_unchecked(index + 1) })
  }

  /// Add an unsigned integer to a index. Check for bound violation and return `None` if the result will be larger than or equal to `usize::MAX`
  #[cfg(feature = "std")]
  #[inline]
  fn checked_add(&self, rhs: usize) -> Option<Self> {
    self.0.checked_add(rhs).map(Self)
  }

  /// Subtract an unsigned integer from a index. Check for bound violation and return `None` if the result will be less than 0.
  #[cfg(feature = "std")]
  #[inline]
  fn checked_sub(&self, rhs: usize) -> Option<Self> {
    // Safety: `self` is less than `usize::MAX`, so `self - rhs` can only be less than `usize::MAX`
    self
      .get()
      .checked_sub(rhs)
      .map(|i| unsafe { Self::new_unchecked(i) })
  }

  #[cfg(feature = "std")]
  #[inline]
  const fn zero() -> Self {
    Self(unsafe { NonZeroUsize::new_unchecked(1) })
  }
}

impl PartialEq<usize> for NonMaxUsize {
  fn eq(&self, other: &usize) -> bool {
    self.get() == *other
  }
}

impl PartialOrd<usize> for NonMaxUsize {
  fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
    self.get().partial_cmp(other)
  }
}

/// A semi-doubly linked list implemented with a vector.
///
/// This provides many of the benefits of an actual linked list with a few tradeoffs. First, due to the use of an
/// underlying vector, an individual insert operation may be O(n) due to allocating more space for the vector. However,
/// it is amortized O(1) and it avoids the frequent allocations that traditional linked lists suffer from.
///
/// Another tradeoff is that extending a traditional linked list with another list is O(1) but a vector based
/// implementation is O(n). Splicing has a similar disadvantage.
///
/// Lastly, the vector based implementation is likely to have better cache locality in general.
pub struct VecList<T> {
  /// The backing storage for the list. This includes both used and unused indices.
  entries: Vec<Entry<T>>,

  /// The current generation of the list. This is used to avoid the ABA problem.
  generation: u64,

  /// The index of the head of the list.
  head: Option<NonMaxUsize>,

  /// The length of the list since we cannot rely on the length of [`VecList::entries`] because it includes unused
  /// indices.
  length: usize,

  /// The index of the tail of the list.
  tail: Option<NonMaxUsize>,

  /// The index of the head of the vacant indices.
  vacant_head: Option<NonMaxUsize>,
}

impl<T: Clone> Clone for VecList<T> {
  fn clone(&self) -> Self {
    Self {
      entries: self.entries.clone(),
      generation: self.generation,
      head: self.head,
      length: self.length,
      tail: self.tail,
      vacant_head: self.vacant_head,
    }
  }

  fn clone_from(&mut self, source: &Self) {
    self.entries.clone_from(&source.entries);
    self.generation = source.generation;
    self.head = source.head;
    self.length = source.length;
    self.tail = source.tail;
    self.vacant_head = source.vacant_head;
  }
}

impl<T> VecList<T> {
  /// Returns an immutable reference to the value at the back of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.back(), None);
  ///
  /// list.push_back(0);
  /// list.push_back(5);
  /// assert_eq!(list.back(), Some(&5));
  /// ```
  #[must_use]
  pub fn back(&self) -> Option<&T> {
    let index = self.tail?.get();

    match &self.entries[index] {
      Entry::Occupied(entry) => Some(&entry.value),
      _ => None,
    }
  }

  /// Returns the index of the value at the back of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.back_index(), None);
  ///
  /// list.push_back(0);
  /// let index = list.push_back(5);
  /// assert_eq!(list.back_index(), Some(index));
  /// ```
  #[must_use]
  pub fn back_index(&self) -> Option<Index<T>> {
    let index = self.tail?;
    let entry = self.entries[index.get()].occupied_ref();
    let index = Index::new(index, entry.generation);
    Some(index)
  }

  /// Returns a mutable reference to the value at the back of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.back_mut(), None);
  ///
  /// list.push_back(0);
  /// list.push_back(5);
  ///
  /// let mut back = list.back_mut().unwrap();
  /// assert_eq!(back, &mut 5);
  /// *back *= 2;
  ///
  /// assert_eq!(list.back(), Some(&10));
  /// ```
  #[must_use]
  pub fn back_mut(&mut self) -> Option<&mut T> {
    let index = self.tail?.get();

    match &mut self.entries[index] {
      Entry::Occupied(entry) => Some(&mut entry.value),
      _ => None,
    }
  }

  /// Returns the capacity of the list.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let list: VecList<u32> = VecList::new();
  /// assert_eq!(list.capacity(), 0);
  ///
  /// let list: VecList<u32> = VecList::with_capacity(10);
  /// assert_eq!(list.capacity(), 10);
  /// ```
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.entries.capacity()
  }

  /// Removes all values from the list and invalidates all existing indices.
  ///
  /// Complexity: O(n)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  ///
  /// list.push_back(5);
  /// assert!(!list.is_empty());
  ///
  /// list.clear();
  /// assert!(list.is_empty());
  /// ```
  pub fn clear(&mut self) {
    self.entries.clear();
    self.generation = self.generation.wrapping_add(1);
    self.head = None;
    self.length = 0;
    self.tail = None;
    self.vacant_head = None;
  }

  /// Returns whether or not the list contains the given value.
  ///
  /// Complexity: O(n)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert!(!list.contains(&0));
  ///
  /// list.push_back(0);
  /// assert!(list.contains(&0));
  /// ```
  #[must_use]
  pub fn contains(&self, value: &T) -> bool
  where
    T: PartialEq,
  {
    self.iter().any(|entry| entry == value)
  }

  /// Creates a draining iterator that removes all values from the list and yields them in order.
  ///
  /// All values are removed even if the iterator is only partially consumed or not consumed at all.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_back(0);
  /// list.push_back(5);
  ///
  /// {
  ///     let mut iter = list.drain();
  ///     assert_eq!(iter.next(), Some(0));
  ///     assert_eq!(iter.next(), Some(5));
  ///     assert_eq!(iter.next(), None);
  /// }
  ///
  /// println!("{}", list.len());
  /// assert!(list.is_empty());
  /// ```
  pub fn drain(&mut self) -> Drain<'_, T> {
    Drain {
      head: self.head,
      remaining: self.length,
      tail: self.tail,
      list: self,
    }
  }

  /// Returns an immutable reference to the value at the front of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.front(), None);
  ///
  /// list.push_front(0);
  /// list.push_front(5);
  /// assert_eq!(list.front(), Some(&5));
  /// ```
  #[must_use]
  pub fn front(&self) -> Option<&T> {
    let index = self.head?.get();

    match &self.entries[index] {
      Entry::Occupied(entry) => Some(&entry.value),
      _ => None,
    }
  }

  /// Returns the index of the value at the front of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.front_index(), None);
  ///
  /// list.push_front(0);
  /// let index = list.push_front(5);
  /// assert_eq!(list.front_index(), Some(index));
  /// ```
  #[must_use]
  pub fn front_index(&self) -> Option<Index<T>> {
    let index = self.head?;
    let entry = self.entries[index.get()].occupied_ref();
    let index = Index::new(index, entry.generation);
    Some(index)
  }

  /// Returns a mutable reference to the value at the front of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.front_mut(), None);
  ///
  /// list.push_front(0);
  /// list.push_front(5);
  ///
  /// let mut front = list.front_mut().unwrap();
  /// assert_eq!(front, &mut 5);
  /// *front *= 2;
  ///
  /// assert_eq!(list.front(), Some(&10));
  /// ```
  #[must_use]
  pub fn front_mut(&mut self) -> Option<&mut T> {
    let index = self.head?.get();

    match &mut self.entries[index] {
      Entry::Occupied(entry) => Some(&mut entry.value),
      _ => None,
    }
  }

  /// Returns an immutable reference to the value at the given index.
  ///
  /// If the index refers to an index not in the list anymore or if the index has been invalidated, then [`None`] will
  /// be returned.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index = list.push_front(0);
  /// assert_eq!(list.get(index), Some(&0));
  ///
  /// let index = list.push_front(5);
  /// assert_eq!(list.get(index), Some(&5));
  /// ```
  #[must_use]
  pub fn get(&self, index: Index<T>) -> Option<&T> {
    match self.entries.get(index.index())? {
      Entry::Occupied(entry) if entry.generation == index.generation => Some(&entry.value),
      _ => None,
    }
  }

  /// Returns an immutable reference to the value at the given index.
  ///
  /// Complexity: O(1)
  ///
  /// # Safety
  ///
  /// Caller needs to guarantee that the index is in bound, and has never been removed from the
  /// list. This function does not perform generation checks. So if an element is removed then a
  /// new element is added at the same index, then the returned reference will be to the new
  /// element.
  #[must_use]
  pub unsafe fn get_unchecked(&self, index: Index<T>) -> &T {
    match unsafe { self.entries.get_unchecked(index.index()) } {
      Entry::Occupied(entry) => &entry.value,
      _ => unsafe { unreachable_unchecked() },
    }
  }

  /// Returns a mutable reference to the value at the given index.
  ///
  /// If the index refers to an index not in the list anymore or if the index has been invalidated, then [`None`] will
  /// be returned.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index = list.push_front(0);
  /// let value = list.get_mut(index).unwrap();
  /// *value = 100;
  /// assert_eq!(list.get(index), Some(&100));
  /// ```
  #[must_use]
  pub fn get_mut(&mut self, index: Index<T>) -> Option<&mut T> {
    match self.entries.get_mut(index.index())? {
      Entry::Occupied(entry) if entry.generation == index.generation => Some(&mut entry.value),
      _ => None,
    }
  }

  /// Returns an mutable reference to the value at the given index.
  ///
  /// # Safety
  ///
  /// Caller needs to guarantee that the index is in bound, and has never been removed from the list.
  /// See also: [`VecList::get_unchecked`].
  ///
  /// Complexity: O(1)
  #[must_use]
  pub unsafe fn get_unchecked_mut(&mut self, index: Index<T>) -> &mut T {
    match unsafe { self.entries.get_unchecked_mut(index.index()) } {
      Entry::Occupied(entry) => &mut entry.value,
      _ => unsafe { unreachable_unchecked() },
    }
  }

  /// Returns the index of the value next to the value at the given index.
  ///
  /// If the index refers to an index not in the list anymore or if the index has been invalidated, then [`None`] will
  /// be returned.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  ///
  /// let index_1 = list.push_back(0);
  /// assert_eq!(list.get_next_index(index_1), None);
  ///
  /// let index_2 = list.push_back(5);
  /// assert_eq!(list.get_next_index(index_1), Some(index_2));
  /// ```
  #[must_use]
  pub fn get_next_index(&self, index: Index<T>) -> Option<Index<T>> {
    match self.entries.get(index.index())? {
      Entry::Occupied(entry) if entry.generation == index.generation => {
        let next_index = entry.next?;
        let next_entry = self.entries[next_index.get()].occupied_ref();
        Some(Index::new(next_index, next_entry.generation))
      }
      _ => None,
    }
  }

  /// Returns the index of the value previous to the value at the given index.
  ///
  /// If the index refers to an index not in the list anymore or if the index has been invalidated, then [`None`] will
  /// be returned.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  ///
  /// let index_1 = list.push_front(0);
  /// assert_eq!(list.get_previous_index(index_1), None);
  ///
  /// let index_2 = list.push_front(5);
  /// assert_eq!(list.get_previous_index(index_1), Some(index_2));
  /// ```
  #[must_use]
  pub fn get_previous_index(&self, index: Index<T>) -> Option<Index<T>> {
    match self.entries.get(index.index())? {
      Entry::Occupied(entry) if entry.generation == index.generation => {
        let previous_index = entry.previous?;
        let previous_entry = self.entries[previous_index.get()].occupied_ref();
        Some(Index::new(previous_index, previous_entry.generation))
      }
      _ => None,
    }
  }

  /// Connect the node at `index` to the node at `next`. If `index` is `None`, then the head will be
  /// set to `next`; if `next` is `None`, then the tail will be set to `index`.
  #[inline]
  fn update_link(&mut self, index: Option<NonMaxUsize>, next: Option<NonMaxUsize>) {
    if let Some(index) = index {
      let entry = self.entries[index.get()].occupied_mut();
      entry.next = next;
    } else {
      self.head = next
    }
    if let Some(next) = next {
      let entry = self.entries[next.get()].occupied_mut();
      entry.previous = index;
    } else {
      self.tail = index;
    }
  }

  /// Move the node at `index` to after the node at `target`.
  ///
  /// # Panics
  ///
  /// Panics if either `index` or `target` is invalidated. Also panics if `index` is the same as `target`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index_1 = list.push_back(0);
  /// let index_2 = list.push_back(1);
  /// let index_3 = list.push_back(2);
  /// let index_4 = list.push_back(3);
  ///
  /// list.move_after(index_1, index_3);
  /// assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 0, 3]);
  /// assert_eq!(list.iter().rev().copied().collect::<Vec<_>>(), vec![3, 0, 2, 1]);
  /// ```
  pub fn move_after(&mut self, index: Index<T>, target: Index<T>) {
    let (previous_index, next_index) = match &self.entries[index.index()] {
      Entry::Occupied(entry) if entry.generation == index.generation => {
        (entry.previous, entry.next)
      }
      _ => panic!("expected occupied entry with correct generation at `index`"),
    };
    let target_next_index = match &self.entries[target.index()] {
      Entry::Occupied(entry) if entry.generation == target.generation => entry.next,
      _ => panic!("expected occupied entry with correct generation at `target`"),
    };
    if target.index == index.index {
      panic!("cannot move after `index` itself");
    }
    if previous_index == Some(target.index) {
      // Already in the right place
      return;
    }
    self.update_link(previous_index, next_index);
    self.update_link(Some(target.index), Some(index.index));
    self.update_link(Some(index.index), target_next_index);
  }

  /// Move the node at `index` to before the node at `target`.
  ///
  /// # Panics
  ///
  /// Panics if either `index` or `target` is invalidated. Also panics if `index` is the same as `target`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index_1 = list.push_back(0);
  /// let index_2 = list.push_back(1);
  /// let index_3 = list.push_back(2);
  /// let index_4 = list.push_back(3);
  ///
  /// list.move_before(index_1, index_3);
  /// assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 0, 2, 3]);
  /// assert_eq!(list.iter().rev().copied().collect::<Vec<_>>(), vec![3, 2, 0, 1]);
  /// ```
  pub fn move_before(&mut self, index: Index<T>, target: Index<T>) {
    let (previous_index, next_index) = match &self.entries[index.index()] {
      Entry::Occupied(entry) if entry.generation == index.generation => {
        (entry.previous, entry.next)
      }
      _ => panic!("expected occupied entry with correct generation at `index`"),
    };
    let target_previous_index = match &self.entries[target.index()] {
      Entry::Occupied(entry) if entry.generation == target.generation => entry.previous,
      _ => panic!("expected occupied entry with correct generation at `target`"),
    };
    if target.index == index.index {
      panic!("cannot move before `index` itself");
    }
    if next_index == Some(target.index) {
      // Already in the right place
      return;
    }
    self.update_link(previous_index, next_index);
    self.update_link(Some(index.index), Some(target.index));
    self.update_link(target_previous_index, Some(index.index));
  }

  /// Creates an indices iterator which will yield all indices of the list in order.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_front(0);
  /// list.push_front(5);
  ///
  /// let mut indices = list.indices();
  /// let index = indices.next().unwrap();
  /// assert_eq!(list.get(index), Some(&5));
  ///
  /// let index = indices.next().unwrap();
  /// assert_eq!(list.get(index), Some(&0));
  ///
  /// assert_eq!(indices.next(), None);
  /// ```
  #[must_use]
  pub fn indices(&self) -> Indices<'_, T> {
    Indices {
      entries: &self.entries,
      head: self.head,
      remaining: self.length,
      tail: self.tail,
    }
  }

  /// Inserts the given value after the value at the given index.
  ///
  /// The index of the newly inserted value will be returned.
  ///
  /// Complexity: amortized O(1)
  ///
  /// # Panics
  ///
  /// Panics if the index refers to an index not in the list anymore or if the index has been invalidated. This is
  /// enforced because this function will consume the value to be inserted, and if it cannot be inserted (due to the
  /// index not being valid), then it will be lost.
  ///
  /// Also panics if the new capacity overflows `usize`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_front(0);
  /// let index_1 = list.push_front(5);
  /// list.push_front(10);
  ///
  /// let index_2 = list.insert_after(index_1, 1000);
  /// assert_eq!(list.get_next_index(index_1), Some(index_2));
  /// ```
  pub fn insert_after(&mut self, index: Index<T>, value: T) -> Index<T> {
    let next_index = match &mut self.entries[index.index()] {
      Entry::Occupied(entry) if entry.generation == index.generation => entry.next,
      _ => panic!("expected occupied entry with correct generation"),
    };
    let new_index = self.insert_new(value, Some(index.index), next_index);
    let entry = self.entries[index.index()].occupied_mut();
    entry.next = Some(new_index);

    if Some(index.index) == self.tail {
      self.tail = Some(new_index);
    }

    if let Some(next_index) = next_index {
      self.entries[next_index.get()].occupied_mut().previous = Some(new_index);
    }

    Index::new(new_index, self.generation)
  }

  /// Inserts the given value before the value at the given index.
  ///
  /// The index of the newly inserted value will be returned.
  ///
  /// Complexity: amortized O(1)
  ///
  /// # Panics
  ///
  /// Panics if the index refers to an index not in the list anymore or if the index has been invalidated. This is
  /// enforced because this function will consume the value to be inserted, and if it cannot be inserted (due to the
  /// index not being valid), then it will be lost.
  ///
  /// Also panics if the new capacity overflows `usize`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_front(0);
  /// let index_1 = list.push_front(5);
  /// list.push_front(10);
  ///
  /// let index_2 = list.insert_before(index_1, 1000);
  /// assert_eq!(list.get_previous_index(index_1), Some(index_2));
  /// ```
  pub fn insert_before(&mut self, index: Index<T>, value: T) -> Index<T> {
    let previous_index = match &mut self.entries[index.index()] {
      Entry::Occupied(entry) if entry.generation == index.generation => entry.previous,
      _ => panic!("expected occupied entry with correct generation"),
    };
    let new_index = self.insert_new(value, previous_index, Some(index.index));
    let entry = self.entries[index.index()].occupied_mut();
    entry.previous = Some(new_index);

    if Some(index.index) == self.head {
      self.head = Some(new_index);
    }

    if let Some(previous_index) = previous_index {
      self.entries[previous_index.get()].occupied_mut().next = Some(new_index);
    }

    Index::new(new_index, self.generation)
  }

  /// Inserts the given value into the list with the assumption that it is currently empty.
  ///
  /// # Panics
  ///
  /// Panics if the new capacity overflows `usize`.
  fn insert_empty(&mut self, value: T) -> Index<T> {
    let generation = self.generation;
    let index = self.insert_new(value, None, None);
    self.head = Some(index);
    self.tail = Some(index);
    Index::new(index, generation)
  }

  /// Inserts the given value into the list with its expected previous and next value indices.
  ///
  /// # Panics
  ///
  /// Panics if the new capacity overflows `usize`.
  fn insert_new(
    &mut self,
    value: T,
    previous: Option<NonMaxUsize>,
    next: Option<NonMaxUsize>,
  ) -> NonMaxUsize {
    self.length += 1;

    if self.length == usize::max_value() {
      panic!("reached maximum possible length");
    }

    match self.vacant_head {
      Some(index) => {
        self.vacant_head = self.entries[index.get()].vacant_ref().next;
        self.entries[index.get()] =
          Entry::Occupied(OccupiedEntry::new(self.generation, previous, next, value));
        index
      }
      None => {
        self.entries.push(Entry::Occupied(OccupiedEntry::new(
          self.generation,
          previous,
          next,
          value,
        )));
        NonMaxUsize::new(self.entries.len() - 1).unwrap()
      }
    }
  }

  /// Returns whether or not the list is empty.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert!(list.is_empty());
  ///
  /// list.push_back(0);
  /// assert!(!list.is_empty());
  /// ```
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.length == 0
  }

  /// Creates an iterator that yields immutable references to values in the list in order.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_back(0);
  /// list.push_back(10);
  /// list.push_back(200);
  /// list.push_back(-10);
  ///
  /// let mut iter = list.iter();
  /// assert_eq!(iter.next(), Some(&0));
  /// assert_eq!(iter.next(), Some(&10));
  /// assert_eq!(iter.next(), Some(&200));
  /// assert_eq!(iter.next(), Some(&-10));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn iter(&self) -> Iter<'_, T> {
    Iter {
      entries: &self.entries,
      head: self.head,
      remaining: self.length,
      tail: self.tail,
    }
  }

  /// Creates an iterator that yields mutable references to values in the list in order.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_back(0);
  /// list.push_back(10);
  /// list.push_back(200);
  /// list.push_back(-10);
  ///
  /// let mut iter = list.iter_mut();
  /// assert_eq!(iter.next(), Some(&mut 0));
  /// assert_eq!(iter.next(), Some(&mut 10));
  /// assert_eq!(iter.next(), Some(&mut 200));
  /// assert_eq!(iter.next(), Some(&mut -10));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn iter_mut(&mut self) -> IterMut<'_, T> {
    IterMut {
      entries: &mut self.entries,
      head: self.head,
      phantom: PhantomData,
      remaining: self.length,
      tail: self.tail,
    }
  }

  /// Returns the number of values in the list.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.len(), 0);
  ///
  /// list.push_back(0);
  /// list.push_back(1);
  /// list.push_back(2);
  /// assert_eq!(list.len(), 3);
  /// ```
  #[must_use]
  pub fn len(&self) -> usize {
    self.length
  }

  /// Creates a new list with no initial capacity.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index = list.push_back(0);
  /// assert_eq!(list.get(index), Some(&0));
  /// ```
  #[must_use]
  pub fn new() -> Self {
    VecList::default()
  }

  /// Reorganizes the existing values to ensure maximum cache locality and shrinks the list such that the capacity is
  /// exactly [`minimum_capacity`].
  ///
  /// This function can be used to actually increase the capacity of the list.
  ///
  /// Complexity: O(n)
  ///
  /// # Panics
  ///
  /// Panics if the given minimum capacity is less than the current length of the list.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index_1 = list.push_back(5);
  /// let index_2 = list.push_back(10);
  /// let index_3 = list.push_front(100);
  /// list.remove(index_1);
  ///
  /// assert!(list.capacity() >= 3);
  ///
  /// let mut map = list.pack_to(list.len() + 5);
  /// assert_eq!(list.capacity(), 7);
  /// assert_eq!(map.len(), 2);
  ///
  /// let index_2 = map.remove(&index_2).unwrap();
  /// let index_3 = map.remove(&index_3).unwrap();
  ///
  /// assert_eq!(list.get(index_2), Some(&10));
  /// assert_eq!(list.get(index_3), Some(&100));
  ///
  /// let mut iter = list.iter();
  /// assert_eq!(iter.next(), Some(&100));
  /// assert_eq!(iter.next(), Some(&10));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[cfg(feature = "std")]
  pub fn pack_to(&mut self, minimum_capacity: usize) -> HashMap<Index<T>, Index<T>> {
    assert!(
      minimum_capacity >= self.length,
      "cannot shrink to capacity lower than current length"
    );

    let mut count = NonMaxUsize::zero();
    let mut entries = Vec::with_capacity(minimum_capacity);
    let generation = create_initial_generation();
    let length = self.length;
    let mut map = HashMap::with_capacity(length);
    let mut next_index = self.head;

    while let Some(index) = next_index {
      let mut entry = self.remove_entry(index).expect("expected occupied entry");
      next_index = entry.next;

      let _ = map.insert(
        Index::new(index, entry.generation),
        Index::new(count, generation),
      );

      entry.generation = generation;
      entry.previous = if count > 0 {
        Some(count.checked_sub(1).unwrap())
      } else {
        None
      };
      entry.next = if count < length - 1 {
        Some(count.checked_add(1).expect("overflow"))
      } else {
        None
      };

      entries.push(Entry::Occupied(entry));
      count = count.checked_add(1).expect("overflow");
    }

    self.entries = entries;
    self.generation = generation;
    self.length = length;
    self.vacant_head = None;

    if self.length > 0 {
      self.head = Some(NonMaxUsize::zero());
      // Safety: `self.length - 1` is always less than `usize::MAX`.
      self.tail = Some(unsafe { NonMaxUsize::new_unchecked(length - 1) });
    } else {
      self.head = None;
      self.tail = None;
    }

    map
  }

  /// Reorganizes the existing values to ensure maximum cache locality and shrinks the list such that no additional
  /// capacity exists.
  ///
  /// This is equivalent to calling [`VecList::pack_to`] with the current length.
  ///
  /// Complexity: O(n)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index_1 = list.push_back(5);
  /// let index_2 = list.push_back(10);
  /// let index_3 = list.push_front(100);
  /// list.remove(index_1);
  ///
  /// assert!(list.capacity() >= 3);
  ///
  /// let mut map = list.pack_to_fit();
  /// assert_eq!(list.capacity(), 2);
  /// assert_eq!(map.len(), 2);
  ///
  /// let index_2 = map.remove(&index_2).unwrap();
  /// let index_3 = map.remove(&index_3).unwrap();
  ///
  /// assert_eq!(list.get(index_2), Some(&10));
  /// assert_eq!(list.get(index_3), Some(&100));
  ///
  /// let mut iter = list.iter();
  /// assert_eq!(iter.next(), Some(&100));
  /// assert_eq!(iter.next(), Some(&10));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[cfg(feature = "std")]
  pub fn pack_to_fit(&mut self) -> HashMap<Index<T>, Index<T>> {
    self.pack_to(self.length)
  }

  /// Removes and returns the value at the back of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.pop_back(), None);
  ///
  /// list.push_back(0);
  /// list.push_back(1);
  /// list.push_back(2);
  /// assert_eq!(list.len(), 3);
  ///
  /// assert_eq!(list.pop_back(), Some(2));
  /// assert_eq!(list.len(), 2);
  /// ```
  pub fn pop_back(&mut self) -> Option<T> {
    self.remove_entry(self.tail?).map(|entry| entry.value)
  }

  /// Removes and returns the value at the front of the list, if it exists.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// assert_eq!(list.pop_front(), None);
  ///
  /// list.push_front(0);
  /// list.push_front(1);
  /// list.push_front(2);
  /// assert_eq!(list.len(), 3);
  ///
  /// assert_eq!(list.pop_front(), Some(2));
  /// assert_eq!(list.len(), 2);
  /// ```
  pub fn pop_front(&mut self) -> Option<T> {
    self.remove_entry(self.head?).map(|entry| entry.value)
  }

  /// Inserts the given value to the back of the list.
  ///
  /// The index of the newly inserted value will be returned.
  ///
  /// Complexity: amortized O(1)
  ///
  /// # Panics
  ///
  /// Panics if the new capacity overflows `usize`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index = list.push_back(0);
  /// assert_eq!(list.get(index), Some(&0));
  /// ```
  pub fn push_back(&mut self, value: T) -> Index<T> {
    let tail_index = match self.tail {
      Some(index) => index,
      None => return self.insert_empty(value),
    };
    let index = self.insert_new(value, Some(tail_index), None);
    self.entries[tail_index.get()].occupied_mut().next = Some(index);
    self.tail = Some(index);
    Index::new(index, self.generation)
  }

  /// Inserts the given value to the front of the list.
  ///
  /// The index of the newly inserted value will be returned.
  ///
  /// Complexity: amortized O(1)
  ///
  /// # Panics
  ///
  /// Panics if the new capacity overflows `usize`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index = list.push_front(0);
  /// assert_eq!(list.get(index), Some(&0));
  /// ```
  pub fn push_front(&mut self, value: T) -> Index<T> {
    let head_index = match self.head {
      Some(index) => index,
      None => return self.insert_empty(value),
    };
    let index = self.insert_new(value, None, Some(head_index));
    self.entries[head_index.get()].occupied_mut().previous = Some(index);
    self.head = Some(index);
    Index::new(index, self.generation)
  }

  /// Removes and returns the value at the given index, if it exists.
  ///
  /// If the index refers to an index not in the list anymore or if the index has been invalidated, then [`None`] will
  /// be returned and the list will be unaffected.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// let index = list.push_back(0);
  /// assert_eq!(list.remove(index), Some(0));
  /// assert_eq!(list.remove(index), None);
  /// ```
  pub fn remove(&mut self, index: Index<T>) -> Option<T> {
    let (previous_index, next_index) = match &self.entries[index.index()] {
      Entry::Occupied(entry) if entry.generation == index.generation => {
        (entry.previous, entry.next)
      }
      _ => return None,
    };
    Some(
      self
        .remove_helper(previous_index, index.index, next_index)
        .value,
    )
  }

  /// Removes and returns the entry at the given index, if it exists.
  ///
  /// If the index refers to an index not in the list anymore or if the index has been invalidated, then [`None`] will
  /// be returned and the list will be unaffected.
  fn remove_entry(&mut self, index: NonMaxUsize) -> Option<OccupiedEntry<T>> {
    let (previous_index, next_index) = match &self.entries[index.get()] {
      Entry::Occupied(entry) => (entry.previous, entry.next),
      Entry::Vacant(_) => return None,
    };
    Some(self.remove_helper(previous_index, index, next_index))
  }

  /// Removes and returns the entry at the given index with the entries previous and next index
  /// values.
  ///
  /// It is assumed that there is an entry at the given index.
  ///
  /// # Panics
  ///
  /// Panics if called when the list is empty. Behavior is undefined if provided indices do not follow the expected
  /// constraints.
  fn remove_helper(
    &mut self,
    previous_index: Option<NonMaxUsize>,
    index: NonMaxUsize,
    next_index: Option<NonMaxUsize>,
  ) -> OccupiedEntry<T> {
    let head_index = self.head.expect("expected head index");
    let tail_index = self.tail.expect("expected tail index");
    let vacant_head = self.vacant_head;
    let removed_entry = mem::replace(
      &mut self.entries[index.get()],
      Entry::Vacant(VacantEntry::new(vacant_head)),
    );

    self.generation = self.generation.wrapping_add(1);
    self.length -= 1;
    self.vacant_head = Some(index);

    if index == head_index && index == tail_index {
      self.head = None;
      self.tail = None;
    } else if index == head_index {
      self.entries[next_index.expect("expected next entry to exist").get()]
        .occupied_mut()
        .previous = None;
      self.head = next_index;
    } else if index == tail_index {
      self.entries[previous_index
        .expect("expected previous entry to exist")
        .get()]
      .occupied_mut()
      .next = None;
      self.tail = previous_index;
    } else {
      self.entries[next_index.expect("expected next entry to exist").get()]
        .occupied_mut()
        .previous = previous_index;
      self.entries[previous_index
        .expect("expected previous entry to exist")
        .get()]
      .occupied_mut()
      .next = next_index;
    }

    removed_entry.occupied()
  }

  /// Reserves capacity for the given expected size increase.
  ///
  /// The collection may reserve more space to avoid frequent reallocations. After calling this function, capacity will
  /// be greater than or equal to `self.len() + additional_capacity`. Does nothing if the current capacity is already
  /// sufficient.
  ///
  /// # Panics
  ///
  /// Panics if the new capacity overflows `usize`.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list: VecList<u32> = VecList::new();
  /// assert_eq!(list.capacity(), 0);
  ///
  /// list.reserve(10);
  /// assert!(list.capacity() >= 10);
  /// ```
  pub fn reserve(&mut self, additional_capacity: usize) {
    self.entries.reserve(additional_capacity);
  }

  /// Removes all elements from the list not satisfying the given predicate.
  ///
  /// Complexity: O(n)
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list = VecList::new();
  /// list.push_back(0);
  /// list.push_back(-1);
  /// list.push_back(1);
  /// list.push_back(-2);
  /// list.retain(|&mut value| value >= 0);
  ///
  /// let mut iter = list.iter();
  /// assert_eq!(iter.next(), Some(&0));
  /// assert_eq!(iter.next(), Some(&1));
  /// assert_eq!(iter.next(), None);
  /// ```
  pub fn retain<Predicate>(&mut self, mut predicate: Predicate)
  where
    Predicate: FnMut(&mut T) -> bool,
  {
    let mut next_index = self.head;

    while let Some(index) = next_index {
      let entry = self.entries[index.get()].occupied_mut();
      next_index = entry.next;

      if !predicate(&mut entry.value) {
        let _ = self.remove_entry(index);
      }
    }
  }

  /// Creates a new list with the given capacity.
  ///
  /// # Examples
  ///
  /// ```
  /// use dlv_list::VecList;
  ///
  /// let mut list: VecList<u32> = VecList::new();
  /// assert_eq!(list.capacity(), 0);
  ///
  /// let mut list: VecList<u32> = VecList::with_capacity(10);
  /// assert_eq!(list.capacity(), 10);
  /// ```
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    VecList {
      entries: Vec::with_capacity(capacity),
      generation: create_initial_generation(),
      head: None,
      length: 0,
      tail: None,
      vacant_head: None,
    }
  }
}

impl<T> Debug for VecList<T>
where
  T: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.debug_list().entries(self.iter()).finish()
  }
}

impl<T> Default for VecList<T> {
  fn default() -> Self {
    VecList {
      entries: Vec::default(),
      generation: create_initial_generation(),
      head: None,
      length: 0,
      tail: None,
      vacant_head: None,
    }
  }
}

impl<T> Eq for VecList<T> where T: Eq {}

impl<T> Extend<T> for VecList<T> {
  fn extend<Iter>(&mut self, iter: Iter)
  where
    Iter: IntoIterator<Item = T>,
  {
    let iter = iter.into_iter();
    self.reserve(iter.size_hint().0);

    for value in iter {
      let _ = self.push_back(value);
    }
  }
}

impl<'a, T> Extend<&'a T> for VecList<T>
where
  T: 'a + Copy,
{
  fn extend<Iter>(&mut self, iter: Iter)
  where
    Iter: IntoIterator<Item = &'a T>,
  {
    self.extend(iter.into_iter().copied());
  }
}

impl<T> FromIterator<T> for VecList<T> {
  fn from_iter<Iter>(iter: Iter) -> Self
  where
    Iter: IntoIterator<Item = T>,
  {
    let mut list = VecList::new();
    list.extend(iter);
    list
  }
}

impl<T> Hash for VecList<T>
where
  T: Hash,
{
  fn hash<StateHasher>(&self, state: &mut StateHasher)
  where
    StateHasher: Hasher,
  {
    self.len().hash(state);

    for value in self {
      value.hash(state);
    }
  }
}

impl<T> ops::Index<Index<T>> for VecList<T> {
  type Output = T;

  fn index(&self, index: Index<T>) -> &Self::Output {
    self.get(index).expect("expected entry at index")
  }
}

impl<T> ops::IndexMut<Index<T>> for VecList<T> {
  fn index_mut(&mut self, index: Index<T>) -> &mut Self::Output {
    self.get_mut(index).expect("expected entry at index")
  }
}

impl<T> IntoIterator for VecList<T> {
  type IntoIter = IntoIter<T>;
  type Item = T;

  fn into_iter(self) -> Self::IntoIter {
    IntoIter {
      head: self.head,
      remaining: self.length,
      tail: self.tail,
      list: self,
    }
  }
}

impl<'a, T> IntoIterator for &'a VecList<T> {
  type IntoIter = Iter<'a, T>;
  type Item = &'a T;

  fn into_iter(self) -> Self::IntoIter {
    Iter {
      entries: &self.entries,
      head: self.head,
      remaining: self.length,
      tail: self.tail,
    }
  }
}

impl<'a, T> IntoIterator for &'a mut VecList<T> {
  type IntoIter = IterMut<'a, T>;
  type Item = &'a mut T;

  fn into_iter(self) -> Self::IntoIter {
    IterMut {
      entries: &mut self.entries,
      head: self.head,
      phantom: PhantomData,
      remaining: self.length,
      tail: self.tail,
    }
  }
}

impl<T> Ord for VecList<T>
where
  T: Ord,
{
  fn cmp(&self, other: &Self) -> Ordering {
    self.iter().cmp(other)
  }
}

impl<T> PartialEq for VecList<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.len() == other.len() && self.iter().eq(other)
  }
}

impl<T> PartialEq<LinkedList<T>> for VecList<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &LinkedList<T>) -> bool {
    self.len() == other.len() && self.iter().eq(other)
  }
}

impl<T> PartialEq<VecList<T>> for LinkedList<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &VecList<T>) -> bool {
    other == self
  }
}

impl<T> PartialEq<Vec<T>> for VecList<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &Vec<T>) -> bool {
    self.len() == other.len() && self.iter().eq(other)
  }
}

impl<T> PartialEq<VecList<T>> for Vec<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &VecList<T>) -> bool {
    other == self
  }
}

impl<T, const N: usize> PartialEq<[T; N]> for VecList<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &[T; N]) -> bool {
    self.len() == other.len() && self.iter().eq(other.iter())
  }
}

impl<T, const N: usize> PartialEq<VecList<T>> for [T; N]
where
  T: PartialEq,
{
  fn eq(&self, other: &VecList<T>) -> bool {
    other == self
  }
}

impl<'a, T> PartialEq<&'a [T]> for VecList<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &&'a [T]) -> bool {
    self.len() == other.len() && self.iter().eq(other.iter())
  }
}

impl<T> PartialEq<VecList<T>> for &'_ [T]
where
  T: PartialEq,
{
  fn eq(&self, other: &VecList<T>) -> bool {
    other == self
  }
}

impl<T> PartialOrd for VecList<T>
where
  T: PartialOrd<T>,
{
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.iter().partial_cmp(other)
  }
}

/// A wrapper type that indicates an index into the list.
///
/// This index may be invalidated by operations on the list itself.
pub struct Index<T> {
  /// The generation of the entry currently at this index. This is used to avoid the ABA problem.
  generation: u64,

  /// The actual index into the entry list.
  index: NonMaxUsize,

  /// This type is parameterized on the entry data type to avoid indices being used across differently typed lists.
  phantom: PhantomData<T>,
}

impl<T> Clone for Index<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Index<T> {}

impl<T> Debug for Index<T> {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter
      .debug_tuple("Index")
      .field(&self.index)
      .field(&self.generation)
      .finish()
  }
}

impl<T> Eq for Index<T> {}

impl<T> Hash for Index<T> {
  fn hash<StateHasher>(&self, hasher: &mut StateHasher)
  where
    StateHasher: Hasher,
  {
    self.index.hash(hasher);
    self.generation.hash(hasher);
  }
}

impl<T> PartialEq for Index<T> {
  fn eq(&self, other: &Self) -> bool {
    self.generation == other.generation && self.index == other.index
  }
}

impl<T> Index<T> {
  /// Convenience function for creating new index.
  #[must_use]
  pub(self) fn new(index: NonMaxUsize, generation: u64) -> Index<T> {
    Index {
      generation,
      index,
      phantom: PhantomData,
    }
  }

  /// Get the index as usize
  #[inline]
  pub(self) fn index(&self) -> usize {
    self.index.get()
  }
}

/// An entry in the list. This can be either occupied or vacant.
#[derive(Clone)]
enum Entry<T> {
  /// An occupied entry contains actual entry data inserted by the user.
  Occupied(OccupiedEntry<T>),

  /// A vacant entry is one that can be reused.
  Vacant(VacantEntry),
}

impl<T> Entry<T> {
  /// Returns the occupied entry by moving it out of the entry.
  ///
  /// # Panics
  ///
  /// Panics if the variant is actually [`Entry::Vacant`].
  #[must_use]
  pub fn occupied(self) -> OccupiedEntry<T> {
    match self {
      Entry::Occupied(entry) => entry,
      Entry::Vacant(_) => panic!("expected occupied entry"),
    }
  }

  /// Returns an immutable reference to the occupied entry.
  ///
  /// # Panics
  ///
  /// Panics if the variant is actually [`Entry::Vacant`].
  #[must_use]
  pub fn occupied_ref(&self) -> &OccupiedEntry<T> {
    match self {
      Entry::Occupied(entry) => entry,
      Entry::Vacant(_) => panic!("expected occupied entry"),
    }
  }

  /// Returns a mutable reference to the occupied entry.
  ///
  /// # Panics
  ///
  /// Panics if the variant is actually [`Entry::Vacant`].
  #[must_use]
  pub fn occupied_mut(&mut self) -> &mut OccupiedEntry<T> {
    match self {
      Entry::Occupied(entry) => entry,
      Entry::Vacant(_) => panic!("expected occupied entry"),
    }
  }

  /// Returns an immutable reference to the vacant entry.
  ///
  /// # Panics
  ///
  /// Panics if the variant is actually [`Entry::Occupied`].
  #[must_use]
  pub fn vacant_ref(&self) -> &VacantEntry {
    match self {
      Entry::Vacant(entry) => entry,
      Entry::Occupied(_) => panic!("expected vacant entry"),
    }
  }
}

/// An occupied entry in the list.
#[derive(Clone)]
struct OccupiedEntry<T> {
  /// The generation of when this entry was inserted. This is used to avoid the ABA problem.
  generation: u64,

  /// The index of the next occupied entry in the list.
  next: Option<NonMaxUsize>,

  /// The index of the previous occupied entry in the list.
  previous: Option<NonMaxUsize>,

  /// The actual value being stored in this entry.
  value: T,
}

impl<T> OccupiedEntry<T> {
  /// Convenience function for creating a new occupied entry.
  #[must_use]
  pub fn new(
    generation: u64,
    previous: Option<NonMaxUsize>,
    next: Option<NonMaxUsize>,
    value: T,
  ) -> OccupiedEntry<T> {
    OccupiedEntry {
      generation,
      next,
      previous,
      value,
    }
  }
}

/// A vacant entry in the list.
#[derive(Clone, Debug)]
struct VacantEntry {
  /// The index of the next vacant entry in the list.
  next: Option<NonMaxUsize>,
}

impl VacantEntry {
  /// Convenience function for creating a new vacant entry.
  #[must_use]
  pub fn new(next: Option<NonMaxUsize>) -> VacantEntry {
    VacantEntry { next }
  }
}

/// An iterator that yields and removes all entries from the list.
pub struct Drain<'a, T> {
  /// The index of the head of the unvisited portion of the list.
  head: Option<NonMaxUsize>,

  /// A reference to the entry list.
  list: &'a mut VecList<T>,

  /// The number of entries that have not been visited.
  remaining: usize,

  /// The index of the tail of the unvisited portion of the list.
  tail: Option<NonMaxUsize>,
}

impl<T> Drain<'_, T> {
  /// Creates an iterator that yields immutable references to entries in the list.
  #[must_use]
  pub fn iter(&self) -> Iter<'_, T> {
    Iter {
      entries: &self.list.entries,
      head: self.head,
      remaining: self.remaining,
      tail: self.tail,
    }
  }
}

impl<T> Debug for Drain<'_, T>
where
  T: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("Drain(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<T> DoubleEndedIterator for Drain<'_, T> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail.map(|index| {
        let entry = self
          .list
          .remove_entry(index)
          .expect("expected occupied entry");
        self.tail = entry.previous;
        self.remaining -= 1;
        entry.value
      })
    }
  }
}

impl<T> Drop for Drain<'_, T> {
  fn drop(&mut self) {
    self.list.clear();
  }
}

impl<T> ExactSizeIterator for Drain<'_, T> {}

impl<T> FusedIterator for Drain<'_, T> {}

impl<T> Iterator for Drain<'_, T> {
  type Item = T;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head.map(|index| {
        let entry = self
          .list
          .remove_entry(index)
          .expect("expected occupied entry");
        self.head = entry.next;
        self.remaining -= 1;
        entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

/// An iterator that yields all indices in the list.
pub struct Indices<'a, T> {
  /// A reference to the actual storage for the entry list.
  entries: &'a Vec<Entry<T>>,

  /// The index of the head of the unvisited portion of the list.
  head: Option<NonMaxUsize>,

  /// The number of entries that have not been visited.
  remaining: usize,

  /// The index of the tail of the unvisited portion of the list.
  tail: Option<NonMaxUsize>,
}

impl<T> Clone for Indices<'_, T> {
  fn clone(&self) -> Self {
    Indices {
      entries: self.entries,
      head: self.head,
      remaining: self.remaining,
      tail: self.tail,
    }
  }
}

impl<T> Debug for Indices<'_, T>
where
  T: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("Indices(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<T> DoubleEndedIterator for Indices<'_, T> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail.map(|index| {
        let entry = self.entries[index.get()].occupied_ref();
        let index = Index::new(index, entry.generation);
        self.tail = entry.previous;
        self.remaining -= 1;
        index
      })
    }
  }
}

impl<T> ExactSizeIterator for Indices<'_, T> {}

impl<T> FusedIterator for Indices<'_, T> {}

impl<T> Iterator for Indices<'_, T> {
  type Item = Index<T>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head.map(|index| {
        let entry = self.entries[index.get()].occupied_ref();
        let index = Index::new(index, entry.generation);
        self.head = entry.next;
        self.remaining -= 1;
        index
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

/// An iterator that moves all entries out of the entry list.
#[derive(Clone)]
pub struct IntoIter<T> {
  /// The index of the head of the unvisited portion of the list.
  head: Option<NonMaxUsize>,

  /// The entry list from which entries are yielded.
  list: VecList<T>,

  /// The number of entries that have not been visited.
  remaining: usize,

  /// The index of the tail of the unvisited portion of the list.
  tail: Option<NonMaxUsize>,
}

impl<T> IntoIter<T> {
  /// Creates an iterator that yields immutable references to entries in the list.
  #[must_use]
  pub fn iter(&self) -> Iter<'_, T> {
    Iter {
      entries: &self.list.entries,
      head: self.head,
      remaining: self.remaining,
      tail: self.tail,
    }
  }
}

impl<T> Debug for IntoIter<T>
where
  T: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("IntoIter(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail.map(|index| {
        let entry = self
          .list
          .remove_entry(index)
          .expect("expected occupied entry");
        self.tail = entry.previous;
        self.remaining -= 1;
        entry.value
      })
    }
  }
}

impl<T> ExactSizeIterator for IntoIter<T> {}

impl<T> FusedIterator for IntoIter<T> {}

impl<T> Iterator for IntoIter<T> {
  type Item = T;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head.map(|index| {
        let entry = self
          .list
          .remove_entry(index)
          .expect("expected occupied entry");
        self.head = entry.next;
        self.remaining -= 1;
        entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

/// An iterator that yields immutable references to entries in the list.
pub struct Iter<'a, T> {
  /// A reference to the actual storage for the entry list.
  entries: &'a Vec<Entry<T>>,

  /// The index of the head of the unvisited portion of the list.
  head: Option<NonMaxUsize>,

  /// The number of entries that have not been visited.
  remaining: usize,

  /// The index of the tail of the unvisited portion of the list.
  tail: Option<NonMaxUsize>,
}

impl<'a, T> Clone for Iter<'a, T> {
  fn clone(&self) -> Iter<'a, T> {
    Iter {
      entries: self.entries,
      head: self.head,
      remaining: self.remaining,
      tail: self.tail,
    }
  }
}

impl<T> Debug for Iter<'_, T>
where
  T: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("Iter(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail.map(|index| {
        let entry = self.entries[index.get()].occupied_ref();
        self.tail = entry.previous;
        self.remaining -= 1;
        &entry.value
      })
    }
  }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

impl<T> FusedIterator for Iter<'_, T> {}

impl<'a, T> Iterator for Iter<'a, T> {
  type Item = &'a T;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head.map(|index| {
        let entry = self.entries[index.get()].occupied_ref();
        self.head = entry.next;
        self.remaining -= 1;
        &entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

/// An iterator that yields mutable references to entries in the list.
pub struct IterMut<'a, T> {
  entries: *mut Vec<Entry<T>>,

  /// The index of the head of the unvisited portion of the list.
  head: Option<NonMaxUsize>,

  /// Because [`IterMut::entries`] is a pointer, we need to have a phantom data here for the lifetime parameter.
  phantom: PhantomData<&'a mut Vec<Entry<T>>>,

  /// The number of entries that have not been visited.
  remaining: usize,

  /// The index of the tail of the unvisited portion of the list.
  tail: Option<NonMaxUsize>,
}

impl<T> IterMut<'_, T> {
  /// Creates an iterator that yields immutable references to entries in the list.
  #[must_use]
  pub fn iter(&self) -> Iter<'_, T> {
    Iter {
      entries: unsafe { &*self.entries },
      head: self.head,
      remaining: self.remaining,
      tail: self.tail,
    }
  }
}

impl<T> Debug for IterMut<'_, T>
where
  T: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("IterMut(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<T> DoubleEndedIterator for IterMut<'_, T> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail.map(|index| {
        let entry = unsafe { &mut (*self.entries)[index.get()] }.occupied_mut();
        self.tail = entry.previous;
        self.remaining -= 1;
        &mut entry.value
      })
    }
  }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {}

impl<T> FusedIterator for IterMut<'_, T> {}

impl<'a, T> Iterator for IterMut<'a, T> {
  type Item = &'a mut T;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head.map(|index| {
        let entry = unsafe { &mut (*self.entries)[index.get()] }.occupied_mut();
        self.head = entry.next;
        self.remaining -= 1;
        &mut entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

unsafe impl<T> Send for IterMut<'_, T> where T: Send {}

unsafe impl<T> Sync for IterMut<'_, T> where T: Sync {}

/// Creates the initial generation seeded by the current time.
#[must_use]
fn create_initial_generation() -> u64 {
  #[cfg(feature = "std")]
  {
    use std::{collections::hash_map::RandomState, hash::BuildHasher};

    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u32(0);
    hasher.finish()
  }

  #[cfg(not(feature = "std"))]
  {
    use core::sync::atomic::{AtomicU32, Ordering};

    // Generate a u32 randomly.
    #[cfg_attr(mutants, mutants::skip)]
    fn gen_u32() -> u32 {
      static SEED: AtomicU32 = AtomicU32::new({
        // Random seed generated at compile time.
        const_random::const_random!(u32)
      });

      // Xorshift is "good enough" in most cases.
      let mut x = SEED.load(Ordering::Relaxed);

      loop {
        let mut random = x;
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;

        // Put the new seed in.
        if let Err(actual) = SEED.compare_exchange(x, random, Ordering::SeqCst, Ordering::SeqCst) {
          x = actual;
        } else {
          return random;
        }
      }
    }

    // Put two u32's together
    gen_u32() as u64 | ((gen_u32() as u64) << 32)
  }
}

#[allow(unused_results)]
#[cfg(test)]
mod test {
  use coverage_helper::test;

  use super::*;
  use alloc::{format, vec};

  #[cfg(feature = "std")]
  use std::{collections::hash_map::RandomState, hash::BuildHasher};

  #[test]
  fn test_bounds() {
    fn check_bounds<Type: Send + Sync>() {}

    check_bounds::<VecList<()>>();
    check_bounds::<Index<()>>();
    check_bounds::<Drain<'_, ()>>();
    check_bounds::<Indices<'_, ()>>();
    check_bounds::<IntoIter<()>>();
    check_bounds::<Iter<'_, ()>>();
    check_bounds::<IterMut<'_, ()>>();
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_non_max_usize_eq() {
    let zero = NonMaxUsize::zero();
    assert_eq!(zero, 0usize);
    assert_ne!(zero, 1usize);
  }

  #[test]
  fn test_drain_debug() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let drain = list.drain();
    assert_eq!(format!("{drain:?}"), "Drain([0, 1, -1, 2, -2])");
  }

  #[test]
  fn test_drain_double_ended() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut drain = list.drain();
    assert_eq!(drain.next(), Some(0));
    assert_eq!(drain.next_back(), Some(-2));
    assert_eq!(drain.next(), Some(1));
    assert_eq!(drain.next_back(), Some(2));
    assert_eq!(drain.next(), Some(-1));
    assert_eq!(drain.next_back(), None);
  }

  #[test]
  fn test_drain_empty() {
    let mut list: VecList<i32> = VecList::new();
    let mut drain = list.drain();
    assert_eq!(drain.next(), None);
  }

  #[test]
  fn test_drain_fused() {
    let mut list: VecList<i32> = VecList::new();
    list.push_back(0);
    let mut drain = list.drain();
    assert_eq!(drain.next(), Some(0));
    assert_eq!(drain.next(), None);
    assert_eq!(drain.next(), None);
    assert_eq!(drain.next(), None);
  }

  #[test]
  fn test_drain_size_hint() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut drain = list.drain();

    assert_eq!(drain.size_hint(), (5, Some(5)));
    drain.next();
    assert_eq!(drain.size_hint(), (4, Some(4)));
    drain.next();
    assert_eq!(drain.size_hint(), (3, Some(3)));
    drain.next();
    assert_eq!(drain.size_hint(), (2, Some(2)));
    drain.next();
    assert_eq!(drain.size_hint(), (1, Some(1)));
    drain.next();
    assert_eq!(drain.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_index_debug() {
    let mut list = VecList::new();
    let index = list.push_back(5);

    assert_eq!(
      format!("{index:?}"),
      format!("Index(0, {})", index.generation)
    );
  }

  #[test]
  fn test_index_equality() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.indices().next().unwrap();
    assert_eq!(index_1, index_2);

    let index_3 = list.push_back(1);
    assert_ne!(index_1, index_3);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_index_hash() {
    let state = RandomState::new();

    fn hash(state: &RandomState, value: &Index<usize>) -> u64 {
      let mut hasher = state.build_hasher();
      value.hash(&mut hasher);
      hasher.finish()
    }

    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(2);

    assert_eq!(hash(&state, &index_1), hash(&state, &index_1));
    assert_ne!(hash(&state, &index_1), hash(&state, &index_2));
  }

  #[test]
  fn test_indices_debug() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let indices = list.indices();
    assert_eq!(
      format!("{indices:?}"),
      format!(
        "Indices([Index(0, {}), Index(1, {}), Index(2, {}), Index(3, {}), Index(4, {})])",
        list.generation, list.generation, list.generation, list.generation, list.generation
      )
    );
  }

  #[test]
  fn test_indices_double_ended() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut indices = list.indices();
    assert_eq!(indices.next().unwrap().index.get(), 0);
    assert_eq!(indices.next_back().unwrap().index.get(), 4);
    assert_eq!(indices.next().unwrap().index.get(), 1);
    assert_eq!(indices.next_back().unwrap().index.get(), 3);
    assert_eq!(indices.next().unwrap().index.get(), 2);
    assert_eq!(indices.next_back(), None);
  }

  #[test]
  fn test_indices_empty() {
    let list: VecList<i32> = VecList::new();
    let mut indices = list.indices();
    assert_eq!(indices.next(), None);
  }

  #[test]
  fn test_indices_fused() {
    let mut list: VecList<i32> = VecList::new();
    list.push_back(0);
    let mut indices = list.indices();
    assert_eq!(indices.next().unwrap().index.get(), 0);
    assert_eq!(indices.next(), None);
    assert_eq!(indices.next(), None);
    assert_eq!(indices.next(), None);
  }

  #[test]
  fn test_indices_size_hint() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut indices = list.indices();

    assert_eq!(indices.size_hint(), (5, Some(5)));
    indices.next();
    assert_eq!(indices.size_hint(), (4, Some(4)));
    indices.next();
    assert_eq!(indices.size_hint(), (3, Some(3)));
    indices.next();
    assert_eq!(indices.size_hint(), (2, Some(2)));
    indices.next();
    assert_eq!(indices.size_hint(), (1, Some(1)));
    indices.next();
    assert_eq!(indices.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_into_iter_debug() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let iter = list.into_iter();
    assert_eq!(format!("{iter:?}"), "IntoIter([0, 1, -1, 2, -2])");
  }

  #[test]
  fn test_into_iter_double_ended() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut iter = list.into_iter();
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next_back(), Some(-2));
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next_back(), Some(2));
    assert_eq!(iter.next(), Some(-1));
    assert_eq!(iter.next_back(), None);
  }

  #[test]
  fn test_into_iter_empty() {
    let list: VecList<i32> = VecList::new();
    let mut iter = list.into_iter();
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_into_iter_fused() {
    let mut list: VecList<i32> = VecList::new();
    list.push_back(0);
    let mut iter = list.into_iter();
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_into_iter_size_hint() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut iter = list.into_iter();

    assert_eq!(iter.size_hint(), (5, Some(5)));
    iter.next();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    iter.next();
    assert_eq!(iter.size_hint(), (3, Some(3)));
    iter.next();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    iter.next();
    assert_eq!(iter.size_hint(), (1, Some(1)));
    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_iter_debug() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let iter = list.iter();
    assert_eq!(format!("{iter:?}"), "Iter([0, 1, -1, 2, -2])");
  }

  #[test]
  fn test_iter_double_ended() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut iter = list.iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next_back(), Some(&-2));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next_back(), Some(&2));
    assert_eq!(iter.next(), Some(&-1));
    assert_eq!(iter.next_back(), None);
  }

  #[test]
  fn test_iter_empty() {
    let list: VecList<i32> = VecList::new();
    let mut iter = list.iter();
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_fused() {
    let mut list: VecList<i32> = VecList::new();
    list.push_back(0);
    let mut iter = list.iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_size_hint() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut iter = list.iter();

    assert_eq!(iter.size_hint(), (5, Some(5)));
    iter.next();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    iter.next();
    assert_eq!(iter.size_hint(), (3, Some(3)));
    iter.next();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    iter.next();
    assert_eq!(iter.size_hint(), (1, Some(1)));
    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_iter_mut_debug() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let iter = list.iter_mut();
    assert_eq!(format!("{iter:?}"), "IterMut([0, 1, -1, 2, -2])");
  }

  #[test]
  fn test_iter_mut_double_ended() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut iter = list.iter_mut();
    assert_eq!(iter.next(), Some(&mut 0));
    assert_eq!(iter.next_back(), Some(&mut -2));
    assert_eq!(iter.next(), Some(&mut 1));
    assert_eq!(iter.next_back(), Some(&mut 2));
    assert_eq!(iter.next(), Some(&mut -1));
    assert_eq!(iter.next_back(), None);
  }

  #[test]
  fn test_iter_mut_empty() {
    let mut list: VecList<i32> = VecList::new();
    let mut iter = list.iter_mut();
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_mut_fused() {
    let mut list: VecList<i32> = VecList::new();
    list.push_back(0);
    let mut iter = list.iter_mut();
    assert_eq!(iter.next(), Some(&mut 0));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_mut_size_hint() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    let mut iter = list.iter_mut();

    assert_eq!(iter.size_hint(), (5, Some(5)));
    iter.next();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    iter.next();
    assert_eq!(iter.size_hint(), (3, Some(3)));
    iter.next();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    iter.next();
    assert_eq!(iter.size_hint(), (1, Some(1)));
    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_vec_list_back() {
    let mut list = VecList::new();
    assert_eq!(list.back(), None);

    let index_1 = list.push_back(0);
    assert_eq!(list.back(), Some(&0));

    let index_2 = list.push_back(1);
    assert_eq!(list.back(), Some(&1));

    list.remove(index_2);
    assert_eq!(list.back(), Some(&0));

    list.remove(index_1);
    assert_eq!(list.back(), None);
  }

  #[test]
  fn test_vec_list_back_mut() {
    let mut list = VecList::new();
    assert_eq!(list.back_mut(), None);

    let index_1 = list.push_back(0);
    assert_eq!(list.back_mut(), Some(&mut 0));

    let index_2 = list.push_back(1);
    assert_eq!(list.back_mut(), Some(&mut 1));

    list.remove(index_2);
    assert_eq!(list.back_mut(), Some(&mut 0));

    list.remove(index_1);
    assert_eq!(list.back_mut(), None);
  }

  #[test]
  fn test_vec_list_capacity() {
    let list: VecList<i32> = VecList::new();
    assert_eq!(list.capacity(), 0);
  }

  #[test]
  fn test_vec_list_clear() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    list.clear();
    assert!(list.is_empty());
    assert_eq!(list.get(index), None);
  }

  #[test]
  fn test_vec_list_contains() {
    let mut list = VecList::new();
    assert!(!list.contains(&0));

    let index = list.push_back(0);
    assert!(list.contains(&0));

    list.remove(index);
    assert!(!list.contains(&0));
  }

  #[test]
  fn test_vec_list_drain() {
    let mut list = VecList::new();
    list.drain();
    assert!(list.is_empty());

    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.drain();
    assert!(list.is_empty());
  }

  #[test]
  fn test_vec_list_debug() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    assert_eq!(format!("{list:?}"), "[0, 1, -1, 2, -2]");
  }

  #[test]
  fn test_vec_list_equality() {
    let mut list_1 = VecList::new();
    list_1.push_back(0);
    list_1.push_back(1);
    list_1.push_back(-1);
    list_1.push_back(2);
    list_1.push_back(-2);

    assert_eq!(list_1, Vec::from_iter([0, 1, -1, 2, -2]));
    assert_eq!(Vec::from_iter([0, 1, -1, 2, -2]), list_1);
    assert_ne!(list_1, Vec::new());
    assert_ne!(Vec::new(), list_1);

    assert_eq!(list_1, LinkedList::from_iter([0, 1, -1, 2, -2]));
    assert_eq!(LinkedList::from_iter([0, 1, -1, 2, -2]), list_1);
    assert_ne!(list_1, LinkedList::new());
    assert_ne!(LinkedList::new(), list_1);

    assert_eq!(list_1, [0, 1, -1, 2, -2]);
    assert_eq!([0, 1, -1, 2, -2], list_1);
    assert_ne!(list_1, []);
    assert_ne!([], list_1);

    assert_eq!(list_1, [0, 1, -1, 2, -2].as_slice());
    assert_eq!([0, 1, -1, 2, -2].as_slice(), list_1);
    assert_ne!(list_1, [].as_slice());
    assert_ne!([].as_slice(), list_1);

    let mut list_2 = list_1.clone();
    list_2.pop_back();
    assert_ne!(list_1, list_2);

    list_2.push_back(-2);
    assert_eq!(list_1, list_2);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_hash() {
    let state = RandomState::new();
    fn hash(state: &RandomState, value: &VecList<usize>) -> u64 {
      let mut hasher = state.build_hasher();
      value.hash(&mut hasher);
      hasher.finish()
    }

    let mut list_1 = VecList::new();
    list_1.push_back(0);

    let list_2 = VecList::new();

    assert_eq!(hash(&state, &list_1), hash(&state, &list_1));
    assert_ne!(hash(&state, &list_1), hash(&state, &list_2));
  }

  #[test]
  fn test_vec_list_extend() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.extend([-1, 2, -2].iter());

    assert_eq!(list, &[0, 1, -1, 2, -2][..]);
  }

  #[test]
  fn test_vec_list_from_iterator() {
    let list = VecList::from_iter([0, 1, -1, 2, -2].iter().cloned());
    assert_eq!(list, &[0, 1, -1, 2, -2][..]);
  }

  #[test]
  fn test_vec_list_front() {
    let mut list = VecList::new();
    assert_eq!(list.front(), None);

    let index_1 = list.push_front(0);
    assert_eq!(list.front(), Some(&0));

    let index_2 = list.push_front(1);
    assert_eq!(list.front(), Some(&1));

    list.remove(index_2);
    assert_eq!(list.front(), Some(&0));

    list.remove(index_1);
    assert_eq!(list.front(), None);
  }

  #[test]
  fn test_vec_list_front_mut() {
    let mut list = VecList::new();
    assert_eq!(list.front_mut(), None);

    let index_1 = list.push_front(0);
    assert_eq!(list.front_mut(), Some(&mut 0));

    let index_2 = list.push_front(1);
    assert_eq!(list.front_mut(), Some(&mut 1));

    list.remove(index_2);
    assert_eq!(list.front_mut(), Some(&mut 0));

    list.remove(index_1);
    assert_eq!(list.front_mut(), None);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_get() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    assert_eq!(list.get(index), Some(&0));
    list.remove(index);
    assert_eq!(list.get(index), None);

    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);

    list.remove(index_1);
    list.pack_to_fit();
    assert_eq!(list.get(index_1), None);
    assert_eq!(list.get(index_2), None);
    assert_eq!(list.get(index_3), None);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_get_mut() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    assert_eq!(list.get_mut(index), Some(&mut 0));
    list.remove(index);
    assert_eq!(list.get_mut(index), None);

    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);

    list.remove(index_1);
    list.pack_to_fit();
    assert_eq!(list.get_mut(index_1), None);
    assert_eq!(list.get_mut(index_2), None);
    assert_eq!(list.get_mut(index_3), None);
  }

  #[test]
  fn test_vec_list_get_unchecked() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    assert_eq!(unsafe { list.get_unchecked(index) }, &0);

    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);

    list.remove(index_1);
    assert_eq!(unsafe { list.get_unchecked(index_2) }, &1);
    assert_eq!(unsafe { list.get_unchecked(index_3) }, &2);
  }

  #[test]
  fn test_vec_list_get_unchecked_mut() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    assert_eq!(unsafe { list.get_unchecked_mut(index) }, &mut 0);

    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);

    list.remove(index_1);
    assert_eq!(unsafe { list.get_unchecked_mut(index_2) }, &mut 1);
    assert_eq!(unsafe { list.get_unchecked_mut(index_3) }, &mut 2);
  }

  #[test]
  fn test_vec_list_get_next_index() {
    let mut list = VecList::new();

    let index = list.push_back(0);
    assert_eq!(list.get_next_index(index), None);

    list.push_back(1);
    assert_eq!(list.get_next_index(index).unwrap().index.get(), 1);
  }

  #[test]
  fn test_vec_list_get_previous_index() {
    let mut list = VecList::new();

    let index = list.push_front(0);
    assert_eq!(list.get_previous_index(index), None);

    list.push_front(1);
    assert_eq!(list.get_previous_index(index).unwrap().index.get(), 1);
  }

  #[test]
  fn test_vec_list_index() {
    let mut list = VecList::new();

    let index = list.push_back(5);
    assert_eq!(list[index], 5);

    list[index] = 10;
    assert_eq!(list[index], 10);
  }

  #[should_panic]
  #[test]
  fn test_vec_list_index_panic() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    list.pop_back();
    let _ = list[index];
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_indices() {
    let mut list = VecList::new();
    let mut iter = list.indices();
    assert_eq!(iter.next(), None);

    list.push_back(0);
    let index = list.push_back(1);
    list.push_back(-1);
    list.remove(index);

    let mut iter = list.indices();
    assert_eq!(iter.next().unwrap().index.get(), 0);
    assert_eq!(iter.next().unwrap().index.get(), 2);
    assert_eq!(iter.next(), None);

    list.pack_to_fit();

    let mut iter = list.indices();
    assert_eq!(iter.next().unwrap().index.get(), 0);
    assert_eq!(iter.next().unwrap().index.get(), 1);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_vec_list_insert_after() {
    let mut list = VecList::new();
    let index_1 = list.push_front(0);
    let index_2 = list.insert_after(index_1, 1);

    assert_eq!(list.back(), Some(&1));
    assert_eq!(list.get_previous_index(index_2), Some(index_1));
    assert_eq!(list.get_next_index(index_1), Some(index_2));

    let index_3 = list.insert_after(index_1, 2);

    assert_eq!(list.get_previous_index(index_3), Some(index_1));
    assert_eq!(list.get_next_index(index_1), Some(index_3));
    assert_eq!(list.get_next_index(index_3), Some(index_2));
  }

  #[should_panic]
  #[test]
  fn test_vec_list_insert_after_panic_index_invalidated() {
    let mut list = VecList::new();
    let index = list.push_front(0);
    list.remove(index);
    list.insert_after(index, 1);
  }

  #[cfg(feature = "std")]
  #[should_panic]
  #[test]
  fn test_vec_list_insert_after_panic_index_out_of_bounds() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    list.push_back(1);
    let index_2 = list.push_back(2);

    list.remove(index_1);
    list.pack_to_fit();
    list.insert_after(index_2, 3);
  }

  #[test]
  fn test_vec_list_insert_before() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.insert_before(index_1, 1);

    assert_eq!(list.front(), Some(&1));
    assert_eq!(list.get_previous_index(index_1), Some(index_2));
    assert_eq!(list.get_next_index(index_2), Some(index_1));

    let index_3 = list.insert_before(index_1, 2);

    assert_eq!(list.get_previous_index(index_1), Some(index_3));
    assert_eq!(list.get_next_index(index_3), Some(index_1));
    assert_eq!(list.get_next_index(index_2), Some(index_3));
  }

  #[should_panic]
  #[test]
  fn test_vec_list_insert_before_panic_index_invalidated() {
    let mut list = VecList::new();
    let index = list.push_front(0);
    list.remove(index);
    list.insert_before(index, 1);
  }

  #[cfg(feature = "std")]
  #[should_panic]
  #[test]
  fn test_vec_list_insert_before_panic_index_out_of_bounds() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    list.push_back(1);
    let index_2 = list.push_back(2);

    list.remove(index_1);
    list.pack_to_fit();
    list.insert_before(index_2, 3);
  }

  #[test]
  fn test_vec_list_into_iterator() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    assert_eq!(list.into_iter().collect::<Vec<_>>(), [0, 1, -1, 2, -2]);
  }

  #[test]
  fn test_vec_list_is_empty() {
    let mut list = VecList::new();
    assert!(list.is_empty());
    list.push_back(0);
    assert!(!list.is_empty());
  }

  #[test]
  fn test_vec_list_iter() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(2);

    let mut iter = list.iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_vec_list_iter_mut() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(2);

    let mut iter = list.iter_mut();
    let value = iter.next().unwrap();
    *value = 100;

    assert_eq!(iter.next(), Some(&mut 1));
    assert_eq!(iter.next(), Some(&mut 2));
    assert_eq!(iter.next(), None);
    assert_eq!(list.front(), Some(&100));
  }

  #[test]
  fn test_vec_list_len() {
    let mut list = VecList::new();
    assert_eq!(list.len(), 0);
    let index = list.push_back(0);
    assert_eq!(list.len(), 1);
    list.remove(index);
    assert_eq!(list.len(), 0);
  }

  #[test]
  fn test_vec_list_new() {
    let list: VecList<i32> = VecList::new();
    assert_eq!(list.capacity(), 0);
    assert_eq!(list.len(), 0);
  }

  #[test]
  fn test_vec_list_ordering() {
    let mut list_1 = VecList::new();
    list_1.push_back(0);
    list_1.push_back(1);
    list_1.push_back(-1);
    list_1.push_back(2);
    list_1.push_back(-2);

    let mut list_2 = list_1.clone();

    list_2.push_back(5);
    assert!(list_1 < list_2);

    list_2.pop_back();
    list_2.pop_back();
    assert!(list_1 > list_2);

    list_2.push_back(3);
    assert!(list_1 < list_2);

    list_2.pop_back();
    list_2.push_back(-3);
    assert!(list_1 > list_2);
  }

  #[test]
  fn test_vec_list_pop_back() {
    let mut list = VecList::new();
    assert_eq!(list.pop_back(), None);

    list.push_back(0);
    assert_eq!(list.pop_back(), Some(0));
  }

  #[test]
  fn test_vec_list_pop_front() {
    let mut list = VecList::new();
    assert_eq!(list.pop_front(), None);

    list.push_front(0);
    assert_eq!(list.pop_front(), Some(0));
  }

  #[test]
  fn test_vec_list_push_back() {
    let mut list = VecList::new();
    list.push_back(0);
    assert_eq!(list.back(), Some(&0));
    list.push_back(1);
    assert_eq!(list.back(), Some(&1));
    list.push_back(2);
    assert_eq!(list.back(), Some(&2));
  }

  #[test]
  fn test_vec_list_push_back_capacity_increases() {
    let mut list = VecList::with_capacity(1);
    assert_eq!(list.capacity(), 1);

    let index = list.push_back(0);
    assert_eq!(list.capacity(), 1);

    list.remove(index);
    assert_eq!(list.capacity(), 1);

    list.push_back(0);
    assert_eq!(list.capacity(), 1);

    list.push_back(1);
    assert!(list.capacity() > 1);
  }

  #[test]
  fn test_vec_list_push_front() {
    let mut list = VecList::new();
    list.push_front(0);
    assert_eq!(list.front(), Some(&0));
    list.push_front(1);
    assert_eq!(list.front(), Some(&1));
    list.push_front(2);
    assert_eq!(list.front(), Some(&2));
  }

  #[test]
  fn test_vec_list_remove() {
    let mut list = VecList::new();
    let index = list.push_back(0);
    assert_eq!(list.remove(index), Some(0));
    assert_eq!(list.remove(index), None);
  }

  #[test]
  fn test_vec_list_reserve() {
    let mut list: VecList<i32> = VecList::new();
    assert_eq!(list.capacity(), 0);

    list.reserve(10);
    let capacity = list.capacity();

    assert!(capacity >= 10);
    list.reserve(5);

    assert_eq!(list.capacity(), capacity);
  }

  #[test]
  fn test_vec_list_retain() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(-1);
    list.push_back(2);
    list.push_back(-2);

    list.retain(|&mut value| value >= 0);
    assert_eq!(list.into_iter().collect::<Vec<_>>(), [0, 1, 2]);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_pack_to() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);
    assert!(list.capacity() >= 3);

    list.remove(index_1);
    assert!(list.capacity() >= 3);

    let indices = list.indices();
    assert_eq!(
      indices.map(|index| index.index.get()).collect::<Vec<_>>(),
      [1, 2]
    );

    let map = list.pack_to(5);
    assert_eq!(list.capacity(), 5);

    let indices = list.indices();
    assert_eq!(
      indices.map(|index| index.index.get()).collect::<Vec<_>>(),
      [0, 1]
    );

    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&index_2).unwrap().index.get(), 0);
    assert_eq!(map.get(&index_3).unwrap().index.get(), 1);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_pack_to_empty() {
    let mut list: VecList<i32> = VecList::with_capacity(5);
    list.pack_to(0);
    assert_eq!(list.capacity(), 0);
  }

  #[cfg(feature = "std")]
  #[should_panic]
  #[test]
  fn test_vec_list_pack_to_panic() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(2);
    list.pack_to(2);
  }

  #[cfg(feature = "std")]
  #[test]
  fn test_vec_list_pack_to_fit() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);
    assert!(list.capacity() >= 3);

    list.remove(index_1);
    assert!(list.capacity() >= 3);

    let indices = list.indices();
    assert_eq!(
      indices.map(|index| index.index.get()).collect::<Vec<_>>(),
      [1, 2]
    );

    let map = list.pack_to_fit();
    assert_eq!(list.capacity(), 2);

    let indices = list.indices();
    assert_eq!(
      indices.map(|index| index.index.get()).collect::<Vec<_>>(),
      [0, 1]
    );

    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&index_2).unwrap().index.get(), 0);
    assert_eq!(map.get(&index_3).unwrap().index.get(), 1);
  }

  #[test]
  fn test_vec_list_with_capacity() {
    let list: VecList<i32> = VecList::with_capacity(10);
    assert_eq!(list.capacity(), 10);
  }

  #[test]
  fn test_vec_list_clone_from() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);

    let mut list2 = VecList::new();
    list2.clone_from(&list);
    assert_eq!(list2.get(index_1), Some(&0));
    assert_eq!(list2.get(index_2), Some(&1));
    assert_eq!(list2.get(index_3), Some(&2));
  }

  #[test]
  fn test_move_individual_elements() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    let index_3 = list.push_back(2);
    let index_4 = list.push_back(3);

    // Move to tail
    list.move_after(index_1, index_4);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 0]);
    assert_eq!(
      list.iter().rev().copied().collect::<Vec<_>>(),
      vec![0, 3, 2, 1]
    );
    assert_eq!(list.back(), list.get(index_1));

    // Move to head
    list.move_before(index_1, index_2);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert_eq!(
      list.iter().rev().copied().collect::<Vec<_>>(),
      vec![3, 2, 1, 0]
    );

    // Move non-tail/head node
    list.move_before(index_3, index_2);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 2, 1, 3]);
    assert_eq!(
      list.iter().rev().copied().collect::<Vec<_>>(),
      vec![3, 1, 2, 0]
    );
  }

  #[test]
  fn test_move_back_index_front_index() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    list.push_back(1);
    list.push_back(2);
    list.push_back(3);

    // Move to tail
    list.move_after(index_1, list.back_index().unwrap());
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 0]);
    assert_eq!(
      list.iter().rev().copied().collect::<Vec<_>>(),
      vec![0, 3, 2, 1]
    );
    assert_eq!(list.back(), list.get(index_1));

    // Move to head
    list.move_before(index_1, list.front_index().unwrap());
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert_eq!(
      list.iter().rev().copied().collect::<Vec<_>>(),
      vec![3, 2, 1, 0]
    );
  }

  #[should_panic]
  #[test]
  fn test_move_after_panic1() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    list.remove(index_1);
    list.move_after(index_1, index_2);
  }

  #[should_panic]
  #[test]
  fn test_move_after_panic2() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    list.remove(index_1);
    list.move_after(index_2, index_1);
  }

  #[should_panic]
  #[test]
  fn test_move_after_panic3() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    list.move_after(index_1, index_1);
  }

  #[should_panic]
  #[test]
  fn test_move_before_panic1() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    list.remove(index_1);
    list.move_before(index_1, index_2);
  }

  #[should_panic]
  #[test]
  fn test_move_before_panic2() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    let index_2 = list.push_back(1);
    list.remove(index_1);
    list.move_before(index_2, index_1);
  }

  #[should_panic]
  #[test]
  fn test_move_before_panic3() {
    let mut list = VecList::new();
    let index_1 = list.push_back(0);
    list.move_before(index_1, index_1);
  }
}
