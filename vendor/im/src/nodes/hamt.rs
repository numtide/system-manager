// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::borrow::Borrow;
use std::fmt;
use std::hash::{BuildHasher, Hash, Hasher};
use std::iter::FusedIterator;
use std::slice::{Iter as SliceIter, IterMut as SliceIterMut};
use std::{mem, ptr};

use bitmaps::Bits;
use sized_chunks::sparse_chunk::{Iter as ChunkIter, IterMut as ChunkIterMut, SparseChunk};
use typenum::{Pow, Unsigned, U2};

use crate::config::HashLevelSize;
use crate::util::{clone_ref, Pool, PoolClone, PoolDefault, PoolRef, Ref};

pub(crate) type HashWidth = <U2 as Pow<HashLevelSize>>::Output;
pub(crate) type HashBits = <HashWidth as Bits>::Store; // a uint of HASH_SIZE bits
pub(crate) const HASH_SHIFT: usize = HashLevelSize::USIZE;
pub(crate) const HASH_WIDTH: usize = HashWidth::USIZE;
pub(crate) const HASH_MASK: HashBits = (HASH_WIDTH - 1) as HashBits;

pub(crate) fn hash_key<K: Hash + ?Sized, S: BuildHasher>(bh: &S, key: &K) -> HashBits {
    let mut hasher = bh.build_hasher();
    key.hash(&mut hasher);
    hasher.finish() as HashBits
}

#[inline]
fn mask(hash: HashBits, shift: usize) -> HashBits {
    hash >> shift & HASH_MASK
}

pub trait HashValue {
    type Key: Eq;

    fn extract_key(&self) -> &Self::Key;
    fn ptr_eq(&self, other: &Self) -> bool;
}

#[derive(Clone)]
pub(crate) struct Node<A> {
    data: SparseChunk<Entry<A>, HashWidth>,
}

#[allow(unsafe_code)]
impl<A> PoolDefault for Node<A> {
    #[cfg(feature = "pool")]
    unsafe fn default_uninit(target: &mut mem::MaybeUninit<Self>) {
        SparseChunk::default_uninit(
            target
                .as_mut_ptr()
                .cast::<mem::MaybeUninit<SparseChunk<Entry<A>, HashWidth>>>()
                .as_mut()
                .unwrap(),
        )
    }
}

#[allow(unsafe_code)]
impl<A> PoolClone for Node<A>
where
    A: Clone,
{
    #[cfg(feature = "pool")]
    unsafe fn clone_uninit(&self, target: &mut mem::MaybeUninit<Self>) {
        self.data.clone_uninit(
            target
                .as_mut_ptr()
                .cast::<mem::MaybeUninit<SparseChunk<Entry<A>, HashWidth>>>()
                .as_mut()
                .unwrap(),
        )
    }
}

#[derive(Clone)]
pub(crate) struct CollisionNode<A> {
    hash: HashBits,
    data: Vec<A>,
}

pub(crate) enum Entry<A> {
    Value(A, HashBits),
    Collision(Ref<CollisionNode<A>>),
    Node(PoolRef<Node<A>>),
}

impl<A: Clone> Clone for Entry<A> {
    fn clone(&self) -> Self {
        match self {
            Entry::Value(value, hash) => Entry::Value(value.clone(), *hash),
            Entry::Collision(coll) => Entry::Collision(coll.clone()),
            Entry::Node(node) => Entry::Node(node.clone()),
        }
    }
}

impl<A> Entry<A> {
    fn is_value(&self) -> bool {
        matches!(self, Entry::Value(_, _))
    }

    fn unwrap_value(self) -> A {
        match self {
            Entry::Value(a, _) => a,
            _ => panic!("nodes::hamt::Entry::unwrap_value: unwrapped a non-value"),
        }
    }

    fn from_node(pool: &Pool<Node<A>>, node: Node<A>) -> Self {
        Entry::Node(PoolRef::new(pool, node))
    }
}

impl<A> From<CollisionNode<A>> for Entry<A> {
    fn from(node: CollisionNode<A>) -> Self {
        Entry::Collision(Ref::new(node))
    }
}

impl<A> Default for Node<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A> Node<A> {
    #[inline]
    pub(crate) fn new() -> Self {
        Node {
            data: SparseChunk::new(),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub(crate) fn unit(index: usize, value: Entry<A>) -> Self {
        Node {
            data: SparseChunk::unit(index, value),
        }
    }

    #[inline]
    pub(crate) fn pair(index1: usize, value1: Entry<A>, index2: usize, value2: Entry<A>) -> Self {
        Node {
            data: SparseChunk::pair(index1, value1, index2, value2),
        }
    }

    #[inline]
    pub(crate) fn single_child(pool: &Pool<Node<A>>, index: usize, node: Self) -> Self {
        Node {
            data: SparseChunk::unit(index, Entry::from_node(pool, node)),
        }
    }

    fn pop(&mut self) -> Entry<A> {
        self.data.pop().unwrap()
    }
}

impl<A: HashValue> Node<A> {
    fn merge_values(
        pool: &Pool<Node<A>>,
        value1: A,
        hash1: HashBits,
        value2: A,
        hash2: HashBits,
        shift: usize,
    ) -> Self {
        let index1 = mask(hash1, shift) as usize;
        let index2 = mask(hash2, shift) as usize;
        if index1 != index2 {
            // Both values fit on the same level.
            Node::pair(
                index1,
                Entry::Value(value1, hash1),
                index2,
                Entry::Value(value2, hash2),
            )
        } else if shift + HASH_SHIFT >= HASH_WIDTH {
            // If we're at the bottom, we've got a collision.
            Node::unit(
                index1,
                Entry::from(CollisionNode::new(hash1, value1, value2)),
            )
        } else {
            // Pass the values down a level.
            let node = Node::merge_values(pool, value1, hash1, value2, hash2, shift + HASH_SHIFT);
            Node::single_child(pool, index1, node)
        }
    }

    pub(crate) fn get<BK>(&self, hash: HashBits, shift: usize, key: &BK) -> Option<&A>
    where
        BK: Eq + ?Sized,
        A::Key: Borrow<BK>,
    {
        let index = mask(hash, shift) as usize;
        if let Some(entry) = self.data.get(index) {
            match entry {
                Entry::Value(ref value, _) => {
                    if key == value.extract_key().borrow() {
                        Some(value)
                    } else {
                        None
                    }
                }
                Entry::Collision(ref coll) => coll.get(key),
                Entry::Node(ref child) => child.get(hash, shift + HASH_SHIFT, key),
            }
        } else {
            None
        }
    }

    pub(crate) fn get_mut<BK>(
        &mut self,
        pool: &Pool<Node<A>>,
        hash: HashBits,
        shift: usize,
        key: &BK,
    ) -> Option<&mut A>
    where
        A: Clone,
        BK: Eq + ?Sized,
        A::Key: Borrow<BK>,
    {
        let index = mask(hash, shift) as usize;
        if let Some(entry) = self.data.get_mut(index) {
            match entry {
                Entry::Value(ref mut value, _) => {
                    if key == value.extract_key().borrow() {
                        Some(value)
                    } else {
                        None
                    }
                }
                Entry::Collision(ref mut coll_ref) => {
                    let coll = Ref::make_mut(coll_ref);
                    coll.get_mut(key)
                }
                Entry::Node(ref mut child_ref) => {
                    let child = PoolRef::make_mut(pool, child_ref);
                    child.get_mut(pool, hash, shift + HASH_SHIFT, key)
                }
            }
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &mut self,
        pool: &Pool<Node<A>>,
        hash: HashBits,
        shift: usize,
        value: A,
    ) -> Option<A>
    where
        A: Clone,
    {
        let index = mask(hash, shift) as usize;
        if let Some(entry) = self.data.get_mut(index) {
            let mut fallthrough = false;
            // Value is here
            match entry {
                // Update value or create a subtree
                Entry::Value(ref current, _) => {
                    if current.extract_key() == value.extract_key() {
                        // If we have a key match, fall through to the outer
                        // level where we replace the current value. If we
                        // don't, fall through to the inner level where we merge
                        // some nodes.
                        fallthrough = true;
                    }
                }
                // There's already a collision here.
                Entry::Collision(ref mut collision) => {
                    let coll = Ref::make_mut(collision);
                    return coll.insert(value);
                }
                Entry::Node(ref mut child_ref) => {
                    // Child node
                    let child = PoolRef::make_mut(pool, child_ref);
                    return child.insert(pool, hash, shift + HASH_SHIFT, value);
                }
            }
            if !fallthrough {
                // If we get here, we're looking at a value entry that needs a merge.
                // We're going to be unsafe and pry it out of the reference, trusting
                // that we overwrite it with the merged node.
                #[allow(unsafe_code)]
                let old_entry = unsafe { ptr::read(entry) };
                if shift + HASH_SHIFT >= HASH_WIDTH {
                    // We're at the lowest level, need to set up a collision node.
                    let coll = CollisionNode::new(hash, old_entry.unwrap_value(), value);
                    #[allow(unsafe_code)]
                    unsafe {
                        ptr::write(entry, Entry::from(coll))
                    };
                } else if let Entry::Value(old_value, old_hash) = old_entry {
                    let node = Node::merge_values(
                        pool,
                        old_value,
                        old_hash,
                        value,
                        hash,
                        shift + HASH_SHIFT,
                    );
                    #[allow(unsafe_code)]
                    unsafe {
                        ptr::write(entry, Entry::from_node(pool, node))
                    };
                } else {
                    unreachable!()
                }
                return None;
            }
        }
        // If we get here, either we found nothing at this index, in which case
        // we insert a new entry, or we hit a value entry with the same key, in
        // which case we replace it.
        self.data
            .insert(index, Entry::Value(value, hash))
            .map(Entry::unwrap_value)
    }

    pub(crate) fn remove<BK>(
        &mut self,
        pool: &Pool<Node<A>>,
        hash: HashBits,
        shift: usize,
        key: &BK,
    ) -> Option<A>
    where
        A: Clone,
        BK: Eq + ?Sized,
        A::Key: Borrow<BK>,
    {
        let index = mask(hash, shift) as usize;
        let mut new_node = None;
        let mut removed = None;
        if let Some(entry) = self.data.get_mut(index) {
            match entry {
                Entry::Value(ref value, _) => {
                    if key != value.extract_key().borrow() {
                        // Key wasn't in the map.
                        return None;
                    } // Otherwise, fall through to the removal.
                }
                Entry::Collision(ref mut coll_ref) => {
                    let coll = Ref::make_mut(coll_ref);
                    removed = coll.remove(key);
                    if coll.len() == 1 {
                        new_node = Some(coll.pop());
                    } else {
                        return removed;
                    }
                }
                Entry::Node(ref mut child_ref) => {
                    let child = PoolRef::make_mut(pool, child_ref);
                    match child.remove(pool, hash, shift + HASH_SHIFT, key) {
                        None => {
                            return None;
                        }
                        Some(value) => {
                            if child.len() == 1
                                && child.data[child.data.first_index().unwrap()].is_value()
                            {
                                // If the child now contains only a single value node,
                                // pull it up one level and discard the child.
                                removed = Some(value);
                                new_node = Some(child.pop());
                            } else {
                                return Some(value);
                            }
                        }
                    }
                }
            }
        }
        if let Some(node) = new_node {
            self.data.insert(index, node);
            return removed;
        }
        self.data.remove(index).map(Entry::unwrap_value)
    }
}

impl<A: HashValue> CollisionNode<A> {
    fn new(hash: HashBits, value1: A, value2: A) -> Self {
        CollisionNode {
            hash,
            data: vec![value1, value2],
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.data.len()
    }

    fn get<BK>(&self, key: &BK) -> Option<&A>
    where
        BK: Eq + ?Sized,
        A::Key: Borrow<BK>,
    {
        for entry in &self.data {
            if key == entry.extract_key().borrow() {
                return Some(entry);
            }
        }
        None
    }

    fn get_mut<BK>(&mut self, key: &BK) -> Option<&mut A>
    where
        BK: Eq + ?Sized,
        A::Key: Borrow<BK>,
    {
        for entry in &mut self.data {
            if key == entry.extract_key().borrow() {
                return Some(entry);
            }
        }
        None
    }

    fn insert(&mut self, value: A) -> Option<A> {
        for item in &mut self.data {
            if value.extract_key() == item.extract_key() {
                return Some(mem::replace(item, value));
            }
        }
        self.data.push(value);
        None
    }

    fn remove<BK>(&mut self, key: &BK) -> Option<A>
    where
        BK: Eq + ?Sized,
        A::Key: Borrow<BK>,
    {
        let mut loc = None;
        for (index, item) in self.data.iter().enumerate() {
            if key == item.extract_key().borrow() {
                loc = Some(index);
            }
        }
        if let Some(index) = loc {
            Some(self.data.remove(index))
        } else {
            None
        }
    }

    fn pop(&mut self) -> Entry<A> {
        Entry::Value(self.data.pop().unwrap(), self.hash)
    }
}

// Ref iterator

pub(crate) struct Iter<'a, A> {
    count: usize,
    stack: Vec<ChunkIter<'a, Entry<A>, HashWidth>>,
    current: ChunkIter<'a, Entry<A>, HashWidth>,
    collision: Option<(HashBits, SliceIter<'a, A>)>,
}

impl<'a, A> Iter<'a, A>
where
    A: 'a,
{
    pub(crate) fn new(root: &'a Node<A>, size: usize) -> Self {
        Iter {
            count: size,
            stack: Vec::with_capacity((HASH_WIDTH / HASH_SHIFT) + 1),
            current: root.data.iter(),
            collision: None,
        }
    }
}

impl<'a, A> Iterator for Iter<'a, A>
where
    A: 'a,
{
    type Item = (&'a A, HashBits);

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        if self.collision.is_some() {
            if let Some((hash, ref mut coll)) = self.collision {
                match coll.next() {
                    None => {}
                    Some(value) => {
                        self.count -= 1;
                        return Some((value, hash));
                    }
                }
            }
            self.collision = None;
            return self.next();
        }
        match self.current.next() {
            Some(Entry::Value(value, hash)) => {
                self.count -= 1;
                Some((value, *hash))
            }
            Some(Entry::Node(child)) => {
                let current = mem::replace(&mut self.current, child.data.iter());
                self.stack.push(current);
                self.next()
            }
            Some(Entry::Collision(coll)) => {
                self.collision = Some((coll.hash, coll.data.iter()));
                self.next()
            }
            None => match self.stack.pop() {
                None => None,
                Some(iter) => {
                    self.current = iter;
                    self.next()
                }
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}

impl<'a, A> ExactSizeIterator for Iter<'a, A> where A: 'a {}

impl<'a, A> FusedIterator for Iter<'a, A> where A: 'a {}

// Mut ref iterator

pub(crate) struct IterMut<'a, A> {
    count: usize,
    pool: Pool<Node<A>>,
    stack: Vec<ChunkIterMut<'a, Entry<A>, HashWidth>>,
    current: ChunkIterMut<'a, Entry<A>, HashWidth>,
    collision: Option<(HashBits, SliceIterMut<'a, A>)>,
}

impl<'a, A> IterMut<'a, A>
where
    A: 'a,
{
    pub(crate) fn new(pool: &Pool<Node<A>>, root: &'a mut Node<A>, size: usize) -> Self {
        IterMut {
            count: size,
            pool: pool.clone(),
            stack: Vec::with_capacity((HASH_WIDTH / HASH_SHIFT) + 1),
            current: root.data.iter_mut(),
            collision: None,
        }
    }
}

impl<'a, A> Iterator for IterMut<'a, A>
where
    A: Clone + 'a,
{
    type Item = (&'a mut A, HashBits);

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        if self.collision.is_some() {
            if let Some((hash, ref mut coll)) = self.collision {
                match coll.next() {
                    None => {}
                    Some(value) => {
                        self.count -= 1;
                        return Some((value, hash));
                    }
                }
            }
            self.collision = None;
            return self.next();
        }
        match self.current.next() {
            Some(Entry::Value(value, hash)) => {
                self.count -= 1;
                Some((value, *hash))
            }
            Some(Entry::Node(child_ref)) => {
                let child = PoolRef::make_mut(&self.pool, child_ref);
                let current = mem::replace(&mut self.current, child.data.iter_mut());
                self.stack.push(current);
                self.next()
            }
            Some(Entry::Collision(coll_ref)) => {
                let coll = Ref::make_mut(coll_ref);
                self.collision = Some((coll.hash, coll.data.iter_mut()));
                self.next()
            }
            None => match self.stack.pop() {
                None => None,
                Some(iter) => {
                    self.current = iter;
                    self.next()
                }
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}

impl<'a, A> ExactSizeIterator for IterMut<'a, A> where A: Clone + 'a {}

impl<'a, A> FusedIterator for IterMut<'a, A> where A: Clone + 'a {}

// Consuming iterator

pub(crate) struct Drain<A>
where
    A: HashValue,
{
    count: usize,
    pool: Pool<Node<A>>,
    stack: Vec<PoolRef<Node<A>>>,
    current: PoolRef<Node<A>>,
    collision: Option<CollisionNode<A>>,
}

impl<A> Drain<A>
where
    A: HashValue,
{
    pub(crate) fn new(pool: &Pool<Node<A>>, root: PoolRef<Node<A>>, size: usize) -> Self {
        Drain {
            count: size,
            pool: pool.clone(),
            stack: vec![],
            current: root,
            collision: None,
        }
    }
}

impl<A> Iterator for Drain<A>
where
    A: HashValue + Clone,
{
    type Item = (A, HashBits);

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        if self.collision.is_some() {
            if let Some(ref mut coll) = self.collision {
                if let Some(value) = coll.data.pop() {
                    self.count -= 1;
                    return Some((value, coll.hash));
                }
            }
            self.collision = None;
            return self.next();
        }
        match PoolRef::make_mut(&self.pool, &mut self.current).data.pop() {
            Some(Entry::Value(value, hash)) => {
                self.count -= 1;
                Some((value, hash))
            }
            Some(Entry::Collision(coll_ref)) => {
                self.collision = Some(clone_ref(coll_ref));
                self.next()
            }
            Some(Entry::Node(child)) => {
                let parent = mem::replace(&mut self.current, child);
                self.stack.push(parent);
                self.next()
            }
            None => match self.stack.pop() {
                None => None,
                Some(parent) => {
                    self.current = parent;
                    self.next()
                }
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}

impl<A: HashValue> ExactSizeIterator for Drain<A> where A: Clone {}

impl<A: HashValue> FusedIterator for Drain<A> where A: Clone {}

impl<A: HashValue + fmt::Debug> fmt::Debug for Node<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Node[ ")?;
        for i in self.data.indices() {
            write!(f, "{}: ", i)?;
            match &self.data[i] {
                Entry::Value(v, h) => write!(f, "{:?} :: {}, ", v, h)?,
                Entry::Collision(c) => write!(f, "Coll{:?} :: {}", c.data, c.hash)?,
                Entry::Node(n) => write!(f, "{:?}, ", n)?,
            }
        }
        write!(f, " ]")
    }
}
