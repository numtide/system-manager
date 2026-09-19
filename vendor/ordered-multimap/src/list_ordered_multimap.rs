//! Provides types related to the usage of [`ListOrderedMultimap`].

#![allow(unsafe_code)]

use alloc::vec;
use core::{
  borrow::Borrow,
  fmt::{self, Debug, Formatter},
  hash::{BuildHasher, Hash, Hasher},
  iter::{FromIterator, FusedIterator},
  marker::PhantomData,
};

use dlv_list::{
  Index, IntoIter as VecListIntoIter, Iter as VecListIter, IterMut as VecListIterMut, VecList,
};
use hashbrown::{
  hash_map::{RawEntryMut, RawOccupiedEntryMut},
  HashMap,
};

/// A random state to use for the hashmap in the multimap.
#[cfg(feature = "std")]
pub type RandomState = std::collections::hash_map::RandomState;

/// A random state to use for the hashmap in the multimap.
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct RandomState(core::convert::Infallible);

#[cfg(not(feature = "std"))]
impl RandomState {
  /// Creates a new random state.
  #[cfg_attr(mutants, mutants::skip)]
  #[must_use]
  pub fn new() -> RandomState {
    panic!("RandomState is not available without std")
  }
}

#[cfg(not(feature = "std"))]
impl Default for RandomState {
  #[cfg_attr(mutants, mutants::skip)]
  fn default() -> RandomState {
    RandomState::new()
  }
}

#[cfg(not(feature = "std"))]
impl BuildHasher for RandomState {
  type Hasher = DummyHasher;

  #[cfg_attr(mutants, mutants::skip)]
  fn build_hasher(&self) -> Self::Hasher {
    match self.0 {}
  }
}

#[derive(Clone)]
/// A multimap that associates with each key a list of values.
///
/// # Ordering
///
/// The primary guarantee this type gives is that regardless of what you do to the multimap, you are always able to
/// iterate through all keys and values in the order they were inserted. Values can be iterated by their insertion order
/// either for a specific key or for the entire map.
///
/// # Allocations
///
/// Allocations may be performed on any key-value insertion.
pub struct ListOrderedMultimap<Key, Value, State = RandomState> {
  /// The hasher builder that constructs new hashers for hashing keys. We have to keep this separate from the hashmap
  /// itself as we need to be able to access it when the hashmap keys are reallocated due to changes. We cannot use the
  /// hash of the actual keys in the map as those hashes are not representative.
  pub(crate) build_hasher: State,

  /// The list of the keys in the multimap. This is ordered by time of insertion.
  pub(crate) keys: VecList<Key>,

  /// The map from indices of keys to the indices of their values in the value list. The list of the indices is ordered
  /// by time of insertion. We never use hasher of the hashmap explicitly here, we instead use
  /// [`ListOrderedMultimap::build_hasher`].
  pub(crate) map: HashMap<Index<Key>, MapEntry<Key, Value>, DummyState>,

  /// The list of the values in the multimap. This is ordered by time of insertion.
  pub(crate) values: VecList<ValueEntry<Key, Value>>,
}

#[cfg(feature = "std")]
impl<Key, Value> ListOrderedMultimap<Key, Value, RandomState> {
  /// Creates a new multimap with no initial capacity.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key1", "value1");
  /// assert_eq!(map.get(&"key1"), Some(&"value1"));
  /// ```
  #[must_use]
  pub fn new() -> ListOrderedMultimap<Key, Value, RandomState> {
    ListOrderedMultimap {
      build_hasher: RandomState::new(),
      keys: VecList::new(),
      map: HashMap::with_hasher(DummyState),
      values: VecList::new(),
    }
  }

  /// Creates a new multimap with the specified capacities.
  ///
  /// The multimap will be able to hold at least `key_capacity` keys and `value_capacity` values without reallocating.
  /// A capacity of 0 will result in no allocation for the respective container.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
  /// assert_eq!(map.keys_capacity(), 0);
  /// assert_eq!(map.values_capacity(), 0);
  ///
  /// let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::with_capacity(5, 10);
  /// assert_eq!(map.keys_capacity(), 5);
  /// assert_eq!(map.values_capacity(), 10);
  /// ```
  #[must_use]
  pub fn with_capacity(
    key_capacity: usize,
    value_capacity: usize,
  ) -> ListOrderedMultimap<Key, Value, RandomState> {
    ListOrderedMultimap {
      build_hasher: RandomState::new(),
      keys: VecList::with_capacity(key_capacity),
      map: HashMap::with_capacity_and_hasher(key_capacity, DummyState),
      values: VecList::with_capacity(value_capacity),
    }
  }
}

impl<Key, Value, State> ListOrderedMultimap<Key, Value, State>
where
  State: BuildHasher,
{
  /// Creates a new multimap with the specified capacities and the given hash builder to hash keys.
  ///
  /// The multimap will be able to hold at least `key_capacity` keys and `value_capacity` values without reallocating. A
  /// capacity of 0 will result in no allocation for the respective container.
  ///
  /// The `state` is normally randomly generated and is designed to allow multimaps to be resistant to attacks that
  /// cause many collisions and very poor performance. Setting it manually using this function can expose a DoS attack
  /// vector.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use std::collections::hash_map::RandomState;
  ///
  /// let state = RandomState::new();
  /// let mut map = ListOrderedMultimap::with_capacity_and_hasher(10, 10, state);
  /// map.insert("key", "value");
  /// assert_eq!(map.keys_capacity(), 10);
  /// assert_eq!(map.values_capacity(), 10);
  /// ```
  #[must_use]
  pub fn with_capacity_and_hasher(
    key_capacity: usize,
    value_capacity: usize,
    state: State,
  ) -> ListOrderedMultimap<Key, Value, State> {
    ListOrderedMultimap {
      build_hasher: state,
      keys: VecList::with_capacity(key_capacity),
      map: HashMap::with_capacity_and_hasher(key_capacity, DummyState),
      values: VecList::with_capacity(value_capacity),
    }
  }

  /// Creates a new multimap with no capacity which will use the given hash builder to hash keys.
  ///
  /// The `state` is normally randomly generated and is designed to allow multimaps to be resistant to attacks that
  /// cause many collisions and very poor performance. Setting it manually using this function can expose a DoS attack
  /// vector.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use std::collections::hash_map::RandomState;
  ///
  /// let state = RandomState::new();
  /// let mut map = ListOrderedMultimap::with_hasher(state);
  /// map.insert("key", "value");
  /// ```
  #[must_use]
  pub fn with_hasher(state: State) -> ListOrderedMultimap<Key, Value, State> {
    ListOrderedMultimap {
      build_hasher: state,
      keys: VecList::new(),
      map: HashMap::with_hasher(DummyState),
      values: VecList::new(),
    }
  }
}

impl<Key, Value, State> ListOrderedMultimap<Key, Value, State> {
  /// Returns an immutable reference to the first key-value pair in the multimap
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.back(), None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.back(), Some((&"key", &"value")));
  /// ```
  #[must_use]
  pub fn back(&self) -> Option<(&Key, &Value)> {
    self.iter().next_back()
  }

  /// Returns an immutable reference to the first key-value pair in the multimap
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.back_mut(), None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.back_mut(), Some((&"key", &mut "value")));
  /// ```
  #[must_use]
  pub fn back_mut(&mut self) -> Option<(&Key, &mut Value)> {
    self.iter_mut().next_back()
  }

  /// Removes all keys and values from the multimap.
  ///
  /// Complexity: O(|K| + |V|) where |K| is the number of keys and |V| is the number of values.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  /// assert_eq!(map.keys_len(), 1);
  /// assert_eq!(map.values_len(), 1);
  ///
  /// map.clear();
  /// assert_eq!(map.keys_len(), 0);
  /// assert_eq!(map.values_len(), 0);
  /// ```
  pub fn clear(&mut self) {
    self.keys.clear();
    self.map.clear();
    self.values.clear();
  }

  /// Returns an immutable reference to the first key-value pair in the multimap
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.front(), None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.front(), Some((&"key", &"value")));
  /// ```
  #[must_use]
  pub fn front(&self) -> Option<(&Key, &Value)> {
    self.iter().next()
  }

  /// Returns an immutable reference to the first key-value pair in the multimap
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.front_mut(), None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.front_mut(), Some((&"key", &mut "value")));
  /// ```
  #[must_use]
  pub fn front_mut(&mut self) -> Option<(&Key, &mut Value)> {
    self.iter_mut().next()
  }

  /// Returns a reference to the multimap's [`BuildHasher`].
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
  /// let hasher = map.hasher();
  /// ```
  #[must_use]
  pub fn hasher(&self) -> &State {
    &self.build_hasher
  }

  /// Returns whether the multimap is empty.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert!(map.is_empty());
  ///
  /// map.insert("key1", "value");
  /// assert!(!map.is_empty());
  ///
  /// map.remove(&"key1");
  /// assert!(map.is_empty());
  /// ```
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.keys.is_empty()
  }

  /// Returns an iterator that yields immutable references to all key-value pairs in the multimap by insertion order.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key1", "value1");
  /// map.insert("key2", "value1");
  /// map.append(&"key1", "value2");
  /// map.append(&"key2", "value2");
  ///
  /// let mut iter = map.iter();
  /// assert_eq!(iter.size_hint(), (4, Some(4)));
  /// assert_eq!(iter.next(), Some((&"key1", &"value1")));
  /// assert_eq!(iter.next(), Some((&"key2", &"value1")));
  /// assert_eq!(iter.next(), Some((&"key1", &"value2")));
  /// assert_eq!(iter.next(), Some((&"key2", &"value2")));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn iter(&self) -> Iter<'_, Key, Value> {
    Iter {
      keys: &self.keys,
      iter: self.values.iter(),
    }
  }

  /// Returns an iterator that yields mutable references to all key-value pairs in the multimap by insertion order.
  ///
  /// Only the values are mutable, the keys are immutable.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key1", "value1");
  /// map.insert("key2", "value1");
  /// map.append(&"key1", "value2");
  /// map.append(&"key2", "value2");
  ///
  /// let mut iter = map.iter_mut();
  /// assert_eq!(iter.size_hint(), (4, Some(4)));
  ///
  /// let first = iter.next().unwrap();
  /// assert_eq!(first, (&"key1", &mut "value1"));
  /// *first.1 = "value3";
  ///
  /// assert_eq!(iter.next(), Some((&"key2", &mut "value1")));
  /// assert_eq!(iter.next(), Some((&"key1", &mut "value2")));
  /// assert_eq!(iter.next(), Some((&"key2", &mut "value2")));
  /// assert_eq!(iter.next(), None);
  ///
  /// assert_eq!(map.get(&"key1"), Some(&"value3"));
  /// ```
  #[must_use]
  pub fn iter_mut(&mut self) -> IterMut<'_, Key, Value> {
    IterMut {
      keys: &self.keys,
      iter: self.values.iter_mut(),
    }
  }

  /// Returns an iterator that yields immutable references to all keys in the multimap by insertion order.
  ///
  /// Insertion order of keys is determined by the order in which a given key is first inserted into the multimap with a
  /// value. Any subsequent insertions with that key without first removing it will not affect its ordering.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key1", "value");
  /// map.insert("key2", "value");
  /// map.insert("key3", "value");
  ///
  /// let mut keys = map.keys();
  /// assert_eq!(keys.next(), Some(&"key1"));
  /// assert_eq!(keys.next(), Some(&"key2"));
  /// assert_eq!(keys.next(), Some(&"key3"));
  /// assert_eq!(keys.next(), None);
  /// ```
  #[must_use]
  pub fn keys(&self) -> Keys<'_, Key> {
    Keys(self.keys.iter())
  }

  /// Returns the number of keys the multimap can hold without reallocating.
  ///
  /// This number is a lower bound, and the multimap may be able to hold more.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.keys_capacity(), 0);
  ///
  /// map.insert("key", "value");
  /// assert!(map.keys_capacity() > 0);
  /// ```
  #[must_use]
  pub fn keys_capacity(&self) -> usize {
    self.keys.capacity()
  }

  /// Returns the number of keys in the multimap.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.keys_len(), 0);
  ///
  /// map.insert("key1", "value");
  /// map.insert("key2", "value");
  /// map.insert("key3", "value");
  /// assert_eq!(map.keys_len(), 3);
  /// ```
  #[must_use]
  pub fn keys_len(&self) -> usize {
    self.keys.len()
  }

  /// Returns an iterator that yields immutable references to keys and all associated values with those keys as separate
  /// iterators. The order of yielded pairs will be the order in which the keys were first inserted into the multimap.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  ///
  /// let mut iter = map.pairs();
  ///
  /// let (key, mut values) = iter.next().unwrap();
  /// assert_eq!(key, &"key");
  /// assert_eq!(values.next(), Some(&"value1"));
  /// assert_eq!(values.next(), Some(&"value2"));
  /// assert_eq!(values.next(), None);
  /// ```
  #[must_use]
  pub fn pairs(&self) -> KeyValues<'_, Key, Value, State> {
    KeyValues {
      build_hasher: &self.build_hasher,
      keys: &self.keys,
      iter: self.keys.iter(),
      map: &self.map,
      values: &self.values,
    }
  }

  /// Returns an iterator that yields immutable references to keys and mutable references to all associated values with
  /// those keys as separate iterators. The order of yielded pairs will be the order in which the keys were first
  /// inserted into the multimap.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  ///
  /// let mut iter = map.pairs_mut();
  ///
  /// let (key, mut values) = iter.next().unwrap();
  /// assert_eq!(key, &"key");
  /// assert_eq!(values.next(), Some(&mut "value1"));
  /// assert_eq!(values.next(), Some(&mut "value2"));
  /// assert_eq!(values.next(), None);
  /// ```
  #[must_use]
  pub fn pairs_mut(&mut self) -> KeyValuesMut<'_, Key, Value, State> {
    KeyValuesMut {
      build_hasher: &self.build_hasher,
      keys: &self.keys,
      iter: self.keys.iter(),
      map: &self.map,
      values: &mut self.values,
    }
  }

  /// Reserves additional capacity such that more values can be stored in the multimap.
  ///
  /// If the existing capacity minus the current length is enough to satisfy the additional capacity, the capacity will
  /// remain unchanged.
  ///
  /// If the capacity is increased, the capacity may be increased by more than what was requested.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::with_capacity(1, 1);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.values_capacity(), 1);
  ///
  /// map.reserve_values(10);
  /// assert!(map.values_capacity() >= 11);
  /// ```
  pub fn reserve_values(&mut self, additional_capacity: usize) {
    self.values.reserve(additional_capacity);
  }

  /// Returns an iterator that yields immutable references to all values in the multimap by insertion order.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key1", "value1");
  /// map.insert("key2", "value1");
  /// map.append(&"key1", "value2");
  /// map.append(&"key2", "value2");
  ///
  /// let mut iter = map.values();
  /// assert_eq!(iter.size_hint(), (4, Some(4)));
  /// assert_eq!(iter.next(), Some(&"value1"));
  /// assert_eq!(iter.next(), Some(&"value1"));
  /// assert_eq!(iter.next(), Some(&"value2"));
  /// assert_eq!(iter.next(), Some(&"value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn values(&self) -> Values<'_, Key, Value> {
    Values(self.values.iter())
  }

  /// Returns an iterator that yields mutable references to all values in the multimap by insertion order.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key1", "value1");
  /// map.insert("key2", "value1");
  /// map.append(&"key1", "value2");
  /// map.append(&"key2", "value2");
  ///
  /// let mut iter = map.values_mut();
  /// assert_eq!(iter.size_hint(), (4, Some(4)));
  ///
  /// let first = iter.next().unwrap();
  /// assert_eq!(first, &mut "value1");
  /// *first = "value3";
  ///
  /// assert_eq!(iter.next(), Some(&mut "value1"));
  /// assert_eq!(iter.next(), Some(&mut "value2"));
  /// assert_eq!(iter.next(), Some(&mut "value2"));
  /// assert_eq!(iter.next(), None);
  ///
  /// assert_eq!(map.get(&"key1"), Some(&"value3"));
  /// ```
  #[must_use]
  pub fn values_mut(&mut self) -> ValuesMut<'_, Key, Value> {
    ValuesMut(self.values.iter_mut())
  }

  /// Returns the number of values the multimap can hold without reallocating.
  ///
  /// This number is a lower bound, and the multimap may be able to hold more.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.values_capacity(), 0);
  ///
  /// map.insert("key", "value");
  /// assert!(map.values_capacity() > 0);
  /// ```
  #[must_use]
  pub fn values_capacity(&self) -> usize {
    self.values.capacity()
  }

  /// Returns the total number of values in the multimap across all keys.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.values_len(), 0);
  ///
  /// map.insert("key1", "value1");
  /// assert_eq!(map.values_len(), 1);
  ///
  /// map.append("key1", "value2");
  /// assert_eq!(map.values_len(), 2);
  /// ```
  #[must_use]
  pub fn values_len(&self) -> usize {
    self.values.len()
  }
}

impl<Key, Value, State> ListOrderedMultimap<Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  /// Appends a value to the list of values associated with the given key.
  ///
  /// If the key is not already in the multimap, this will be identical to an insert and the return value will be
  /// `false`. Otherwise, `true` will be returned.
  ///
  /// Complexity: amortized O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// let already_exists = map.append("key", "value");
  /// assert!(!already_exists);
  /// assert_eq!(map.values_len(), 1);
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  ///
  /// let already_exists = map.append("key", "value2");
  /// assert!(already_exists);
  /// assert_eq!(map.values_len(), 2);
  /// ```
  pub fn append(&mut self, key: Key, value: Value) -> bool {
    let hash = hash_key(&self.build_hasher, &key);
    let entry = raw_entry_mut(&self.keys, &mut self.map, hash, &key);
    let build_hasher = &self.build_hasher;

    match entry {
      RawEntryMut::Occupied(mut entry) => {
        let key_index = entry.key();
        let mut value_entry = ValueEntry::new(*key_index, value);
        let map_entry = entry.get_mut();
        value_entry.previous_index = Some(map_entry.tail_index);
        let index = self.values.push_back(value_entry);
        self
          .values
          .get_mut(map_entry.tail_index)
          .unwrap()
          .next_index = Some(index);
        map_entry.append(index);
        true
      }
      RawEntryMut::Vacant(entry) => {
        let key_index = self.keys.push_back(key);
        let value_entry = ValueEntry::new(key_index, value);
        let index = self.values.push_back(value_entry);
        let keys = &self.keys;
        let _ = entry.insert_with_hasher(hash, key_index, MapEntry::new(index), |&key_index| {
          let key = keys.get(key_index).unwrap();
          hash_key(build_hasher, key)
        });
        false
      }
    }
  }

  /// Returns whether the given key is in the multimap.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert!(!map.contains_key(&"key"));
  /// map.insert("key", "value");
  /// assert!(map.contains_key(&"key"));
  /// ```
  #[must_use]
  pub fn contains_key<KeyQuery>(&self, key: &KeyQuery) -> bool
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);
    raw_entry(&self.keys, &self.map, hash, key).is_some()
  }

  /// Returns whether the given key is in the multimap.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// let value = map.entry("key").or_insert("value");
  /// assert_eq!(value, &"value");
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  /// ```
  #[must_use]
  pub fn entry(&mut self, key: Key) -> Entry<'_, Key, Value, State> {
    let hash = hash_key(&self.build_hasher, &key);

    // TODO: This ugliness arises from borrow checking issues which seems to happen when the vacant entry is created in
    // the match block further below for `Vacant` even though it should be perfectly safe. Is there a better way to do
    // this?
    if !self.contains_key(&key) {
      Entry::Vacant(VacantEntry {
        build_hasher: &self.build_hasher,
        hash,
        key,
        keys: &mut self.keys,
        map: &mut self.map,
        values: &mut self.values,
      })
    } else {
      match raw_entry_mut(&self.keys, &mut self.map, hash, &key) {
        RawEntryMut::Occupied(entry) => Entry::Occupied(OccupiedEntry {
          entry,
          keys: &mut self.keys,
          values: &mut self.values,
        }),
        _ => panic!("expected occupied entry"),
      }
    }
  }

  /// Returns the number of values associated with a key.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.entry_len(&"key"), 0);
  ///
  /// map.insert("key", "value1");
  /// assert_eq!(map.entry_len(&"key"), 1);
  ///
  /// map.append(&"key", "value2");
  /// assert_eq!(map.entry_len(&"key"), 2);
  /// ```
  #[must_use]
  pub fn entry_len<KeyQuery>(&self, key: &KeyQuery) -> usize
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);

    match raw_entry(&self.keys, &self.map, hash, key) {
      Some((_, map_entry)) => map_entry.length,
      None => 0,
    }
  }

  /// Returns an immutable reference to the first value, by insertion order, associated with the given key, or `None` if
  /// the key is not in the multimap.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
  /// assert_eq!(map.get(&"key"), None);
  ///
  /// ```
  #[must_use]
  pub fn get<KeyQuery>(&self, key: &KeyQuery) -> Option<&Value>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);
    let (_, map_entry) = raw_entry(&self.keys, &self.map, hash, key)?;
    self
      .values
      .get(map_entry.head_index)
      .map(|entry| &entry.value)
  }

  /// Returns an iterator that yields immutable references to all values associated with the given key by insertion
  /// order.
  ///
  /// If the key is not in the multimap, the iterator will yield no values.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  /// map.append("key", "value2");
  ///
  /// let mut iter = map.get_all(&"key");
  /// assert_eq!(iter.next(), Some(&"value"));
  /// assert_eq!(iter.next(), Some(&"value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn get_all<KeyQuery>(&self, key: &KeyQuery) -> EntryValues<'_, Key, Value>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);

    match raw_entry(&self.keys, &self.map, hash, key) {
      Some((_, map_entry)) => EntryValues::from_map_entry(&self.values, map_entry),
      None => EntryValues::empty(&self.values),
    }
  }

  /// Returns an iterator that yields mutable references to all values associated with the given key by insertion order.
  ///
  /// If the key is not in the multimap, the iterator will yield no values.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  ///
  /// let mut iter = map.get_all_mut(&"key");
  ///
  /// let first = iter.next().unwrap();
  /// assert_eq!(first, &mut "value1");
  /// *first = "value3";
  ///
  /// assert_eq!(iter.next(), Some(&mut "value2"));
  /// assert_eq!(iter.next(), None);
  ///
  /// assert_eq!(map.get(&"key"), Some(&"value3"));
  /// ```
  #[must_use]
  pub fn get_all_mut<KeyQuery>(&mut self, key: &KeyQuery) -> EntryValuesMut<'_, Key, Value>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);

    match raw_entry(&self.keys, &self.map, hash, key) {
      Some((_, map_entry)) => EntryValuesMut::from_map_entry(&mut self.values, map_entry),
      None => EntryValuesMut::empty(&mut self.values),
    }
  }

  /// Returns a mutable reference to the first value, by insertion order, associated with the given key, or `None` if
  /// the key is not in the multimap.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert_eq!(map.get(&"key"), None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  ///
  /// let mut value = map.get_mut(&"key").unwrap();
  /// *value = "value2";
  ///
  /// assert_eq!(map.get(&"key"), Some(&"value2"));
  /// ```
  #[must_use]
  pub fn get_mut<KeyQuery>(&mut self, key: &KeyQuery) -> Option<&mut Value>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);
    let (_, map_entry) = raw_entry(&self.keys, &self.map, hash, key)?;
    self
      .values
      .get_mut(map_entry.head_index)
      .map(|entry| &mut entry.value)
  }

  /// Inserts the key-value pair into the multimap and returns the first value, by insertion order, that was already
  /// associated with the key.
  ///
  /// If the key is not already in the multimap, `None` will be returned. If the key is already in the multimap, the
  /// insertion ordering of the keys will remain unchanged.
  ///
  /// Complexity: O(1) amortized
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert!(map.is_empty());
  ///
  /// let old_value = map.insert("key", "value");
  /// assert!(old_value.is_none());
  /// assert_eq!(map.values_len(), 1);
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  ///
  /// let old_value = map.insert("key", "value2");
  /// assert_eq!(old_value, Some("value"));
  /// assert_eq!(map.values_len(), 1);
  /// assert_eq!(map.get(&"key"), Some(&"value2"));
  /// ```
  pub fn insert(&mut self, key: Key, value: Value) -> Option<Value> {
    self.insert_all(key, value).next()
  }

  /// Inserts the key-value pair into the multimap and returns an iterator that yields all values previously associated
  /// with the key by insertion order.
  ///
  /// If the key is not already in the multimap, the iterator will yield no values.If the key is already in the
  /// multimap, the insertion ordering of the keys will remain unchanged.
  ///
  /// Complexity: O(1) amortized
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// assert!(map.is_empty());
  ///
  /// {
  ///   let mut old_values = map.insert_all("key", "value");
  ///   assert_eq!(old_values.next(), None);
  /// }
  ///
  /// assert_eq!(map.values_len(), 1);
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  ///
  /// map.append("key", "value2");
  ///
  /// {
  ///   let mut old_values = map.insert_all("key", "value3");
  ///   assert_eq!(old_values.next(), Some("value"));
  ///   assert_eq!(old_values.next(), Some("value2"));
  ///   assert_eq!(old_values.next(), None);
  /// }
  ///
  /// assert_eq!(map.values_len(), 1);
  /// assert_eq!(map.get(&"key"), Some(&"value3"));
  /// ```
  pub fn insert_all(&mut self, key: Key, value: Value) -> EntryValuesDrain<'_, Key, Value> {
    let hash = hash_key(&self.build_hasher, &key);
    let entry = raw_entry_mut(&self.keys, &mut self.map, hash, &key);
    let build_hasher = &self.build_hasher;

    match entry {
      RawEntryMut::Occupied(mut entry) => {
        let key_index = entry.key();
        let value_entry = ValueEntry::new(*key_index, value);
        let index = self.values.push_back(value_entry);
        let map_entry = entry.get_mut();
        let iter = EntryValuesDrain::from_map_entry(&mut self.values, map_entry);
        map_entry.reset(index);
        iter
      }
      RawEntryMut::Vacant(entry) => {
        let key_index = self.keys.push_back(key);
        let value_entry = ValueEntry::new(key_index, value);
        let index = self.values.push_back(value_entry);
        let keys = &self.keys;
        let _ = entry.insert_with_hasher(hash, key_index, MapEntry::new(index), |&key_index| {
          let key = keys.get(key_index).unwrap();
          hash_key(build_hasher, key)
        });
        EntryValuesDrain::empty(&mut self.values)
      }
    }
  }

  /// Reorganizes the multimap to ensure maximum spatial locality and changes the key and value capacities to the
  /// provided values.
  ///
  /// This function can be used to actually increase the capacity of the multimap.
  ///
  /// Complexity: O(|K| + |V|) where |K| is the number of keys and |V| is the number of values.
  ///
  /// # Panics
  ///
  /// Panics if either of the given minimum capacities are less than their current respective lengths.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::with_capacity(10, 10);
  ///
  /// map.insert("key1", "value1");
  /// map.insert("key2", "value2");
  /// map.append("key2", "value3");
  /// map.append("key1", "value4");
  /// map.pack_to(5, 5);
  ///
  /// assert_eq!(map.keys_capacity(), 5);
  /// assert_eq!(map.keys_len(), 2);
  /// assert_eq!(map.values_capacity(), 5);
  /// assert_eq!(map.values_len(), 4);
  /// ```
  #[cfg(feature = "std")]
  pub fn pack_to(&mut self, keys_minimum_capacity: usize, values_minimum_capacity: usize)
  where
    State: Default,
  {
    assert!(
      keys_minimum_capacity >= self.keys_len(),
      "cannot pack multimap keys lower than current length"
    );
    assert!(
      values_minimum_capacity >= self.values_len(),
      "cannot pack multimap values lower than current length"
    );

    let key_map = self.keys.pack_to(keys_minimum_capacity);
    let value_map = self.values.pack_to(values_minimum_capacity);
    let mut map = HashMap::with_capacity_and_hasher(keys_minimum_capacity, DummyState);
    let build_hasher = &self.build_hasher;

    for value_entry in self.values.iter_mut() {
      value_entry.key_index = key_map[&value_entry.key_index];
      value_entry.next_index = value_entry.next_index.map(|index| value_map[&index]);
      value_entry.previous_index = value_entry.previous_index.map(|index| value_map[&index]);
    }

    for (key_index, mut map_entry) in self.map.drain() {
      map_entry.head_index = value_map[&map_entry.head_index];
      map_entry.tail_index = value_map[&map_entry.tail_index];
      let key_index = key_map[&key_index];
      let key = self.keys.get(key_index).unwrap();
      let hash = hash_key(&self.build_hasher, key);

      match map.raw_entry_mut().from_hash(hash, |_| false) {
        RawEntryMut::Vacant(entry) => {
          let keys = &self.keys;
          let _ = entry.insert_with_hasher(hash, key_index, map_entry, |&key_index| {
            let key = keys.get(key_index).unwrap();
            hash_key(build_hasher, key)
          });
        }
        _ => panic!("expected vacant entry"),
      }
    }

    self.map = map;
  }

  /// Reorganizes the multimap to ensure maximum spatial locality and removes any excess key and value capacity.
  ///
  /// Complexity: O(|K| + |V|) where |K| is the number of keys and |V| is the number of values.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::with_capacity(5, 5);
  ///
  /// map.insert("key1", "value1");
  /// map.insert("key2", "value2");
  /// map.append("key2", "value3");
  /// map.append("key1", "value4");
  /// map.pack_to_fit();
  ///
  /// assert_eq!(map.keys_capacity(), 2);
  /// assert_eq!(map.keys_len(), 2);
  /// assert_eq!(map.values_capacity(), 4);
  /// assert_eq!(map.values_len(), 4);
  /// ```
  #[cfg(feature = "std")]
  pub fn pack_to_fit(&mut self)
  where
    State: Default,
  {
    self.pack_to(self.keys_len(), self.values_len());
  }

  /// Removes the last key-value pair to have been inserted.
  ///
  /// Because a single key can be associated with many values, the key returned by this function is a [`KeyWrapper`]
  /// which can be either owned or borrowed. If the value removed was the only value associated with the key, then the
  /// key will be returned. Otherwise, a reference to the key will be returned.
  ///
  /// This function along with [`ListOrderedMultimap::pop_front`] act as replacements for a drain iterator since an
  /// iterator cannot be done over [`KeyWrapper`].
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::KeyWrapper;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  ///
  /// let (key, value) = map.pop_back().unwrap();
  /// assert_eq!(key, KeyWrapper::Borrowed(&"key"));
  /// assert_eq!(&value, &"value2");
  ///
  /// let (key, value) = map.pop_back().unwrap();
  /// assert_eq!(key, KeyWrapper::Owned("key"));
  /// assert_eq!(&value, &"value1");
  /// ```
  pub fn pop_back(&mut self) -> Option<(KeyWrapper<'_, Key>, Value)> {
    let value_entry = self.values.pop_back()?;

    let key_wrapper = match value_entry.previous_index {
      Some(previous_index) => {
        let key = self.keys.get(value_entry.key_index).unwrap();
        let hash = hash_key(&self.build_hasher, &key);

        let mut entry = match raw_entry_mut(&self.keys, &mut self.map, hash, key) {
          RawEntryMut::Occupied(entry) => entry,
          _ => panic!("expected occupied entry in internal map"),
        };
        let map_entry = entry.get_mut();
        map_entry.length -= 1;
        map_entry.tail_index = previous_index;

        let previous_value_entry = self.values.get_mut(previous_index).unwrap();
        previous_value_entry.next_index = None;

        KeyWrapper::Borrowed(key)
      }
      None => {
        let key = self.keys.remove(value_entry.key_index).unwrap();
        let hash = hash_key(&self.build_hasher, &key);

        match raw_entry_mut_empty(&self.keys, &mut self.map, hash) {
          RawEntryMut::Occupied(entry) => {
            let _ = entry.remove();
          }
          _ => panic!("expectd occupied entry in internal map"),
        }

        KeyWrapper::Owned(key)
      }
    };

    Some((key_wrapper, value_entry.value))
  }

  /// Removes the first key-value pair to have been inserted.
  ///
  /// Because a single key can be associated with many values, the key returned by this function is a [`KeyWrapper`]
  /// which can be either owned or borrowed. If the value removed was the only value associated with the key, then the
  /// key will be returned. Otherwise, a reference to the key will be returned.
  ///
  /// This function along with [`ListOrderedMultimap::pop_back`] act as replacements for a drain iterator since an
  /// iterator cannot be done over [`KeyWrapper`].
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::KeyWrapper;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  ///
  /// let (key, value) = map.pop_front().unwrap();
  /// assert_eq!(key, KeyWrapper::Borrowed(&"key"));
  /// assert_eq!(&value, &"value1");
  ///
  /// let (key, value) = map.pop_front().unwrap();
  /// assert_eq!(key, KeyWrapper::Owned("key"));
  /// assert_eq!(&value, &"value2");
  /// ```
  pub fn pop_front(&mut self) -> Option<(KeyWrapper<'_, Key>, Value)> {
    let value_entry = self.values.pop_front()?;

    let key_wrapper = match value_entry.next_index {
      Some(next_index) => {
        let key = self.keys.get(value_entry.key_index).unwrap();
        let hash = hash_key(&self.build_hasher, &key);

        let mut entry = match raw_entry_mut(&self.keys, &mut self.map, hash, key) {
          RawEntryMut::Occupied(entry) => entry,
          _ => panic!("expected occupied entry in internal map"),
        };
        let map_entry = entry.get_mut();
        map_entry.length -= 1;
        map_entry.head_index = next_index;

        let next_value_entry = self.values.get_mut(next_index).unwrap();
        next_value_entry.previous_index = None;

        KeyWrapper::Borrowed(key)
      }
      None => {
        let key = self.keys.remove(value_entry.key_index).unwrap();
        let hash = hash_key(&self.build_hasher, &key);

        match raw_entry_mut_empty(&self.keys, &mut self.map, hash) {
          RawEntryMut::Occupied(entry) => {
            let _ = entry.remove();
          }
          _ => panic!("expectd occupied entry in internal map"),
        }

        KeyWrapper::Owned(key)
      }
    };

    Some((key_wrapper, value_entry.value))
  }

  /// Removes all values associated with the given key from the map and returns the first value by insertion order.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// let removed_value = map.remove(&"key");
  /// assert_eq!(removed_value, None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  ///
  /// let removed_value = map.remove(&"key");
  /// assert_eq!(removed_value, Some("value"));
  /// assert_eq!(map.get(&"key"), None);
  /// ```
  pub fn remove<KeyQuery>(&mut self, key: &KeyQuery) -> Option<Value>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    self.remove_entry(key).map(|(_, value)| value)
  }

  /// Removes all values associated with the given key from the map and returns an iterator that yields those values.
  ///
  /// If the key is not already in the map, the iterator will yield no values.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// {
  ///     let mut removed_values = map.remove_all(&"key");
  ///     assert_eq!(removed_values.next(), None);
  /// }
  ///
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  /// assert_eq!(map.get(&"key"), Some(&"value1"));
  ///
  /// {
  ///     let mut removed_values = map.remove_all(&"key");
  ///     assert_eq!(removed_values.next(), Some("value1"));
  ///     assert_eq!(removed_values.next(), Some("value2"));
  ///     assert_eq!(removed_values.next(), None);
  /// }
  ///
  /// assert_eq!(map.get(&"key"), None);
  /// ```
  pub fn remove_all<KeyQuery>(&mut self, key: &KeyQuery) -> EntryValuesDrain<'_, Key, Value>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);
    let entry = raw_entry_mut(&self.keys, &mut self.map, hash, key);

    match entry {
      RawEntryMut::Occupied(entry) => {
        let (key_index, map_entry) = entry.remove_entry();
        let _ = self.keys.remove(key_index).unwrap();
        EntryValuesDrain::from_map_entry(&mut self.values, &map_entry)
      }
      RawEntryMut::Vacant(_) => EntryValuesDrain::empty(&mut self.values),
    }
  }

  /// Removes all values associated with the given key from the map and returns the key and first value.
  ///
  /// If the key is not already in the map, then `None` will be returned.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// let entry = map.remove_entry(&"key");
  /// assert_eq!(entry, None);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  ///
  /// let entry = map.remove_entry(&"key");
  /// assert_eq!(entry, Some(("key", "value")));
  /// assert_eq!(map.get(&"key"), None);
  /// ```
  pub fn remove_entry<KeyQuery>(&mut self, key: &KeyQuery) -> Option<(Key, Value)>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let (key, mut iter) = self.remove_entry_all(key)?;
    Some((key, iter.next().unwrap()))
  }

  /// Removes all values associated with the given key from the map and returns the key and an iterator that yields
  /// those values.
  ///
  /// If the key is not already in the map, then `None` will be returned.
  ///
  /// Complexity: O(1)
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// {
  ///     let entry = map.remove_entry_all(&"key");
  ///     assert!(entry.is_none());
  /// }
  ///
  /// map.insert("key", "value1");
  /// map.append("key", "value2");
  /// assert_eq!(map.get(&"key"), Some(&"value1"));
  ///
  /// {
  ///     let (key, mut iter) = map.remove_entry_all(&"key").unwrap();
  ///     assert_eq!(key, "key");
  ///     assert_eq!(iter.next(), Some("value1"));
  ///     assert_eq!(iter.next(), Some("value2"));
  ///     assert_eq!(iter.next(), None);
  /// }
  ///
  /// assert_eq!(map.get(&"key"), None);
  /// ```
  pub fn remove_entry_all<KeyQuery>(
    &mut self,
    key: &KeyQuery,
  ) -> Option<(Key, EntryValuesDrain<'_, Key, Value>)>
  where
    Key: Borrow<KeyQuery>,
    KeyQuery: ?Sized + Eq + Hash,
  {
    let hash = hash_key(&self.build_hasher, &key);
    let entry = raw_entry_mut(&self.keys, &mut self.map, hash, key);

    match entry {
      RawEntryMut::Occupied(entry) => {
        let (key_index, map_entry) = entry.remove_entry();
        let key = self.keys.remove(key_index).unwrap();
        let iter = EntryValuesDrain::from_map_entry(&mut self.values, &map_entry);
        Some((key, iter))
      }
      _ => None,
    }
  }

  /// Reserves additional capacity such that more keys can be stored in the multimap.
  ///
  /// If the existing capacity minus the current length is enough to satisfy the additional capacity, the capacity will
  /// remain unchanged.
  ///
  /// If the capacity is increased, the capacity may be increased by more than what was requested.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::with_capacity(1, 1);
  ///
  /// map.insert("key", "value");
  /// assert_eq!(map.keys_capacity(), 1);
  ///
  /// map.reserve_keys(10);
  /// assert!(map.keys_capacity() >= 11);
  /// assert_eq!(map.get(&"key"), Some(&"value"));
  /// ```
  pub fn reserve_keys(&mut self, additional_capacity: usize) {
    if self.keys.capacity() - self.keys.len() >= additional_capacity {
      return;
    }

    let capacity = self.map.capacity() + additional_capacity;
    let mut map = HashMap::with_capacity_and_hasher(capacity, DummyState);

    for (key_index, map_entry) in self.map.drain() {
      let key = self.keys.get(key_index).unwrap();
      let hash = hash_key(&self.build_hasher, key);
      let entry = match raw_entry_mut(&self.keys, &mut map, hash, key) {
        RawEntryMut::Vacant(entry) => entry,
        _ => panic!("expected vacant entry"),
      };
      let _ = entry.insert_hashed_nocheck(hash, key_index, map_entry);
    }

    self.keys.reserve(additional_capacity);
    self.map = map;
  }

  /// Keeps all key-value pairs that satisfy the given predicate function.
  ///
  /// Complexity: O(|V|) where |V| is the number of values
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// map.insert("key1", 1);
  /// map.insert("key2", 5);
  /// map.append("key1", -1);
  /// map.insert("key3", -10);
  ///
  /// map.retain(|_, &mut value| value >= 0);
  ///
  /// let mut iter = map.iter();
  /// assert_eq!(iter.next(), Some((&"key1", &1)));
  /// assert_eq!(iter.next(), Some((&"key2", &5)));
  /// assert_eq!(iter.next(), None);
  /// ```
  pub fn retain<Function>(&mut self, function: Function)
  where
    Function: FnMut(&Key, &mut Value) -> bool,
  {
    ListOrderedMultimap::retain_helper(
      &self.build_hasher,
      &mut self.keys,
      &mut self.map,
      &mut self.values,
      function,
    );
  }

  /// Helper function for [`ListOrderedMultimap::retain`] to deal with borrowing issues.
  fn retain_helper<'map, Function>(
    build_hasher: &'map State,
    keys: &'map mut VecList<Key>,
    map: &'map mut HashMap<Index<Key>, MapEntry<Key, Value>, DummyState>,
    values: &'map mut VecList<ValueEntry<Key, Value>>,
    mut function: Function,
  ) where
    Function: FnMut(&Key, &mut Value) -> bool,
  {
    let mut post_updates = vec![];

    values.retain(|value_entry| {
      let key = keys.get(value_entry.key_index).unwrap();

      if function(key, &mut value_entry.value) {
        true
      } else {
        let hash = hash_key(build_hasher, key);
        let mut entry = match raw_entry_mut(keys, map, hash, key) {
          RawEntryMut::Occupied(entry) => entry,
          _ => panic!("expected occupied entry in internal map"),
        };

        if value_entry.previous_index.is_none() && value_entry.next_index.is_none() {
          let _ = entry.remove();
          let _ = keys.remove(value_entry.key_index);
        } else {
          let map_entry = entry.get_mut();
          map_entry.length -= 1;

          if let Some(previous_index) = value_entry.previous_index {
            post_updates.push((previous_index, None, Some(value_entry.next_index)));
          } else {
            map_entry.head_index = value_entry.next_index.unwrap();
          }

          if let Some(next_index) = value_entry.next_index {
            post_updates.push((next_index, Some(value_entry.previous_index), None));
          } else {
            map_entry.tail_index = value_entry.previous_index.unwrap();
          }
        }

        false
      }
    });

    for (index, new_previous_index, new_next_index) in post_updates {
      let value_entry = values.get_mut(index).unwrap();

      if let Some(new_previous_index) = new_previous_index {
        value_entry.previous_index = new_previous_index;
      }

      if let Some(new_next_index) = new_next_index {
        value_entry.next_index = new_next_index;
      }
    }
  }
}

impl<Key, Value, State> Debug for ListOrderedMultimap<Key, Value, State>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.debug_map().entries(self.iter()).finish()
  }
}

#[cfg(feature = "std")]
impl<Key, Value> Default for ListOrderedMultimap<Key, Value, RandomState> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Key, Value, State> Eq for ListOrderedMultimap<Key, Value, State>
where
  Key: Eq,
  Value: PartialEq,
{
}

impl<Key, Value, State> Extend<(Key, Value)> for ListOrderedMultimap<Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  fn extend<Iter>(&mut self, iter: Iter)
  where
    Iter: IntoIterator<Item = (Key, Value)>,
  {
    let iter = iter.into_iter();
    self.reserve_values(iter.size_hint().0);

    for (key, value) in iter {
      let _ = self.append(key, value);
    }
  }
}

impl<'a, Key, Value, State> Extend<(&'a Key, &'a Value)> for ListOrderedMultimap<Key, Value, State>
where
  Key: Copy + Eq + Hash,
  Value: Copy,
  State: BuildHasher,
{
  fn extend<Iter>(&mut self, iter: Iter)
  where
    Iter: IntoIterator<Item = (&'a Key, &'a Value)>,
  {
    self.extend(iter.into_iter().map(|(&key, &value)| (key, value)));
  }
}

impl<Key, Value, State> FromIterator<(Key, Value)> for ListOrderedMultimap<Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher + Default,
{
  fn from_iter<Iter>(iter: Iter) -> Self
  where
    Iter: IntoIterator<Item = (Key, Value)>,
  {
    let mut map = ListOrderedMultimap::with_hasher(State::default());
    map.extend(iter);
    map
  }
}

impl<Key, Value, State> IntoIterator for ListOrderedMultimap<Key, Value, State>
where
  Key: Clone,
{
  type IntoIter = IntoIter<Key, Value>;
  type Item = (Key, Value);

  fn into_iter(self) -> Self::IntoIter {
    IntoIter {
      keys: self.keys,
      iter: self.values.into_iter(),
    }
  }
}

impl<'map, Key, Value, State> IntoIterator for &'map ListOrderedMultimap<Key, Value, State> {
  type IntoIter = Iter<'map, Key, Value>;
  type Item = (&'map Key, &'map Value);

  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

impl<'map, Key, Value, State> IntoIterator for &'map mut ListOrderedMultimap<Key, Value, State> {
  type IntoIter = IterMut<'map, Key, Value>;
  type Item = (&'map Key, &'map mut Value);

  fn into_iter(self) -> Self::IntoIter {
    self.iter_mut()
  }
}

impl<Key, Value, State> PartialEq for ListOrderedMultimap<Key, Value, State>
where
  Key: PartialEq,
  Value: PartialEq,
{
  fn eq(&self, other: &ListOrderedMultimap<Key, Value, State>) -> bool {
    if self.keys_len() != other.keys_len() || self.values_len() != other.values_len() {
      return false;
    }

    self.iter().eq(other.iter())
  }
}

/// A wrapper around a key that is either borrowed or owned.
///
/// This type is similar to [`std::borrow::Cow`] but does not require a [`Clone`] trait bound on the key.
#[allow(single_use_lifetimes)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum KeyWrapper<'map, Key> {
  /// An immutable reference to a key. This implies that the key is still associated to at least one value in the
  /// multimap.
  Borrowed(&'map Key),

  /// An owned key. This will occur when a key is no longer associated with any values in the multimap.
  Owned(Key),
}

impl<Key> KeyWrapper<'_, Key> {
  /// If the key wrapped is owned, it is returned. Otherwise, the borrowed key is cloned and returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::list_ordered_multimap::KeyWrapper;
  ///
  /// let borrowed = KeyWrapper::Borrowed(&0);
  /// assert_eq!(borrowed.into_owned(), 0);
  ///
  /// let owned = KeyWrapper::Owned(0);
  /// assert_eq!(borrowed.into_owned(), 0);
  /// ```
  #[must_use]
  pub fn into_owned(self) -> Key
  where
    Key: Clone,
  {
    match self {
      KeyWrapper::Borrowed(key) => key.clone(),
      KeyWrapper::Owned(key) => key,
    }
  }

  /// Returns whether the wrapped key is borrowed.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::list_ordered_multimap::KeyWrapper;
  ///
  /// let borrowed = KeyWrapper::Borrowed(&0);
  /// assert!(borrowed.is_borrowed());
  ///
  /// let owned = KeyWrapper::Owned(0);
  /// assert!(!owned.is_borrowed());
  /// ```
  #[must_use]
  pub fn is_borrowed(&self) -> bool {
    matches!(self, KeyWrapper::Borrowed(_))
  }

  /// Returns whether the wrapped key is owned.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::list_ordered_multimap::KeyWrapper;
  ///
  /// let borrowed = KeyWrapper::Borrowed(&0);
  /// assert!(!borrowed.is_owned());
  ///
  /// let owned = KeyWrapper::Owned(0);
  /// assert!(owned.is_owned());
  /// ```
  #[must_use]
  pub fn is_owned(&self) -> bool {
    matches!(self, KeyWrapper::Owned(_))
  }
}

/// The value type of the internal hash map.
#[derive(Clone)]
pub(crate) struct MapEntry<Key, Value> {
  /// The index of the first value for this entry.
  head_index: Index<ValueEntry<Key, Value>>,

  /// The number of values for this entry.
  length: usize,

  /// The index of the last value for this entry.
  tail_index: Index<ValueEntry<Key, Value>>,
}

impl<Key, Value> MapEntry<Key, Value> {
  /// Convenience function for adding a new value to the entry.
  pub fn append(&mut self, index: Index<ValueEntry<Key, Value>>) {
    self.length += 1;
    self.tail_index = index;
  }

  /// Convenience function for creating a new multimap entry.
  #[must_use]
  pub fn new(index: Index<ValueEntry<Key, Value>>) -> Self {
    MapEntry {
      head_index: index,
      length: 1,
      tail_index: index,
    }
  }

  /// Convenience function for resetting the entry to contain only one value.
  pub fn reset(&mut self, index: Index<ValueEntry<Key, Value>>) {
    self.head_index = index;
    self.length = 1;
    self.tail_index = index;
  }
}

/// The value entry that is contained within the internal values list.
#[derive(Clone)]
pub(crate) struct ValueEntry<Key, Value> {
  /// The index of the key in the key list for this entry.
  key_index: Index<Key>,

  /// The index of the next value with the same key.
  next_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The index of the previous value with the same key.
  previous_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The actual value stored in this entry.
  value: Value,
}

impl<Key, Value> ValueEntry<Key, Value> {
  /// Convenience function for creating a new value entry.
  #[must_use]
  pub fn new(key_index: Index<Key>, value: Value) -> Self {
    ValueEntry {
      key_index,
      next_index: None,
      previous_index: None,
      value,
    }
  }
}

/// A view into a single entry in the multimap, which may either be vacant or occupied.
pub enum Entry<'map, Key, Value, State = RandomState> {
  /// An occupied entry associated with one or more values.
  Occupied(OccupiedEntry<'map, Key, Value>),

  /// A vacant entry with no associated values.
  Vacant(VacantEntry<'map, Key, Value, State>),
}

impl<'map, Key, Value, State> Entry<'map, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  /// Calls the given function with a mutable reference to the first value of this entry, by insertion order, if it is
  /// vacant, otherwise this function is a no-op.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// map.entry("key")
  ///     .and_modify(|value| *value += 1)
  ///     .or_insert(42);
  /// assert_eq!(map.get(&"key"), Some(&42));
  ///
  /// map.entry("key")
  ///     .and_modify(|value| *value += 1)
  ///     .or_insert(42);
  /// assert_eq!(map.get(&"key"), Some(&43));
  /// ```
  pub fn and_modify<Function>(self, function: Function) -> Self
  where
    Function: FnOnce(&mut Value),
  {
    match self {
      Entry::Occupied(mut entry) => {
        function(entry.get_mut());
        Entry::Occupied(entry)
      }
      Entry::Vacant(entry) => Entry::Vacant(entry),
    }
  }

  /// If the entry is vacant, the given value will be inserted into it and a mutable reference to that value will be
  /// returned. Otherwise, a mutable reference to the first value, by insertion order, will be returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let value = map.entry("key").or_insert("value2");
  /// assert_eq!(value, &"value1");
  ///
  /// let value = map.entry("key2").or_insert("value2");
  /// assert_eq!(value, &"value2");
  /// ```
  pub fn or_insert(self, value: Value) -> &'map mut Value {
    match self {
      Entry::Occupied(entry) => entry.into_mut(),
      Entry::Vacant(entry) => entry.insert(value),
    }
  }

  /// If the entry is vacant, the given value will be inserted into it and the new occupied entry will be returned.
  /// Otherwise, the existing occupied entry will be returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let entry = map.entry("key").or_insert_entry("value2");
  /// assert_eq!(entry.into_mut(), &"value1");
  ///
  /// let entry = map.entry("key2").or_insert_entry("value2");
  /// assert_eq!(entry.into_mut(), &"value2");
  /// ```
  pub fn or_insert_entry(self, value: Value) -> OccupiedEntry<'map, Key, Value> {
    match self {
      Entry::Occupied(entry) => entry,
      Entry::Vacant(entry) => entry.insert_entry(value),
    }
  }

  /// If the entry is vacant, the value returned from the given function will be inserted into it and a mutable
  /// reference to that value will be returned. Otherwise, a mutable reference to the first value, by insertion order,
  /// will be returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let value = map.entry("key").or_insert_with(|| "value2");
  /// assert_eq!(value, &"value1");
  ///
  /// let value = map.entry("key2").or_insert_with(|| "value2");
  /// assert_eq!(value, &"value2");
  /// ```
  pub fn or_insert_with<Function>(self, function: Function) -> &'map mut Value
  where
    Function: FnOnce() -> Value,
  {
    match self {
      Entry::Occupied(entry) => entry.into_mut(),
      Entry::Vacant(entry) => entry.insert(function()),
    }
  }

  /// If the entry is vacant, the value returned from the given function will be inserted into it and the new occupied
  /// entry will be returned. Otherwise, the existing occupied entry will be returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let entry = map.entry("key").or_insert_with_entry(|| "value2");
  /// assert_eq!(entry.into_mut(), &"value1");
  ///
  /// let entry = map.entry("key2").or_insert_with_entry(|| "value2");
  /// assert_eq!(entry.into_mut(), &"value2");
  /// ```
  pub fn or_insert_with_entry<Function>(self, function: Function) -> OccupiedEntry<'map, Key, Value>
  where
    Function: FnOnce() -> Value,
  {
    match self {
      Entry::Occupied(entry) => entry,
      Entry::Vacant(entry) => entry.insert_entry(function()),
    }
  }
}

impl<Key, Value, State> Debug for Entry<'_, Key, Value, State>
where
  Key: Debug,
  State: BuildHasher,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Entry::Occupied(entry) => entry.fmt(formatter),
      Entry::Vacant(entry) => entry.fmt(formatter),
    }
  }
}

/// A view into an occupied entry in the multimap.
pub struct OccupiedEntry<'map, Key, Value> {
  entry: RawOccupiedEntryMut<'map, Index<Key>, MapEntry<Key, Value>, DummyState>,

  keys: &'map mut VecList<Key>,

  values: &'map mut VecList<ValueEntry<Key, Value>>,
}

#[allow(clippy::len_without_is_empty)]
impl<'map, Key, Value> OccupiedEntry<'map, Key, Value> {
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.append("value2");
  ///
  /// let mut iter = map.get_all(&"key");
  /// assert_eq!(iter.next(), Some(&"value1"));
  /// assert_eq!(iter.next(), Some(&"value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  pub fn append(&mut self, value: Value) {
    let key_index = *self.entry.key();
    let map_entry = self.entry.get_mut();
    let mut value_entry = ValueEntry::new(key_index, value);
    value_entry.previous_index = Some(map_entry.tail_index);
    let index = self.values.push_back(value_entry);
    self
      .values
      .get_mut(map_entry.tail_index)
      .unwrap()
      .next_index = Some(index);
    map_entry.length += 1;
    map_entry.tail_index = index;
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.get(), &"value");
  /// ```
  #[must_use]
  pub fn get(&self) -> &Value {
    let index = self.entry.get().head_index;
    &self.values.get(index).unwrap().value
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.get(), &mut "value");
  /// ```
  #[must_use]
  pub fn get_mut(&mut self) -> &mut Value {
    let index = self.entry.get().head_index;
    &mut self.values.get_mut(index).unwrap().value
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.insert("value2");
  ///
  /// assert_eq!(map.get(&"key"), Some(&"value2"));
  /// ```
  pub fn insert(&mut self, value: Value) -> Value {
    let key_index = *self.entry.key();
    let map_entry = self.entry.get_mut();
    let first_index = map_entry.head_index;
    let mut entry = self.values.remove(first_index).unwrap();
    let first_value = entry.value;

    while let Some(next_index) = entry.next_index {
      entry = self.values.remove(next_index).unwrap();
    }

    let value_entry = ValueEntry::new(key_index, value);
    let index = self.values.push_back(value_entry);
    map_entry.head_index = index;
    map_entry.length = 1;
    map_entry.tail_index = index;
    first_value
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///   Entry::Occupied(entry) => entry,
  ///   _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.append("value2");
  ///
  /// let mut iter = entry.insert_all("value3");
  /// assert_eq!(iter.next(), Some("value1"));
  /// assert_eq!(iter.next(), Some("value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  pub fn insert_all(&mut self, value: Value) -> EntryValuesDrain<'_, Key, Value> {
    let key_index = *self.entry.key();
    let map_entry = self.entry.get_mut();
    let value_entry = ValueEntry::new(key_index, value);
    let index = self.values.push_back(value_entry);
    let iter = EntryValuesDrain::from_map_entry(self.values, map_entry);
    map_entry.reset(index);
    iter
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  ///
  /// let mut entry = match map.entry("key") {
  ///   Entry::Occupied(entry) => entry,
  ///   _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.into_mut(), &mut "value");
  /// ```
  #[must_use]
  pub fn into_mut(mut self) -> &'map mut Value {
    let index = self.entry.get_mut().head_index;
    &mut self.values.get_mut(index).unwrap().value
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///   Entry::Occupied(entry) => entry,
  ///   _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.append("value2");
  ///
  /// let mut iter = entry.iter();
  /// assert_eq!(iter.next(), Some(&"value1"));
  /// assert_eq!(iter.next(), Some(&"value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn iter(&self) -> EntryValues<'_, Key, Value> {
    let map_entry = self.entry.get();
    EntryValues::from_map_entry(self.values, map_entry)
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.append("value2");
  ///
  /// let mut iter = entry.iter_mut();
  /// assert_eq!(iter.next(), Some(&mut "value1"));
  /// assert_eq!(iter.next(), Some(&mut "value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  #[must_use]
  pub fn iter_mut(&mut self) -> EntryValuesMut<'_, Key, Value> {
    let map_entry = self.entry.get_mut();
    EntryValuesMut::from_map_entry(self.values, map_entry)
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///   Entry::Occupied(entry) => entry,
  ///   _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.key(), &"key");
  /// ```
  #[must_use]
  pub fn key(&self) -> &Key {
    let key_index = self.entry.key();
    self.keys.get(*key_index).unwrap()
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.len(), 1);
  ///
  /// entry.append("value2");
  /// assert_eq!(entry.len(), 2);
  /// ```
  #[must_use]
  pub fn len(&self) -> usize {
    self.entry.get().length
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.remove(), "value");
  /// ```
  pub fn remove(self) -> Value {
    self.remove_entry().1
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.append("value2");
  ///
  /// let mut iter = entry.remove_all();
  /// assert_eq!(iter.next(), Some("value1"));
  /// assert_eq!(iter.next(), Some("value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  pub fn remove_all(self) -> EntryValuesDrain<'map, Key, Value> {
    self.remove_entry_all().1
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// assert_eq!(entry.remove_entry(), ("key", "value"));
  /// ```
  pub fn remove_entry(self) -> (Key, Value) {
    let (key_index, map_entry) = self.entry.remove_entry();
    let key = self.keys.remove(key_index).unwrap();
    let first_index = map_entry.head_index;
    let mut entry = self.values.remove(first_index).unwrap();
    let first_value = entry.value;

    while let Some(next_index) = entry.next_index {
      entry = self.values.remove(next_index).unwrap();
    }

    (key, first_value)
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  /// map.insert("key", "value1");
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Occupied(entry) => entry,
  ///     _ => panic!("expected occupied entry")
  /// };
  ///
  /// entry.append("value2");
  ///
  /// let (key, mut iter) = entry.remove_entry_all();
  /// assert_eq!(key, "key");
  /// assert_eq!(iter.next(), Some("value1"));
  /// assert_eq!(iter.next(), Some("value2"));
  /// assert_eq!(iter.next(), None);
  /// ```
  pub fn remove_entry_all(self) -> (Key, EntryValuesDrain<'map, Key, Value>) {
    let (key_index, map_entry) = self.entry.remove_entry();
    let key = self.keys.remove(key_index).unwrap();
    let iter = EntryValuesDrain {
      head_index: Some(map_entry.head_index),
      remaining: map_entry.length,
      tail_index: Some(map_entry.tail_index),
      values: self.values,
    };
    (key, iter)
  }
}

impl<Key, Value> Debug for OccupiedEntry<'_, Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("OccupiedEntry")
      .field("key", self.key())
      .field("values", &self.iter())
      .finish()
  }
}

/// A view into a vacant entry in the multimap.
pub struct VacantEntry<'map, Key, Value, State = RandomState> {
  /// The builder hasher for the map, kept separately for mutability concerns.
  build_hasher: &'map State,

  /// The hash of the key for the entry.
  hash: u64,

  /// The key for this entry for when it is to be inserted into the map.
  key: Key,

  keys: &'map mut VecList<Key>,

  /// Reference to the multimap.
  map: &'map mut HashMap<Index<Key>, MapEntry<Key, Value>, DummyState>,

  values: &'map mut VecList<ValueEntry<Key, Value>>,
}

impl<'map, Key, Value, State> VacantEntry<'map, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Vacant(entry) => entry,
  ///     _ => panic!("expected vacant entry")
  /// };
  ///
  /// assert_eq!(entry.insert("value"), &"value");
  /// ```
  pub fn insert(self, value: Value) -> &'map mut Value {
    let build_hasher = self.build_hasher;
    let entry = match raw_entry_mut(self.keys, self.map, self.hash, &self.key) {
      RawEntryMut::Vacant(entry) => entry,
      _ => panic!("expected vacant entry"),
    };
    let key_index = self.keys.push_back(self.key);
    let value_entry = ValueEntry::new(key_index, value);
    let index = self.values.push_back(value_entry);
    let map_entry = MapEntry::new(index);
    let keys = &self.keys;
    let _ = entry.insert_with_hasher(self.hash, key_index, map_entry, |&key_index| {
      let key = keys.get(key_index).unwrap();
      hash_key(build_hasher, key)
    });

    &mut self.values.get_mut(index).unwrap().value
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map = ListOrderedMultimap::new();
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Vacant(entry) => entry,
  ///     _ => panic!("expected vacant entry")
  /// };
  ///
  /// let mut entry = entry.insert_entry("value");
  /// assert_eq!(entry.get(), &"value");
  /// ```
  pub fn insert_entry(self, value: Value) -> OccupiedEntry<'map, Key, Value> {
    let build_hasher = self.build_hasher;
    let entry = match raw_entry_mut(self.keys, self.map, self.hash, &self.key) {
      RawEntryMut::Vacant(entry) => entry,
      _ => panic!("expected vacant entry"),
    };
    let key_index = self.keys.push_back(self.key);
    let value_entry = ValueEntry::new(key_index, value);
    let index = self.values.push_back(value_entry);
    let map_entry = MapEntry::new(index);
    let keys = &self.keys;
    let _ = entry.insert_with_hasher(self.hash, key_index, map_entry, |&key_index| {
      let key = keys.get(key_index).unwrap();
      hash_key(build_hasher, key)
    });

    let key = self.keys.get(key_index).unwrap();
    let entry = match raw_entry_mut(self.keys, self.map, self.hash, key) {
      RawEntryMut::Occupied(entry) => entry,
      _ => panic!("expected occupied entry"),
    };

    OccupiedEntry {
      entry,
      keys: self.keys,
      values: self.values,
    }
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Vacant(entry) => entry,
  ///     _ => panic!("expected vacant entry")
  /// };
  ///
  /// assert_eq!(entry.into_key(), "key");
  /// ```
  #[must_use]
  pub fn into_key(self) -> Key {
    self.key
  }

  /// # Examples
  ///
  /// ```
  /// use ordered_multimap::ListOrderedMultimap;
  /// use ordered_multimap::list_ordered_multimap::Entry;
  ///
  /// let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
  ///
  /// let mut entry = match map.entry("key") {
  ///     Entry::Vacant(entry) => entry,
  ///     _ => panic!("expected vacant entry")
  /// };
  ///
  /// assert_eq!(entry.key(), &"key");
  /// ```
  #[must_use]
  pub fn key(&self) -> &Key {
    &self.key
  }
}

impl<Key, Value, State> Debug for VacantEntry<'_, Key, Value, State>
where
  Key: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter
      .debug_tuple("VacantEntry")
      .field(&self.key)
      .finish()
  }
}

/// An iterator that yields immutable references to all values of a given key. The order of the values is always in the
/// order that they were inserted.
pub struct EntryValues<'map, Key, Value> {
  /// The first index of the values not yet yielded.
  head_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The remaining number of values to be yielded.
  remaining: usize,

  /// The last index of the values not yet yielded.
  tail_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The list of the values in the map. This is ordered by time of insertion.
  values: &'map VecList<ValueEntry<Key, Value>>,
}

impl<'map, Key, Value> EntryValues<'map, Key, Value> {
  /// Convenience function for creating an empty iterator.
  #[must_use]
  fn empty(values: &'map VecList<ValueEntry<Key, Value>>) -> Self {
    EntryValues {
      head_index: None,
      remaining: 0,
      tail_index: None,
      values,
    }
  }

  /// Convenience function for creating a new iterator from a map entry.
  #[must_use]
  fn from_map_entry(
    values: &'map VecList<ValueEntry<Key, Value>>,
    map_entry: &MapEntry<Key, Value>,
  ) -> Self {
    EntryValues {
      head_index: Some(map_entry.head_index),
      remaining: map_entry.length,
      tail_index: Some(map_entry.tail_index),
      values,
    }
  }
}

impl<'map, Key, Value> Clone for EntryValues<'map, Key, Value> {
  fn clone(&self) -> EntryValues<'map, Key, Value> {
    EntryValues {
      head_index: self.head_index,
      remaining: self.remaining,
      tail_index: self.tail_index,
      values: self.values,
    }
  }
}

impl<Key, Value> Debug for EntryValues<'_, Key, Value>
where
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("EntryValues(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for EntryValues<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail_index.map(|index| {
        let entry = self.values.get(index).unwrap();
        self.tail_index = entry.previous_index;
        self.remaining -= 1;
        &entry.value
      })
    }
  }
}

impl<Key, Value> ExactSizeIterator for EntryValues<'_, Key, Value> {}

impl<Key, Value> FusedIterator for EntryValues<'_, Key, Value> {}

impl<'map, Key, Value> Iterator for EntryValues<'map, Key, Value> {
  type Item = &'map Value;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head_index.map(|index| {
        let entry = self.values.get(index).unwrap();
        self.head_index = entry.next_index;
        self.remaining -= 1;
        &entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

/// An iterator that moves all values of a given key out of a multimap but preserves the underlying capacity. The order
/// of the values is always in the order that they were inserted.
pub struct EntryValuesDrain<'map, Key, Value> {
  /// The first index of the values not yet yielded.
  head_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The remaining number of values to be yielded.
  remaining: usize,

  /// The last index of the values not yet yielded.
  tail_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The list of the values in the map. This is ordered by time of insertion.
  values: &'map mut VecList<ValueEntry<Key, Value>>,
}

impl<'map, Key, Value> EntryValuesDrain<'map, Key, Value> {
  /// Convenience function for creating an empty iterator.
  fn empty(values: &'map mut VecList<ValueEntry<Key, Value>>) -> Self {
    EntryValuesDrain {
      head_index: None,
      remaining: 0,
      tail_index: None,
      values,
    }
  }

  /// Convenience function for creating a new iterator from a map entry.
  fn from_map_entry(
    values: &'map mut VecList<ValueEntry<Key, Value>>,
    map_entry: &MapEntry<Key, Value>,
  ) -> Self {
    EntryValuesDrain {
      head_index: Some(map_entry.head_index),
      remaining: map_entry.length,
      tail_index: Some(map_entry.tail_index),
      values,
    }
  }

  /// Creates an iterator that yields immutable references to all values of a given key.
  #[must_use]
  pub fn iter(&self) -> EntryValues<'_, Key, Value> {
    EntryValues {
      head_index: self.head_index,
      remaining: self.remaining,
      tail_index: self.tail_index,
      values: self.values,
    }
  }
}

impl<Key, Value> Debug for EntryValuesDrain<'_, Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("EntryValuesDrain(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for EntryValuesDrain<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail_index.map(|index| {
        let entry = self.values.remove(index).unwrap();
        self.tail_index = entry.previous_index;
        self.remaining -= 1;
        entry.value
      })
    }
  }
}

impl<Key, Value> Drop for EntryValuesDrain<'_, Key, Value> {
  fn drop(&mut self) {
    for _ in self {}
  }
}

impl<Key, Value> ExactSizeIterator for EntryValuesDrain<'_, Key, Value> {}

impl<Key, Value> FusedIterator for EntryValuesDrain<'_, Key, Value> {}

impl<Key, Value> Iterator for EntryValuesDrain<'_, Key, Value> {
  type Item = Value;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head_index.map(|index| {
        let entry = self.values.remove(index).unwrap();
        self.head_index = entry.next_index;
        self.remaining -= 1;
        entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

/// An iterator that yields mutable references to all values of a given key. The order of the values is always in the
/// order that they were inserted.
pub struct EntryValuesMut<'map, Key, Value> {
  /// The first index of the values not yet yielded.
  head_index: Option<Index<ValueEntry<Key, Value>>>,

  /// Because [`EntryValuesMut::values`] is a pointer, we need to have a phantom data here for the lifetime parameter.
  phantom: PhantomData<&'map mut VecList<ValueEntry<Key, Value>>>,

  /// The remaining number of values to be yielded.
  remaining: usize,

  /// The last index of the values not yet yielded.
  tail_index: Option<Index<ValueEntry<Key, Value>>>,

  /// The list of the values in the map. This is ordered by time of insertion.
  values: *mut VecList<ValueEntry<Key, Value>>,
}

impl<'map, Key, Value> EntryValuesMut<'map, Key, Value> {
  /// Convenience function for creating an empty iterator.
  #[must_use]
  fn empty(values: &'map mut VecList<ValueEntry<Key, Value>>) -> Self {
    EntryValuesMut {
      head_index: None,
      phantom: PhantomData,
      remaining: 0,
      tail_index: None,
      values,
    }
  }

  /// Convenience function for creating a new iterator from a map entry.
  #[must_use]
  fn from_map_entry(
    values: &'map mut VecList<ValueEntry<Key, Value>>,
    map_entry: &MapEntry<Key, Value>,
  ) -> Self {
    EntryValuesMut {
      head_index: Some(map_entry.head_index),
      phantom: PhantomData,
      remaining: map_entry.length,
      tail_index: Some(map_entry.tail_index),
      values,
    }
  }

  /// Creates an iterator that yields immutable references to all values of a given key.
  #[must_use]
  pub fn iter(&self) -> EntryValues<'_, Key, Value> {
    EntryValues {
      head_index: self.head_index,
      remaining: self.remaining,
      tail_index: self.tail_index,
      values: unsafe { &*self.values },
    }
  }
}

impl<Key, Value> Debug for EntryValuesMut<'_, Key, Value>
where
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("EntryValuesMut(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for EntryValuesMut<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.tail_index.map(|index| {
        let entry = unsafe { (*self.values).get_mut(index) }.unwrap();
        self.tail_index = entry.previous_index;
        self.remaining -= 1;
        &mut entry.value
      })
    }
  }
}

impl<Key, Value> ExactSizeIterator for EntryValuesMut<'_, Key, Value> {}

impl<Key, Value> FusedIterator for EntryValuesMut<'_, Key, Value> {}

impl<'map, Key, Value> Iterator for EntryValuesMut<'map, Key, Value> {
  type Item = &'map mut Value;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      None
    } else {
      self.head_index.map(|index| {
        let entry = unsafe { (*self.values).get_mut(index) }.unwrap();
        self.head_index = entry.next_index;
        self.remaining -= 1;
        &mut entry.value
      })
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.remaining, Some(self.remaining))
  }
}

unsafe impl<Key, Value> Send for EntryValuesMut<'_, Key, Value>
where
  Key: Send,
  Value: Send,
{
}

unsafe impl<Key, Value> Sync for EntryValuesMut<'_, Key, Value>
where
  Key: Sync,
  Value: Sync,
{
}

/// An iterator that owns and yields all key-value pairs in a multimap by cloning the keys for their possibly multiple
/// values. This is unnecessarily expensive whenever [`Iter`] or [`IterMut`] would suit as well. The order of the
/// yielded items is always in the order that they were inserted.
pub struct IntoIter<Key, Value> {
  // The list of the keys in the map. This is ordered by time of insertion.
  keys: VecList<Key>,

  /// The iterator over the list of all values. This is ordered by time of insertion.
  iter: VecListIntoIter<ValueEntry<Key, Value>>,
}

impl<Key, Value> IntoIter<Key, Value> {
  /// Creates an iterator that yields immutable references to all key-value pairs in a multimap.
  #[must_use]
  pub fn iter(&self) -> Iter<'_, Key, Value> {
    Iter {
      keys: &self.keys,
      iter: self.iter.iter(),
    }
  }
}

impl<Key, Value> Debug for IntoIter<Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("IntoIter(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for IntoIter<Key, Value>
where
  Key: Clone,
{
  fn next_back(&mut self) -> Option<Self::Item> {
    let value_entry = self.iter.next_back()?;
    let key = self.keys.get(value_entry.key_index).cloned().unwrap();
    Some((key, value_entry.value))
  }
}

impl<Key, Value> ExactSizeIterator for IntoIter<Key, Value> where Key: Clone {}

impl<Key, Value> FusedIterator for IntoIter<Key, Value> where Key: Clone {}

impl<Key, Value> Iterator for IntoIter<Key, Value>
where
  Key: Clone,
{
  type Item = (Key, Value);

  fn next(&mut self) -> Option<Self::Item> {
    let value_entry = self.iter.next()?;
    let key = self.keys.get(value_entry.key_index).cloned().unwrap();
    Some((key, value_entry.value))
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

/// An iterator that yields immutable references to all key-value pairs in a multimap. The order of the yielded items is
/// always in the order that they were inserted.
pub struct Iter<'map, Key, Value> {
  // The list of the keys in the map. This is ordered by time of insertion.
  keys: &'map VecList<Key>,

  /// The iterator over the list of all values. This is ordered by time of insertion.
  iter: VecListIter<'map, ValueEntry<Key, Value>>,
}

impl<'map, Key, Value> Clone for Iter<'map, Key, Value> {
  fn clone(&self) -> Iter<'map, Key, Value> {
    Iter {
      keys: self.keys,
      iter: self.iter.clone(),
    }
  }
}

impl<Key, Value> Debug for Iter<'_, Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("Iter(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for Iter<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    let value_entry = self.iter.next_back()?;
    let key = self.keys.get(value_entry.key_index).unwrap();
    Some((key, &value_entry.value))
  }
}

impl<Key, Value> ExactSizeIterator for Iter<'_, Key, Value> {}

impl<Key, Value> FusedIterator for Iter<'_, Key, Value> {}

impl<'map, Key, Value> Iterator for Iter<'map, Key, Value> {
  type Item = (&'map Key, &'map Value);

  fn next(&mut self) -> Option<Self::Item> {
    let value_entry = self.iter.next()?;
    let key = self.keys.get(value_entry.key_index).unwrap();
    Some((key, &value_entry.value))
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

/// An iterator that yields mutable references to all key-value pairs in a multimap. The order of the yielded items is
/// always in the order that they were inserted.
pub struct IterMut<'map, Key, Value> {
  // The list of the keys in the map. This is ordered by time of insertion.
  keys: &'map VecList<Key>,

  /// The iterator over the list of all values. This is ordered by time of insertion.
  iter: VecListIterMut<'map, ValueEntry<Key, Value>>,
}

impl<Key, Value> IterMut<'_, Key, Value> {
  /// Creates an iterator that yields immutable references to all key-value pairs in a multimap.
  #[must_use]
  pub fn iter(&self) -> Iter<'_, Key, Value> {
    Iter {
      keys: self.keys,
      iter: self.iter.iter(),
    }
  }
}

impl<Key, Value> Debug for IterMut<'_, Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("IterMut(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for IterMut<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    let value_entry = self.iter.next_back()?;
    let key = self.keys.get(value_entry.key_index).unwrap();
    Some((key, &mut value_entry.value))
  }
}

impl<Key, Value> ExactSizeIterator for IterMut<'_, Key, Value> {}

impl<Key, Value> FusedIterator for IterMut<'_, Key, Value> {}

impl<'map, Key, Value> Iterator for IterMut<'map, Key, Value> {
  type Item = (&'map Key, &'map mut Value);

  fn next(&mut self) -> Option<Self::Item> {
    let value_entry = self.iter.next()?;
    let key = self.keys.get(value_entry.key_index).unwrap();
    Some((key, &mut value_entry.value))
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

/// An iterator that yields immutable references to all keys and their value iterators. The order of the yielded items
/// is always in the order the keys were first inserted.
pub struct KeyValues<'map, Key, Value, State = RandomState> {
  /// The builder hasher for the map, kept separately for mutability concerns.
  build_hasher: &'map State,

  // The list of the keys in the map. This is ordered by time of insertion.
  keys: &'map VecList<Key>,

  /// The iterator over the list of all values. This is ordered by time of insertion.
  iter: VecListIter<'map, Key>,

  /// The internal mapping from key hashes to associated value indices.
  map: &'map HashMap<Index<Key>, MapEntry<Key, Value>, DummyState>,

  /// The list of the values in the map. This is ordered by time of insertion.
  values: &'map VecList<ValueEntry<Key, Value>>,
}

impl<'map, Key, Value, State> Clone for KeyValues<'map, Key, Value, State> {
  fn clone(&self) -> KeyValues<'map, Key, Value, State> {
    KeyValues {
      build_hasher: self.build_hasher,
      keys: self.keys,
      iter: self.iter.clone(),
      map: self.map,
      values: self.values,
    }
  }
}

impl<Key, Value, State> Debug for KeyValues<'_, Key, Value, State>
where
  Key: Debug + Eq + Hash,
  State: BuildHasher,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("KeyValues(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value, State> DoubleEndedIterator for KeyValues<'_, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  fn next_back(&mut self) -> Option<Self::Item> {
    let key = self.iter.next_back()?;
    let hash = hash_key(self.build_hasher, key);
    let (_, map_entry) = raw_entry(self.keys, self.map, hash, key).unwrap();
    let iter = EntryValues::from_map_entry(self.values, map_entry);
    Some((key, iter))
  }
}

impl<Key, Value, State> ExactSizeIterator for KeyValues<'_, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
}

impl<Key, Value, State> FusedIterator for KeyValues<'_, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
}

impl<'map, Key, Value, State> Iterator for KeyValues<'map, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  type Item = (&'map Key, EntryValues<'map, Key, Value>);

  fn next(&mut self) -> Option<Self::Item> {
    let key = self.iter.next()?;
    let hash = hash_key(self.build_hasher, key);
    let (_, map_entry) = raw_entry(self.keys, self.map, hash, key).unwrap();
    let iter = EntryValues::from_map_entry(self.values, map_entry);
    Some((key, iter))
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

/// An iterator that yields mutable references to all keys and their value iterators. The order of the yielded items is
/// always in the order the keys were first inserted.
pub struct KeyValuesMut<'map, Key, Value, State = RandomState> {
  /// The builder hasher for the map, kept separately for mutability concerns.
  build_hasher: &'map State,

  // The list of the keys in the map. This is ordered by time of insertion.
  keys: &'map VecList<Key>,

  /// The iterator over the list of all values. This is ordered by time of insertion.
  iter: VecListIter<'map, Key>,

  /// The internal mapping from key hashes to associated value indices.
  map: &'map HashMap<Index<Key>, MapEntry<Key, Value>, DummyState>,

  /// The list of the values in the map. This is ordered by time of insertion.
  values: *mut VecList<ValueEntry<Key, Value>>,
}

impl<Key, Value, State> KeyValuesMut<'_, Key, Value, State> {
  /// Creates an iterator that yields mutable references to all key-value pairs of a multimap.
  #[must_use]
  pub fn iter(&self) -> KeyValues<'_, Key, Value, State> {
    KeyValues {
      build_hasher: self.build_hasher,
      keys: self.keys,
      iter: self.iter.clone(),
      map: self.map,
      values: unsafe { &*self.values },
    }
  }
}

impl<Key, Value, State> Debug for KeyValuesMut<'_, Key, Value, State>
where
  Key: Debug + Eq + Hash,
  State: BuildHasher,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("KeyValuesMut(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value, State> DoubleEndedIterator for KeyValuesMut<'_, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  fn next_back(&mut self) -> Option<Self::Item> {
    let key = self.iter.next_back()?;
    let hash = hash_key(self.build_hasher, key);
    let (_, map_entry) = raw_entry(self.keys, self.map, hash, key).unwrap();
    let iter = EntryValuesMut::from_map_entry(unsafe { &mut *self.values }, map_entry);
    Some((key, iter))
  }
}

impl<Key, Value, State> ExactSizeIterator for KeyValuesMut<'_, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
}

impl<Key, Value, State> FusedIterator for KeyValuesMut<'_, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
}

impl<'map, Key, Value, State> Iterator for KeyValuesMut<'map, Key, Value, State>
where
  Key: Eq + Hash,
  State: BuildHasher,
{
  type Item = (&'map Key, EntryValuesMut<'map, Key, Value>);

  fn next(&mut self) -> Option<Self::Item> {
    let key = self.iter.next()?;
    let hash = hash_key(self.build_hasher, key);
    let (_, map_entry) = raw_entry(self.keys, self.map, hash, key).unwrap();
    let iter = EntryValuesMut::from_map_entry(unsafe { &mut *self.values }, map_entry);
    Some((key, iter))
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

unsafe impl<Key, Value> Send for KeyValuesMut<'_, Key, Value>
where
  Key: Send,
  Value: Send,
{
}

unsafe impl<Key, Value> Sync for KeyValuesMut<'_, Key, Value>
where
  Key: Sync,
  Value: Sync,
{
}

/// An iterator that yields immutable references to all keys in the multimap. The order of the keys is always in the
/// order that they were first inserted.
pub struct Keys<'map, Key>(VecListIter<'map, Key>);

impl<'map, Key> Clone for Keys<'map, Key> {
  fn clone(&self) -> Keys<'map, Key> {
    Keys(self.0.clone())
  }
}

impl<Key> Debug for Keys<'_, Key>
where
  Key: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("Keys(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key> DoubleEndedIterator for Keys<'_, Key> {
  fn next_back(&mut self) -> Option<Self::Item> {
    self.0.next_back()
  }
}

impl<Key> ExactSizeIterator for Keys<'_, Key> {}

impl<Key> FusedIterator for Keys<'_, Key> {}

impl<'map, Key> Iterator for Keys<'map, Key> {
  type Item = &'map Key;

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next()
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.0.size_hint()
  }
}

/// An iterator that yields immutable references to all values of a multimap. The order of the values is always in the
/// order that they were inserted.
pub struct Values<'map, Key, Value>(VecListIter<'map, ValueEntry<Key, Value>>);

impl<'map, Key, Value> Clone for Values<'map, Key, Value> {
  fn clone(&self) -> Values<'map, Key, Value> {
    Values(self.0.clone())
  }
}

impl<Key, Value> Debug for Values<'_, Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("Values(")?;
    formatter.debug_list().entries(self.clone()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for Values<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    self.0.next_back().map(|entry| &entry.value)
  }
}

impl<Key, Value> ExactSizeIterator for Values<'_, Key, Value> {}

impl<Key, Value> FusedIterator for Values<'_, Key, Value> {}

impl<'map, Key, Value> Iterator for Values<'map, Key, Value> {
  type Item = &'map Value;

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next().map(|entry| &entry.value)
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.0.size_hint()
  }
}

/// An iterator that yields mutable references to all values of a multimap. The order of the values is always in the
/// order that they were inserted.
pub struct ValuesMut<'map, Key, Value>(VecListIterMut<'map, ValueEntry<Key, Value>>);

impl<Key, Value> ValuesMut<'_, Key, Value> {
  /// Creates an iterator that yields immutable references to all values of a multimap.
  #[must_use]
  pub fn iter(&self) -> Values<'_, Key, Value> {
    Values(self.0.iter())
  }
}

impl<Key, Value> Debug for ValuesMut<'_, Key, Value>
where
  Key: Debug,
  Value: Debug,
{
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("ValuesMut(")?;
    formatter.debug_list().entries(self.iter()).finish()?;
    formatter.write_str(")")
  }
}

impl<Key, Value> DoubleEndedIterator for ValuesMut<'_, Key, Value> {
  fn next_back(&mut self) -> Option<Self::Item> {
    self.0.next_back().map(|entry| &mut entry.value)
  }
}

impl<Key, Value> ExactSizeIterator for ValuesMut<'_, Key, Value> {}

impl<Key, Value> FusedIterator for ValuesMut<'_, Key, Value> {}

impl<'map, Key, Value> Iterator for ValuesMut<'map, Key, Value> {
  type Item = &'map mut Value;

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next().map(|entry| &mut entry.value)
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.0.size_hint()
  }
}

/// Dummy builder hasher that is not meant to be used. It is simply a placeholder.
#[derive(Clone, Debug)]
pub(crate) struct DummyState;

impl BuildHasher for DummyState {
  type Hasher = DummyHasher;

  fn build_hasher(&self) -> Self::Hasher {
    DummyHasher
  }
}

/// Dummy hasher that is not meant to be used. It is simply a placeholder.
#[derive(Clone, Debug)]
pub struct DummyHasher;

impl Hasher for DummyHasher {
  fn finish(&self) -> u64 {
    unimplemented!();
  }

  fn write(&mut self, _: &[u8]) {
    unimplemented!();
  }
}

/// Computes the hash value of the given key.
#[must_use]
fn hash_key<KeyQuery, State>(state: &State, key: &KeyQuery) -> u64
where
  KeyQuery: ?Sized + Eq + Hash,
  State: BuildHasher,
{
  let mut hasher = state.build_hasher();
  key.hash(&mut hasher);
  hasher.finish()
}

#[must_use]
fn raw_entry<'map, Key, KeyQuery, Value, State>(
  keys: &VecList<Key>,
  map: &'map HashMap<Index<Key>, MapEntry<Key, Value>, State>,
  hash: u64,
  key: &KeyQuery,
) -> Option<(&'map Index<Key>, &'map MapEntry<Key, Value>)>
where
  Key: Borrow<KeyQuery> + Eq + Hash,
  KeyQuery: ?Sized + Eq + Hash,
  State: BuildHasher,
{
  map.raw_entry().from_hash(hash, |&key_index| {
    let existing_key = keys.get(key_index).unwrap();
    key == existing_key.borrow()
  })
}

#[must_use]
fn raw_entry_mut<'map, Key, KeyQuery, Value, State>(
  keys: &VecList<Key>,
  map: &'map mut HashMap<Index<Key>, MapEntry<Key, Value>, State>,
  hash: u64,
  key: &KeyQuery,
) -> RawEntryMut<'map, Index<Key>, MapEntry<Key, Value>, State>
where
  Key: Borrow<KeyQuery> + Eq + Hash,
  KeyQuery: ?Sized + Eq + Hash,
  State: BuildHasher,
{
  map.raw_entry_mut().from_hash(hash, |&key_index| {
    let existing_key = keys.get(key_index).unwrap();
    key == existing_key.borrow()
  })
}

#[must_use]
fn raw_entry_mut_empty<'map, Key, KeyQuery, Value, State>(
  keys: &VecList<Key>,
  map: &'map mut HashMap<Index<Key>, MapEntry<Key, Value>, State>,
  hash: u64,
) -> RawEntryMut<'map, Index<Key>, MapEntry<Key, Value>, State>
where
  Key: Borrow<KeyQuery> + Eq + Hash,
  KeyQuery: ?Sized + Eq + Hash,
  State: BuildHasher,
{
  map
    .raw_entry_mut()
    .from_hash(hash, |&key_index| keys.get(key_index).is_none())
}

#[allow(unused_results)]
#[cfg(all(test, feature = "std"))]
mod test {
  use coverage_helper::test;

  use super::*;

  #[test]
  fn test_bounds() {
    fn check_bounds<Type: Send + Sync>() {}

    check_bounds::<EntryValues<'static, (), ()>>();
    check_bounds::<EntryValuesDrain<'static, (), ()>>();
    check_bounds::<EntryValuesMut<'static, (), ()>>();
    check_bounds::<IntoIter<(), ()>>();
    check_bounds::<Iter<'static, (), ()>>();
    check_bounds::<IterMut<'static, (), ()>>();
    check_bounds::<KeyValues<'static, (), ()>>();
    check_bounds::<KeyValuesMut<'static, (), ()>>();
    check_bounds::<ListOrderedMultimap<(), ()>>();
    check_bounds::<Values<'static, (), ()>>();
    check_bounds::<ValuesMut<'static, (), ()>>();
  }

  #[test]
  fn test_collision() {
    struct TestBuildHasher;

    impl BuildHasher for TestBuildHasher {
      type Hasher = TestHasher;

      fn build_hasher(&self) -> Self::Hasher {
        TestHasher
      }
    }

    struct TestHasher;

    impl Hasher for TestHasher {
      fn finish(&self) -> u64 {
        0
      }

      fn write(&mut self, _: &[u8]) {}
    }

    let mut map = ListOrderedMultimap::with_hasher(TestBuildHasher);
    let state = map.hasher();

    assert_eq!(hash_key(state, "key1"), hash_key(state, "key2"));

    map.insert("key1", "value1");
    assert_eq!(map.get(&"key1"), Some(&"value1"));

    map.insert("key2", "value2");
    assert_eq!(map.get(&"key2"), Some(&"value2"));
  }

  #[test]
  fn test_no_collision() {
    let state = RandomState::new();
    let hash_1 = hash_key(&state, "key1");
    let hash_2 = hash_key(&state, "key2");

    assert!(hash_1 != hash_2);
  }

  #[test]
  fn test_entry_and_modify() {
    let mut map = ListOrderedMultimap::new();
    map
      .entry("key")
      .and_modify(|_| panic!("entry should be vacant"));

    map.insert("key", "value1");
    map.entry("key").and_modify(|value| *value = "value2");
    assert_eq!(map.get(&"key"), Some(&"value2"));
  }

  #[test]
  fn test_entry_or_insert() {
    let mut map = ListOrderedMultimap::new();
    let value = map.entry("key").or_insert("value1");
    assert_eq!(value, &"value1");

    let value = map.entry("key").or_insert("value2");
    assert_eq!(value, &"value1");
  }

  #[test]
  fn test_entry_or_insert_entry() {
    let mut map = ListOrderedMultimap::new();
    let entry = map.entry("key").or_insert_entry("value1");
    assert_eq!(entry.get(), &"value1");

    let entry = map.entry("key").or_insert_entry("value2");
    assert_eq!(entry.get(), &"value1");
  }

  #[test]
  fn test_entry_or_insert_with() {
    let mut map = ListOrderedMultimap::new();
    let value = map.entry("key").or_insert_with(|| "value1");
    assert_eq!(value, &"value1");

    let value = map.entry("key").or_insert_with(|| "value2");
    assert_eq!(value, &"value1");
  }

  #[test]
  fn test_entry_or_insert_with_entry() {
    let mut map = ListOrderedMultimap::new();
    let entry = map.entry("key").or_insert_with_entry(|| "value1");
    assert_eq!(entry.get(), &"value1");

    let entry = map.entry("key").or_insert_with_entry(|| "value2");
    assert_eq!(entry.get(), &"value1");
  }

  #[test]
  fn test_entry_debug() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let entry = map.entry("key");

    assert_eq!(format!("{entry:?}"), r#"VacantEntry("key")"#);
  }

  #[test]
  fn test_entry_values_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let iter = map.get_all(&"key");
    assert_eq!(
      format!("{iter:?}"),
      r#"EntryValues(["value1", "value2", "value3", "value4"])"#
    );
  }

  #[test]
  fn test_entry_values_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next_back(), Some(&"value4"));
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next_back(), Some(&"value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_drain_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let iter = map.remove_all(&"key");
    assert_eq!(
      format!("{iter:?}"),
      r#"EntryValuesDrain(["value1", "value2", "value3", "value4"])"#
    );
  }

  #[test]
  fn test_entry_values_drain_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let mut iter = map.remove_all(&"key");
    assert_eq!(iter.next(), Some("value1"));
    assert_eq!(iter.next_back(), Some("value4"));
    assert_eq!(iter.next(), Some("value2"));
    assert_eq!(iter.next_back(), Some("value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_drain_empty() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.remove_all(&"key");
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_drain_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.remove_all(&"key");
    assert_eq!(iter.next(), Some("value"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_drain_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let mut iter = map.remove_all(&"key");
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
  fn test_entry_values_empty() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next(), Some(&"value"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_mut_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let iter = map.get_all_mut(&"key");
    assert_eq!(
      format!("{iter:?}"),
      r#"EntryValuesMut(["value1", "value2", "value3", "value4"])"#
    );
  }

  #[test]
  fn test_entry_values_mut_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let mut iter = map.get_all_mut(&"key");
    assert_eq!(iter.next(), Some(&mut "value1"));
    assert_eq!(iter.next_back(), Some(&mut "value4"));
    assert_eq!(iter.next(), Some(&mut "value2"));
    assert_eq!(iter.next_back(), Some(&mut "value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_mut_empty() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.get_all_mut(&"key");
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_mut_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.get_all_mut(&"key");
    assert_eq!(iter.next(), Some(&mut "value"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_entry_values_mut_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let mut iter = map.get_all_mut(&"key");
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
  fn test_entry_values_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let mut iter = map.get_all(&"key");
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
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.iter();
    assert_eq!(
      format!("{iter:?}"),
      r#"Iter([("key1", "value1"), ("key2", "value2"), ("key2", "value3"), ("key1", "value4")])"#
    );
  }

  #[test]
  fn test_iter_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.iter();
    assert_eq!(iter.next(), Some((&"key1", &"value1")));
    assert_eq!(iter.next_back(), Some((&"key1", &"value4")));
    assert_eq!(iter.next(), Some((&"key2", &"value2")));
    assert_eq!(iter.next_back(), Some((&"key2", &"value3")));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_empty() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.iter();
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.iter();
    assert_eq!(iter.next(), Some((&"key", &"value")));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_mut_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.iter_mut();
    assert_eq!(
      format!("{iter:?}"),
      r#"IterMut([("key1", "value1"), ("key2", "value2"), ("key2", "value3"), ("key1", "value4")])"#
    );
  }

  #[test]
  fn test_iter_mut_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.iter_mut();
    assert_eq!(iter.next(), Some((&"key1", &mut "value1")));
    assert_eq!(iter.next_back(), Some((&"key1", &mut "value4")));
    assert_eq!(iter.next(), Some((&"key2", &mut "value2")));
    assert_eq!(iter.next_back(), Some((&"key2", &mut "value3")));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_mut_empty() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.iter_mut();
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_mut_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.iter_mut();
    assert_eq!(iter.next(), Some((&"key", &mut "value")));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_iter_mut_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.iter_mut();
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
  fn test_iter_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.iter();
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
  fn test_into_iter_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.into_iter();
    assert_eq!(
      format!("{iter:?}"),
      r#"IntoIter([("key1", "value1"), ("key2", "value2"), ("key2", "value3"), ("key1", "value4")])"#
    );
  }

  #[test]
  fn test_into_iter_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.into_iter();
    assert_eq!(iter.next(), Some(("key1", "value1")));
    assert_eq!(iter.next_back(), Some(("key1", "value4")));
    assert_eq!(iter.next(), Some(("key2", "value2")));
    assert_eq!(iter.next_back(), Some(("key2", "value3")));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_into_iter_empty() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.into_iter();
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_into_iter_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.into_iter();
    assert_eq!(iter.next(), Some(("key", "value")));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_into_iter_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.into_iter();
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
  fn test_key_values_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.pairs();
    assert_eq!(
      format!("{iter:?}"),
      r#"KeyValues([("key1", EntryValues(["value1", "value4"])), ("key2", EntryValues(["value2", "value3"]))])"#
    );
  }

  #[test]
  fn test_key_values_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.pairs();

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key1");
    assert_eq!(values.next(), Some(&"value1"));
    assert_eq!(values.next(), Some(&"value4"));
    assert_eq!(values.next(), None);

    let (key, mut values) = iter.next_back().unwrap();
    assert_eq!(key, &"key2");
    assert_eq!(values.next(), Some(&"value2"));
    assert_eq!(values.next(), Some(&"value3"));
    assert_eq!(values.next(), None);

    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
  }

  #[test]
  fn test_key_values_empty() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.pairs();
    assert!(iter.next_back().is_none());
    assert!(iter.next().is_none());
  }

  #[test]
  fn test_key_values_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.pairs();

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key");
    assert_eq!(values.next(), Some(&"value"));
    assert_eq!(values.next(), None);

    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
    assert!(iter.next().is_none());
  }

  #[test]
  fn test_key_values_mut_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.pairs_mut();
    assert_eq!(
      format!("{iter:?}"),
      r#"KeyValuesMut([("key1", EntryValues(["value1", "value4"])), ("key2", EntryValues(["value2", "value3"]))])"#
    );
  }

  #[test]
  fn test_key_values_mut_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.pairs_mut();

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key1");
    assert_eq!(values.next(), Some(&mut "value1"));
    assert_eq!(values.next(), Some(&mut "value4"));
    assert_eq!(values.next(), None);

    let (key, mut values) = iter.next_back().unwrap();
    assert_eq!(key, &"key2");
    assert_eq!(values.next(), Some(&mut "value2"));
    assert_eq!(values.next(), Some(&mut "value3"));
    assert_eq!(values.next(), None);

    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
  }

  #[test]
  fn test_key_values_mut_empty() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.pairs_mut();
    assert!(iter.next_back().is_none());
    assert!(iter.next().is_none());
  }

  #[test]
  fn test_key_values_mut_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.pairs_mut();

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key");
    assert_eq!(values.next(), Some(&mut "value"));
    assert_eq!(values.next(), None);

    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
    assert!(iter.next().is_none());
  }

  #[test]
  fn test_key_values_mut_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.pairs_mut();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    iter.next();
    assert_eq!(iter.size_hint(), (1, Some(1)));
    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_key_values_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.pairs();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    iter.next();
    assert_eq!(iter.size_hint(), (1, Some(1)));
    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_keys_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.keys();
    assert_eq!(format!("{iter:?}"), r#"Keys(["key1", "key2"])"#);
  }

  #[test]
  fn test_keys_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.keys();
    assert_eq!(iter.next(), Some(&"key1"));
    assert_eq!(iter.next_back(), Some(&"key2"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_keys_empty() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.keys();
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_keys_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.keys();
    assert_eq!(iter.next(), Some(&"key"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_keys_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.keys();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    iter.next();
    assert_eq!(iter.size_hint(), (1, Some(1)));
    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(0)));
  }

  #[test]
  fn test_list_ordered_multimap_append() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.entry_len(&"key"), 0);

    let already_exists = map.append("key", "value1");
    assert!(!already_exists);
    assert_eq!(map.entry_len(&"key"), 1);

    let already_exists = map.append("key", "value2");
    assert!(already_exists);
    assert_eq!(map.entry_len(&"key"), 2);

    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_back() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.back(), None);

    map.insert("key1", "value1");
    assert_eq!(map.back(), Some((&"key1", &"value1")));

    map.append("key2", "value2");
    assert_eq!(map.back(), Some((&"key2", &"value2")));

    map.remove(&"key2");
    assert_eq!(map.back(), Some((&"key1", &"value1")));

    map.remove(&"key1");
    assert_eq!(map.back(), None);
  }

  #[test]
  fn test_list_ordered_multimap_back_mut() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.back(), None);

    map.insert("key1", "value1");
    assert_eq!(map.back(), Some((&"key1", &"value1")));

    map.append("key2", "value2");
    assert_eq!(map.back(), Some((&"key2", &"value2")));

    map.remove(&"key2");
    assert_eq!(map.back(), Some((&"key1", &"value1")));

    map.remove(&"key1");
    assert_eq!(map.back(), None);
  }

  #[test]
  fn test_list_ordered_multimap_clear() {
    let mut map = ListOrderedMultimap::new();
    map.insert("key", "value");
    map.insert("key2", "value");

    map.clear();

    assert!(map.is_empty());
    assert_eq!(map.get(&"key"), None);
    assert_eq!(map.get(&"key2"), None);
  }

  #[test]
  fn test_list_ordered_multimap_contains_key() {
    let mut map = ListOrderedMultimap::new();
    assert!(!map.contains_key(&"key"));

    map.insert("key", "value");
    assert!(map.contains_key(&"key"));
  }

  #[test]
  fn test_list_ordered_multimap_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    assert_eq!(
      format!("{map:?}"),
      r#"{"key1": "value1", "key2": "value2", "key2": "value3", "key1": "value4"}"#
    );
  }

  #[test]
  fn test_list_ordered_multimap_entry() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.get(&"key1"), None);

    let value = map.entry("key").or_insert("value1");
    assert_eq!(value, &"value1");
    assert_eq!(map.get(&"key"), Some(&"value1"));

    let value = map.entry("key").or_insert("value2");
    assert_eq!(value, &"value1");
    assert_eq!(map.get(&"key"), Some(&"value1"));
  }

  #[test]
  fn test_list_ordered_multimap_entry_len() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.entry_len(&"key1"), 0);

    map.insert("key", "value");
    assert_eq!(map.entry_len(&"key"), 1);

    map.insert("key", "value");
    assert_eq!(map.entry_len(&"key"), 1);

    map.append("key", "value");
    assert_eq!(map.entry_len(&"key"), 2);

    map.insert("key", "value");
    assert_eq!(map.entry_len(&"key"), 1);

    map.remove(&"key");
    assert_eq!(map.entry_len(&"key"), 0);
  }

  #[test]
  fn test_list_ordered_multimap_equality() {
    let mut map_1 = ListOrderedMultimap::new();

    map_1.insert("key1", "value1");
    map_1.insert("key2", "value2");
    map_1.append("key2", "value3");
    map_1.append("key1", "value4");

    let mut map_2 = map_1.clone();
    map_2.pop_back();

    assert_ne!(map_1, map_2);

    map_2.append("key1", "value4");
    assert_eq!(map_1, map_2);
  }

  #[test]
  fn test_list_ordered_multimap_extend() {
    let mut map = ListOrderedMultimap::new();
    map.extend(vec![
      ("key1", "value1"),
      ("key2", "value2"),
      ("key2", "value3"),
    ]);

    let mut iter = map.get_all(&"key1");
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next(), None);

    let mut iter = map.get_all(&"key2");
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next(), Some(&"value3"));
    assert_eq!(iter.next(), None);

    let mut map = ListOrderedMultimap::new();
    map.extend(vec![(&1, &1), (&2, &1), (&2, &2)]);

    let mut iter = map.get_all(&1);
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    let mut iter = map.get_all(&2);
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_from_iterator() {
    let map: ListOrderedMultimap<_, _, RandomState> = ListOrderedMultimap::from_iter(vec![
      ("key1", "value1"),
      ("key2", "value2"),
      ("key2", "value3"),
    ]);

    let mut iter = map.get_all(&"key1");
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next(), None);

    let mut iter = map.get_all(&"key2");
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next(), Some(&"value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_get() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.get(&"key"), None);

    map.insert("key", "value");
    assert_eq!(map.get(&"key"), Some(&"value"));
  }

  #[test]
  fn test_list_ordered_multimap_get_all() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next(), None);

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");

    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next(), Some(&"value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_get_all_mut() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.get_all(&"key");
    assert_eq!(iter.next(), None);

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");

    let mut iter = map.get_all_mut(&"key");
    assert_eq!(iter.next(), Some(&mut "value1"));
    assert_eq!(iter.next(), Some(&mut "value2"));
    assert_eq!(iter.next(), Some(&mut "value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_get_mut() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.get_mut(&"key"), None);

    map.insert("key", "value");
    assert_eq!(map.get_mut(&"key"), Some(&mut "value"));
  }

  #[test]
  fn test_list_ordered_multimap_insert() {
    let mut map = ListOrderedMultimap::new();
    assert!(!map.contains_key(&"key"));
    assert_eq!(map.get(&"key"), None);

    let value = map.insert("key", "value1");
    assert_eq!(value, None);
    assert!(map.contains_key(&"key"));
    assert_eq!(map.get(&"key"), Some(&"value1"));

    let value = map.insert("key", "value2");
    assert_eq!(value, Some("value1"));
    assert!(map.contains_key(&"key"));
    assert_eq!(map.get(&"key"), Some(&"value2"));
  }

  #[test]
  fn test_list_ordered_multimap_insert_all() {
    let mut map = ListOrderedMultimap::new();
    assert!(!map.contains_key(&"key"));
    assert_eq!(map.get(&"key"), None);

    {
      let mut iter = map.insert_all("key", "value1");
      assert_eq!(iter.next(), None);
    }

    assert!(map.contains_key(&"key"));
    assert_eq!(map.get(&"key"), Some(&"value1"));

    {
      let mut iter = map.insert_all("key", "value2");
      assert_eq!(iter.next(), Some("value1"));
      assert_eq!(iter.next(), None);
    }

    assert!(map.contains_key(&"key"));
    assert_eq!(map.get(&"key"), Some(&"value2"));
  }

  #[test]
  fn test_list_ordered_multimap_is_empty() {
    let mut map = ListOrderedMultimap::new();
    assert!(map.is_empty());

    map.insert("key", "value");
    assert!(!map.is_empty());

    map.remove(&"key");
    assert!(map.is_empty());
  }

  #[test]
  fn test_list_ordered_multimap_iter() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.iter();
    assert_eq!(iter.next(), None);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.iter();
    assert_eq!(iter.next(), Some((&"key1", &"value1")));
    assert_eq!(iter.next(), Some((&"key2", &"value2")));
    assert_eq!(iter.next(), Some((&"key2", &"value3")));
    assert_eq!(iter.next(), Some((&"key1", &"value4")));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_iter_mut() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.iter_mut();
    assert_eq!(iter.next(), None);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.iter_mut();
    assert_eq!(iter.next(), Some((&"key1", &mut "value1")));
    assert_eq!(iter.next(), Some((&"key2", &mut "value2")));
    assert_eq!(iter.next(), Some((&"key2", &mut "value3")));
    assert_eq!(iter.next(), Some((&"key1", &mut "value4")));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_keys() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.keys();
    assert_eq!(iter.next(), None);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.insert("key1", "value3");
    map.insert("key3", "value4");

    let mut iter = map.keys();
    assert_eq!(iter.next(), Some(&"key1"));
    assert_eq!(iter.next(), Some(&"key2"));
    assert_eq!(iter.next(), Some(&"key3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_keys_capacity() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.keys_capacity(), 0);
    map.insert("key", "value");
    assert!(map.keys_capacity() > 0);
  }

  #[test]
  fn test_list_ordered_multimap_keys_len() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.keys_len(), 0);

    map.insert("key1", "value1");
    assert_eq!(map.keys_len(), 1);

    map.insert("key2", "value2");
    assert_eq!(map.keys_len(), 2);

    map.append("key1", "value3");
    assert_eq!(map.keys_len(), 2);

    map.remove(&"key1");
    assert_eq!(map.keys_len(), 1);

    map.remove(&"key2");
    assert_eq!(map.keys_len(), 0);
  }

  #[test]
  fn test_list_ordered_multimap_new() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    assert_eq!(map.keys_capacity(), 0);
    assert_eq!(map.keys_len(), 0);
    assert_eq!(map.values_capacity(), 0);
    assert_eq!(map.values_len(), 0);
  }

  #[test]
  fn test_list_ordered_multimap_pack_to() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::with_capacity(5, 5);
    map.pack_to_fit();
    assert_eq!(map.keys_capacity(), 0);
    assert_eq!(map.values_capacity(), 0);

    let mut map = ListOrderedMultimap::with_capacity(10, 10);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    map.pack_to(5, 5);

    assert_eq!(map.get(&"key1"), Some(&"value1"));
    assert_eq!(map.get(&"key2"), Some(&"value2"));

    assert_eq!(map.keys_capacity(), 5);
    assert_eq!(map.keys_len(), 2);
    assert_eq!(map.values_capacity(), 5);
    assert_eq!(map.values_len(), 4);

    let mut iter = map.iter();
    assert_eq!(iter.next(), Some((&"key1", &"value1")));
    assert_eq!(iter.next(), Some((&"key2", &"value2")));
    assert_eq!(iter.next(), Some((&"key2", &"value3")));
    assert_eq!(iter.next(), Some((&"key1", &"value4")));
    assert_eq!(iter.next(), None);
  }

  #[should_panic]
  #[test]
  fn test_list_ordered_multimap_pack_to_panic_key_capacity() {
    let mut map = ListOrderedMultimap::new();
    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");
    map.pack_to(1, 5);
  }

  #[should_panic]
  #[test]
  fn test_list_ordered_multimap_pack_to_panic_value_capacity() {
    let mut map = ListOrderedMultimap::new();
    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");
    map.pack_to(5, 1);
  }

  #[test]
  fn test_list_ordered_multimap_pack_to_fit() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::with_capacity(5, 5);
    map.pack_to_fit();
    assert_eq!(map.keys_capacity(), 0);
    assert_eq!(map.values_capacity(), 0);

    let mut map = ListOrderedMultimap::with_capacity(5, 5);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    map.pack_to_fit();
    assert_eq!(map.keys_capacity(), 2);
    assert_eq!(map.keys_len(), 2);
    assert_eq!(map.values_capacity(), 4);
    assert_eq!(map.values_len(), 4);

    let mut iter = map.iter();
    assert_eq!(iter.next(), Some((&"key1", &"value1")));
    assert_eq!(iter.next(), Some((&"key2", &"value2")));
    assert_eq!(iter.next(), Some((&"key2", &"value3")));
    assert_eq!(iter.next(), Some((&"key1", &"value4")));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_pairs() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.pairs();

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key1");
    assert_eq!(values.next(), Some(&"value1"));
    assert_eq!(values.next(), Some(&"value4"));
    assert_eq!(values.next(), None);

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key2");
    assert_eq!(values.next(), Some(&"value2"));
    assert_eq!(values.next(), Some(&"value3"));
    assert_eq!(values.next(), None);

    assert!(iter.next().is_none());
  }

  #[test]
  fn test_list_ordered_multimap_pairs_mut() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.pairs_mut();

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key1");
    assert_eq!(values.next(), Some(&mut "value1"));
    assert_eq!(values.next(), Some(&mut "value4"));
    assert_eq!(values.next(), None);

    let (key, mut values) = iter.next().unwrap();
    assert_eq!(key, &"key2");
    assert_eq!(values.next(), Some(&mut "value2"));
    assert_eq!(values.next(), Some(&mut "value3"));
    assert_eq!(values.next(), None);

    assert!(iter.next().is_none());
  }

  #[test]
  fn test_list_ordered_multimap_pop_back() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let (key, value) = map.pop_back().unwrap();
    assert_eq!(key, KeyWrapper::Borrowed(&"key1"));
    assert_eq!(&value, &"value4");
    assert_eq!(map.keys_len(), 2);
    assert_eq!(map.values_len(), 3);

    let (key, value) = map.pop_back().unwrap();
    assert_eq!(key, KeyWrapper::Borrowed(&"key2"));
    assert_eq!(&value, &"value3");
    assert_eq!(map.keys_len(), 2);
    assert_eq!(map.values_len(), 2);

    let (key, value) = map.pop_back().unwrap();
    assert_eq!(key, KeyWrapper::Owned("key2"));
    assert_eq!(&value, &"value2");
    assert_eq!(map.keys_len(), 1);
    assert_eq!(map.values_len(), 1);

    let (key, value) = map.pop_back().unwrap();
    assert_eq!(key, KeyWrapper::Owned("key1"));
    assert_eq!(&value, &"value1");
    assert_eq!(map.keys_len(), 0);
    assert_eq!(map.values_len(), 0);

    assert!(map.pop_back().is_none());
  }

  #[test]
  fn test_list_ordered_multimap_pop_front() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let (key, value) = map.pop_front().unwrap();
    assert_eq!(key, KeyWrapper::Borrowed(&"key1"));
    assert_eq!(&value, &"value1");
    assert_eq!(map.keys_len(), 2);
    assert_eq!(map.values_len(), 3);

    let (key, value) = map.pop_front().unwrap();
    assert_eq!(key, KeyWrapper::Borrowed(&"key2"));
    assert_eq!(&value, &"value2");
    assert_eq!(map.keys_len(), 2);
    assert_eq!(map.values_len(), 2);

    let (key, value) = map.pop_front().unwrap();
    assert_eq!(key, KeyWrapper::Owned("key2"));
    assert_eq!(&value, &"value3");
    assert_eq!(map.keys_len(), 1);
    assert_eq!(map.values_len(), 1);

    let (key, value) = map.pop_front().unwrap();
    assert_eq!(key, KeyWrapper::Owned("key1"));
    assert_eq!(&value, &"value4");
    assert_eq!(map.keys_len(), 0);
    assert_eq!(map.values_len(), 0);

    assert!(map.pop_front().is_none());
  }

  #[test]
  fn test_list_ordered_multimap_remove() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.remove(&"key"), None);

    map.insert("key", "value1");
    map.append("key", "value2");
    assert_eq!(map.remove(&"key"), Some("value1"));
    assert_eq!(map.remove(&"key"), None);
  }

  #[test]
  fn test_list_ordered_multimap_remove_all() {
    let mut map = ListOrderedMultimap::new();

    {
      let mut iter = map.remove_all(&"key");
      assert_eq!(iter.next(), None);
    }

    map.insert("key", "value1");
    map.append("key", "value2");

    {
      let mut iter = map.remove_all(&"key");
      assert_eq!(iter.next(), Some("value1"));
      assert_eq!(iter.next(), Some("value2"));
      assert_eq!(iter.next(), None);
    }

    let mut iter = map.remove_all(&"key");
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_remove_entry() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.remove_entry(&"key"), None);

    map.insert("key", "value1");
    map.append("key", "value2");
    assert_eq!(map.remove_entry(&"key"), Some(("key", "value1")));
    assert_eq!(map.remove_entry(&"key"), None);
  }

  #[test]
  fn test_list_ordered_multimap_remove_entry_all() {
    let mut map = ListOrderedMultimap::new();

    {
      let entry = map.remove_entry_all(&"key");
      assert!(entry.is_none());
    }

    map.insert("key", "value1");
    map.append("key", "value2");

    {
      let (key, mut iter) = map.remove_entry_all(&"key").unwrap();
      assert_eq!(key, "key");
      assert_eq!(iter.next(), Some("value1"));
      assert_eq!(iter.next(), Some("value2"));
      assert_eq!(iter.next(), None);
    }

    let entry = map.remove_entry_all(&"key");
    assert!(entry.is_none());
  }

  #[test]
  fn test_list_ordered_multimap_reserve_keys() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    assert_eq!(map.keys_capacity(), 0);

    map.reserve_keys(5);
    assert!(map.keys_capacity() >= 5);

    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::with_capacity(5, 5);
    assert_eq!(map.keys_capacity(), 5);

    map.reserve_keys(2);
    assert_eq!(map.keys_capacity(), 5);
  }

  #[test]
  fn test_list_ordered_multimap_reserve_values() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    assert_eq!(map.values_capacity(), 0);

    map.reserve_values(5);
    assert!(map.values_capacity() >= 5);

    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::with_capacity(5, 5);
    assert_eq!(map.values_capacity(), 5);

    map.reserve_values(2);
    assert_eq!(map.values_capacity(), 5);
  }

  #[test]
  fn test_list_ordered_multimap_retain() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", 1);
    map.insert("key2", 5);
    map.append("key1", -1);
    map.insert("key3", -10);
    map.insert("key4", 1);
    map.append("key4", -1);
    map.append("key4", 1);

    map.retain(|_, &mut value| value >= 0);

    let mut iter = map.iter();
    assert_eq!(iter.next(), Some((&"key1", &1)));
    assert_eq!(iter.next(), Some((&"key2", &5)));
    assert_eq!(iter.next(), Some((&"key4", &1)));
    assert_eq!(iter.next(), Some((&"key4", &1)));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_values() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.iter();
    assert_eq!(iter.next(), None);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.values();
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next(), Some(&"value3"));
    assert_eq!(iter.next(), Some(&"value4"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_values_mut() {
    let mut map = ListOrderedMultimap::new();

    let mut iter = map.iter();
    assert_eq!(iter.next(), None);

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.values_mut();
    assert_eq!(iter.next(), Some(&mut "value1"));
    assert_eq!(iter.next(), Some(&mut "value2"));
    assert_eq!(iter.next(), Some(&mut "value3"));
    assert_eq!(iter.next(), Some(&mut "value4"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_list_ordered_multimap_values_capacity() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.values_capacity(), 0);
    map.insert("key", "value");
    assert!(map.values_capacity() > 0);
  }

  #[test]
  fn test_list_ordered_multimap_values_len() {
    let mut map = ListOrderedMultimap::new();
    assert_eq!(map.values_len(), 0);

    map.insert("key1", "value1");
    assert_eq!(map.values_len(), 1);

    map.insert("key2", "value2");
    assert_eq!(map.values_len(), 2);

    map.append("key1", "value3");
    assert_eq!(map.values_len(), 3);

    map.remove(&"key1");
    assert_eq!(map.values_len(), 1);

    map.remove(&"key2");
    assert_eq!(map.values_len(), 0);
  }

  #[test]
  fn test_list_ordered_multimap_with_capacity() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::with_capacity(1, 2);
    assert!(map.keys_capacity() >= 1);
    assert_eq!(map.keys_len(), 0);
    assert!(map.values_capacity() >= 2);
    assert_eq!(map.values_len(), 0);
  }

  #[test]
  fn test_list_ordered_multimap_with_capacity_and_hasher() {
    let state = RandomState::new();
    let map: ListOrderedMultimap<&str, &str> =
      ListOrderedMultimap::with_capacity_and_hasher(1, 2, state);
    assert!(map.keys_capacity() >= 1);
    assert_eq!(map.keys_len(), 0);
    assert!(map.values_capacity() >= 2);
    assert_eq!(map.values_len(), 0);
  }

  #[test]
  fn test_occupied_entry_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value1");
    map.append("key", "value2");
    map.append("key", "value3");
    map.append("key", "value4");

    let entry = match map.entry("key") {
      Entry::Occupied(entry) => entry,
      _ => panic!("expected occupied entry"),
    };

    assert_eq!(
      format!("{entry:?}"),
      "OccupiedEntry { \
             key: \"key\", \
             values: EntryValues([\"value1\", \"value2\", \"value3\", \"value4\"]) \
             }"
    );
  }

  #[test]
  fn test_vacant_entry_debug() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let entry = match map.entry("key") {
      Entry::Vacant(entry) => entry,
      _ => panic!("expected vacant entry"),
    };

    assert_eq!(format!("{entry:?}"), r#"VacantEntry("key")"#);
  }

  #[test]
  fn test_values_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.values();
    assert_eq!(
      format!("{iter:?}"),
      r#"Values(["value1", "value2", "value3", "value4"])"#
    );
  }

  #[test]
  fn test_values_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.values();
    assert_eq!(iter.next(), Some(&"value1"));
    assert_eq!(iter.next_back(), Some(&"value4"));
    assert_eq!(iter.next(), Some(&"value2"));
    assert_eq!(iter.next_back(), Some(&"value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_values_empty() {
    let map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.values();
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_values_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.values();
    assert_eq!(iter.next(), Some(&"value"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_values_mut_debug() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let iter = map.values_mut();
    assert_eq!(
      format!("{iter:?}"),
      r#"ValuesMut(["value1", "value2", "value3", "value4"])"#
    );
  }

  #[test]
  fn test_values_mut_double_ended() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.values_mut();
    assert_eq!(iter.next(), Some(&mut "value1"));
    assert_eq!(iter.next_back(), Some(&mut "value4"));
    assert_eq!(iter.next(), Some(&mut "value2"));
    assert_eq!(iter.next_back(), Some(&mut "value3"));
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_values_mut_empty() {
    let mut map: ListOrderedMultimap<&str, &str> = ListOrderedMultimap::new();
    let mut iter = map.values_mut();
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_values_mut_fused() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key", "value");

    let mut iter = map.values_mut();
    assert_eq!(iter.next(), Some(&mut "value"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
  }

  #[test]
  fn test_values_mut_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.values_mut();
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
  fn test_values_size_hint() {
    let mut map = ListOrderedMultimap::new();

    map.insert("key1", "value1");
    map.append("key2", "value2");
    map.append("key2", "value3");
    map.append("key1", "value4");

    let mut iter = map.values();
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

  #[should_panic]
  #[test]
  fn test_dummy_hasher_finish() {
    let hasher = DummyHasher;
    hasher.finish();
  }

  #[should_panic]
  #[test]
  fn test_dummy_hasher_write() {
    let mut hasher = DummyHasher;
    hasher.write(&[]);
  }
}
