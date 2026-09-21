// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::mem::replace;
use std::ops::Range;

use crate::nodes::chunk::{Chunk, CHUNK_SIZE};
use crate::util::{
    Pool, PoolRef,
    Side::{self, Left, Right},
};
use crate::vector::RRBPool;

use self::Entry::*;

pub(crate) const NODE_SIZE: usize = CHUNK_SIZE;

#[derive(Debug)]
enum Size {
    Size(usize),
    Table(PoolRef<Chunk<usize>>),
}

impl Clone for Size {
    fn clone(&self) -> Self {
        match *self {
            Size::Size(size) => Size::Size(size),
            Size::Table(ref table) => Size::Table(table.clone()),
        }
    }
}

impl Size {
    fn size(&self) -> usize {
        match self {
            Size::Size(s) => *s,
            Size::Table(sizes) => *sizes.last().unwrap_or(&0),
        }
    }

    fn is_size(&self) -> bool {
        match self {
            Size::Size(_) => true,
            Size::Table(_) => false,
        }
    }

    fn table_from_size(pool: &Pool<Chunk<usize>>, level: usize, size: usize) -> Self {
        let mut chunk = Chunk::new();
        let mut remaining = size;
        if let Some(child_size) = NODE_SIZE.checked_pow(level as u32) {
            while remaining > child_size {
                let next_value = chunk.last().unwrap_or(&0) + child_size;
                chunk.push_back(next_value);
                remaining -= child_size;
            }
        }
        if remaining > 0 {
            let next_value = chunk.last().unwrap_or(&0) + remaining;
            chunk.push_back(next_value);
        }
        Size::Table(PoolRef::new(pool, chunk))
    }

    fn push(&mut self, pool: &Pool<Chunk<usize>>, side: Side, level: usize, value: usize) {
        let size = match self {
            Size::Size(ref mut size) => match side {
                Left => *size,
                Right => {
                    *size += value;
                    return;
                }
            },
            Size::Table(ref mut size_ref) => {
                let size_table = PoolRef::make_mut(pool, size_ref);
                debug_assert!(size_table.len() < NODE_SIZE);
                match side {
                    Left => {
                        for entry in size_table.iter_mut() {
                            *entry += value;
                        }
                        size_table.push_front(value);
                    }
                    Right => {
                        let prev = *(size_table.last().unwrap_or(&0));
                        size_table.push_back(value + prev);
                    }
                }
                return;
            }
        };
        *self = Size::table_from_size(pool, level, size);
        self.push(pool, side, level, value);
    }

    fn pop(&mut self, pool: &Pool<Chunk<usize>>, side: Side, level: usize, value: usize) {
        let size = match self {
            Size::Size(ref mut size) => match side {
                Left => *size,
                Right => {
                    *size -= value;
                    return;
                }
            },
            Size::Table(ref mut size_ref) => {
                let size_table = PoolRef::make_mut(pool, size_ref);
                match side {
                    Left => {
                        let first = size_table.pop_front();
                        debug_assert_eq!(value, first);
                        for entry in size_table.iter_mut() {
                            *entry -= value;
                        }
                    }
                    Right => {
                        let pop = size_table.pop_back();
                        let last = size_table.last().unwrap_or(&0);
                        debug_assert_eq!(value, pop - last);
                    }
                }
                return;
            }
        };
        *self = Size::table_from_size(pool, level, size);
        self.pop(pool, side, level, value);
    }

    fn update(&mut self, pool: &Pool<Chunk<usize>>, index: usize, level: usize, value: isize) {
        let size = match self {
            Size::Size(ref size) => *size,
            Size::Table(ref mut size_ref) => {
                let size_table = PoolRef::make_mut(pool, size_ref);
                for entry in size_table.iter_mut().skip(index) {
                    *entry = (*entry as isize + value) as usize;
                }
                return;
            }
        };
        *self = Size::table_from_size(pool, level, size);
        self.update(pool, index, level, value);
    }
}

pub(crate) enum PushResult<A> {
    Full(A, usize),
    Done,
}

pub(crate) enum PopResult<A> {
    Done(A),
    Drained(A),
    Empty,
}

pub(crate) enum SplitResult {
    Dropped(usize),
    OutOfBounds,
}

// Invariants: Nodes only at level > 0, Values/Empty only at level = 0
enum Entry<A> {
    Nodes(Size, PoolRef<Chunk<Node<A>>>),
    Values(PoolRef<Chunk<A>>),
    Empty,
}

impl<A: Clone> Clone for Entry<A> {
    fn clone(&self) -> Self {
        match *self {
            Nodes(ref size, ref nodes) => Nodes(size.clone(), nodes.clone()),
            Values(ref values) => Values(values.clone()),
            Empty => Empty,
        }
    }
}

impl<A: Clone> Entry<A> {
    fn len(&self) -> usize {
        match self {
            Nodes(_, ref nodes) => nodes.len(),
            Values(ref values) => values.len(),
            Empty => 0,
        }
    }

    fn is_full(&self) -> bool {
        match self {
            Nodes(_, ref nodes) => nodes.is_full(),
            Values(ref values) => values.is_full(),
            Empty => false,
        }
    }

    fn unwrap_values(&self) -> &Chunk<A> {
        match self {
            Values(ref values) => values,
            _ => panic!("rrb::Entry::unwrap_values: expected values, found nodes"),
        }
    }

    fn unwrap_nodes(&self) -> &Chunk<Node<A>> {
        match self {
            Nodes(_, ref nodes) => nodes,
            _ => panic!("rrb::Entry::unwrap_nodes: expected nodes, found values"),
        }
    }

    fn unwrap_values_mut(&mut self, pool: &RRBPool<A>) -> &mut Chunk<A> {
        match self {
            Values(ref mut values) => PoolRef::make_mut(&pool.value_pool, values),
            _ => panic!("rrb::Entry::unwrap_values_mut: expected values, found nodes"),
        }
    }

    fn unwrap_nodes_mut(&mut self, pool: &RRBPool<A>) -> &mut Chunk<Node<A>> {
        match self {
            Nodes(_, ref mut nodes) => PoolRef::make_mut(&pool.node_pool, nodes),
            _ => panic!("rrb::Entry::unwrap_nodes_mut: expected nodes, found values"),
        }
    }

    fn values(self) -> Chunk<A> {
        match self {
            Values(values) => PoolRef::unwrap_or_clone(values),
            _ => panic!("rrb::Entry::values: expected values, found nodes"),
        }
    }

    fn nodes(self) -> Chunk<Node<A>> {
        match self {
            Nodes(_, nodes) => PoolRef::unwrap_or_clone(nodes),
            _ => panic!("rrb::Entry::nodes: expected nodes, found values"),
        }
    }

    fn is_empty_node(&self) -> bool {
        matches!(self, Empty)
    }
}

// Node

pub(crate) struct Node<A> {
    children: Entry<A>,
}

impl<A: Clone> Clone for Node<A> {
    fn clone(&self) -> Self {
        Node {
            children: self.children.clone(),
        }
    }
}

impl<A: Clone> Default for Node<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Clone> Node<A> {
    pub(crate) fn new() -> Self {
        Node { children: Empty }
    }

    pub(crate) fn parent(pool: &RRBPool<A>, level: usize, children: Chunk<Self>) -> Self {
        let size = {
            let mut size = Size::Size(0);
            let mut it = children.iter().peekable();
            loop {
                match it.next() {
                    None => break,
                    Some(child) => {
                        if size.is_size()
                            && !child.is_completely_dense(level - 1)
                            && it.peek().is_some()
                        {
                            size = Size::table_from_size(&pool.size_pool, level, size.size());
                        }
                        size.push(&pool.size_pool, Right, level, child.len())
                    }
                }
            }
            size
        };
        Node {
            children: Nodes(size, PoolRef::new(&pool.node_pool, children)),
        }
    }

    pub(crate) fn clear_node(&mut self) {
        self.children = Empty;
    }

    pub(crate) fn from_chunk(pool: &RRBPool<A>, level: usize, chunk: PoolRef<Chunk<A>>) -> Self {
        let node = Node {
            children: Values(chunk),
        };
        node.elevate(pool, level)
    }

    pub(crate) fn single_parent(pool: &RRBPool<A>, node: Self) -> Self {
        let size = if node.is_dense() {
            Size::Size(node.len())
        } else {
            let size_table = Chunk::unit(node.len());
            Size::Table(PoolRef::new(&pool.size_pool, size_table))
        };
        let children = PoolRef::new(&pool.node_pool, Chunk::unit(node));
        Node {
            children: Nodes(size, children),
        }
    }

    pub(crate) fn join_dense(pool: &RRBPool<A>, left: Self, right: Self) -> Self {
        let left_len = left.len();
        let right_len = right.len();
        Node {
            children: {
                let children = PoolRef::new(&pool.node_pool, Chunk::pair(left, right));
                Nodes(Size::Size(left_len + right_len), children)
            },
        }
    }

    pub(crate) fn elevate(self, pool: &RRBPool<A>, level_increment: usize) -> Self {
        if level_increment > 0 {
            Self::single_parent(pool, self.elevate(pool, level_increment - 1))
        } else {
            self
        }
    }

    pub(crate) fn join_branches(self, pool: &RRBPool<A>, right: Self, level: usize) -> Self {
        let left_len = self.len();
        let right_len = right.len();
        let size = if self.is_completely_dense(level) && right.is_dense() {
            Size::Size(left_len + right_len)
        } else {
            let size_table = Chunk::pair(left_len, left_len + right_len);
            Size::Table(PoolRef::new(&pool.size_pool, size_table))
        };
        Node {
            children: {
                let children = Chunk::pair(self, right);
                Nodes(size, PoolRef::new(&pool.node_pool, children))
            },
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self.children {
            Entry::Nodes(Size::Size(size), _) => size,
            Entry::Nodes(Size::Table(ref size_table), _) => *(size_table.last().unwrap_or(&0)),
            Entry::Values(ref values) => values.len(),
            Entry::Empty => 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn is_single(&self) -> bool {
        self.children.len() == 1
    }

    pub(crate) fn is_full(&self) -> bool {
        self.children.is_full()
    }

    #[allow(dead_code)] // this is only used by tests
    pub(crate) fn number_of_children(&self) -> usize {
        self.children.len()
    }

    pub(crate) fn first_child(&self) -> &Self {
        self.children.unwrap_nodes().first().unwrap()
    }

    /// True if the node is dense and so doesn't have a size table
    fn is_dense(&self) -> bool {
        !matches!(self.children, Entry::Nodes(Size::Table(_), _))
    }

    /// True if the node and its children are dense and at capacity
    // TODO can use this technique to quickly test if a Size::Table
    // should be converted back to a Size::Size
    fn is_completely_dense(&self, level: usize) -> bool {
        // Size of a full node is NODE_SIZE at level 0, NODE_SIZE² at
        // level 1, etc.
        if let Some(expected_size) = NODE_SIZE.checked_pow(level as u32 + 1) {
            self.size() == expected_size
        } else {
            // We overflowed a usize, there's no way we can be completely dense as we know the size
            // fits in a usize.
            false
        }
    }

    #[inline]
    fn size(&self) -> usize {
        match self.children {
            Entry::Nodes(ref size, _) => size.size(),
            Entry::Values(ref values) => values.len(),
            Entry::Empty => 0,
        }
    }

    #[inline]
    fn push_size(&mut self, pool: &RRBPool<A>, side: Side, level: usize, value: usize) {
        if let Entry::Nodes(ref mut size, _) = self.children {
            size.push(&pool.size_pool, side, level, value)
        }
    }

    #[inline]
    fn pop_size(&mut self, pool: &RRBPool<A>, side: Side, level: usize, value: usize) {
        if let Entry::Nodes(ref mut size, _) = self.children {
            size.pop(&pool.size_pool, side, level, value)
        }
    }

    #[inline]
    fn update_size(&mut self, pool: &RRBPool<A>, index: usize, level: usize, value: isize) {
        if let Entry::Nodes(ref mut size, _) = self.children {
            size.update(&pool.size_pool, index, level, value)
        }
    }

    fn size_up_to(&self, level: usize, index: usize) -> usize {
        if let Entry::Nodes(ref size, _) = self.children {
            if index == 0 {
                0
            } else {
                match size {
                    Size::Table(ref size_table) => size_table[index - 1],
                    Size::Size(_) => index * NODE_SIZE.pow(level as u32),
                }
            }
        } else {
            index
        }
    }

    fn index_in(&self, level: usize, index: usize) -> Option<usize> {
        let mut target_idx = if let Some(child_size) = NODE_SIZE.checked_pow(level as u32) {
            index / child_size
        } else {
            0
        };
        if target_idx >= self.children.len() {
            return None;
        }
        if let Entry::Nodes(Size::Table(ref size_table), _) = self.children {
            while size_table[target_idx] <= index {
                target_idx += 1;
                if target_idx >= size_table.len() {
                    return None;
                }
            }
        }
        Some(target_idx)
    }

    pub(crate) fn index(&self, level: usize, index: usize) -> &A {
        if level == 0 {
            &self.children.unwrap_values()[index]
        } else {
            let target_idx = self.index_in(level, index).unwrap();
            self.children.unwrap_nodes()[target_idx]
                .index(level - 1, index - self.size_up_to(level, target_idx))
        }
    }

    pub(crate) fn index_mut(&mut self, pool: &RRBPool<A>, level: usize, index: usize) -> &mut A {
        if level == 0 {
            &mut self.children.unwrap_values_mut(pool)[index]
        } else {
            let target_idx = self.index_in(level, index).unwrap();
            let offset = index - self.size_up_to(level, target_idx);
            let child = &mut self.children.unwrap_nodes_mut(pool)[target_idx];
            child.index_mut(pool, level - 1, offset)
        }
    }

    pub(crate) fn lookup_chunk(
        &self,
        level: usize,
        base: usize,
        index: usize,
    ) -> (Range<usize>, *const Chunk<A>) {
        if level == 0 {
            (
                base..(base + self.children.len()),
                self.children.unwrap_values() as *const Chunk<A>,
            )
        } else {
            let target_idx = self.index_in(level, index).unwrap();
            let offset = self.size_up_to(level, target_idx);
            let child_base = base + offset;
            let children = self.children.unwrap_nodes();
            let child = &children[target_idx];
            child.lookup_chunk(level - 1, child_base, index - offset)
        }
    }

    pub(crate) fn lookup_chunk_mut(
        &mut self,
        pool: &RRBPool<A>,
        level: usize,
        base: usize,
        index: usize,
    ) -> (Range<usize>, *mut Chunk<A>) {
        if level == 0 {
            (
                base..(base + self.children.len()),
                self.children.unwrap_values_mut(pool) as *mut Chunk<A>,
            )
        } else {
            let target_idx = self.index_in(level, index).unwrap();
            let offset = self.size_up_to(level, target_idx);
            let child_base = base + offset;
            let children = self.children.unwrap_nodes_mut(pool);
            let child = &mut children[target_idx];
            child.lookup_chunk_mut(pool, level - 1, child_base, index - offset)
        }
    }

    fn push_child_node(&mut self, pool: &RRBPool<A>, side: Side, child: Node<A>) {
        let children = self.children.unwrap_nodes_mut(pool);
        match side {
            Left => children.push_front(child),
            Right => children.push_back(child),
        }
    }

    fn pop_child_node(&mut self, pool: &RRBPool<A>, side: Side) -> Node<A> {
        let children = self.children.unwrap_nodes_mut(pool);
        match side {
            Left => children.pop_front(),
            Right => children.pop_back(),
        }
    }

    pub(crate) fn push_chunk(
        &mut self,
        pool: &RRBPool<A>,
        level: usize,
        side: Side,
        mut chunk: PoolRef<Chunk<A>>,
    ) -> PushResult<PoolRef<Chunk<A>>> {
        if chunk.is_empty() {
            return PushResult::Done;
        }
        let is_full = self.is_full();
        if level == 0 {
            if self.children.is_empty_node() {
                self.push_size(pool, side, level, chunk.len());
                self.children = Values(chunk);
                PushResult::Done
            } else {
                let values = self.children.unwrap_values_mut(pool);
                if values.len() + chunk.len() <= NODE_SIZE {
                    let chunk = PoolRef::make_mut(&pool.value_pool, &mut chunk);
                    match side {
                        Side::Left => {
                            chunk.append(values);
                            values.append(chunk);
                        }
                        Side::Right => values.append(chunk),
                    }
                    PushResult::Done
                } else {
                    PushResult::Full(chunk, 0)
                }
            }
        } else if level == 1 {
            // If rightmost existing node has any room, merge as much as
            // possible over from the new node.
            let num_drained = match side {
                Side::Right => {
                    if let Entry::Nodes(ref mut size, ref mut children) = self.children {
                        let rightmost = PoolRef::make_mut(&pool.node_pool, children)
                            .last_mut()
                            .unwrap();
                        let old_size = rightmost.len();
                        let chunk = PoolRef::make_mut(&pool.value_pool, &mut chunk);
                        let values = rightmost.children.unwrap_values_mut(pool);
                        let to_drain = chunk.len().min(NODE_SIZE - values.len());
                        values.drain_from_front(chunk, to_drain);
                        size.pop(&pool.size_pool, Side::Right, level, old_size);
                        size.push(&pool.size_pool, Side::Right, level, values.len());
                        to_drain
                    } else {
                        0
                    }
                }
                Side::Left => {
                    if let Entry::Nodes(ref mut size, ref mut children) = self.children {
                        let leftmost = PoolRef::make_mut(&pool.node_pool, children)
                            .first_mut()
                            .unwrap();
                        let old_size = leftmost.len();
                        let chunk = PoolRef::make_mut(&pool.value_pool, &mut chunk);
                        let values = leftmost.children.unwrap_values_mut(pool);
                        let to_drain = chunk.len().min(NODE_SIZE - values.len());
                        values.drain_from_back(chunk, to_drain);
                        size.pop(&pool.size_pool, Side::Left, level, old_size);
                        size.push(&pool.size_pool, Side::Left, level, values.len());
                        to_drain
                    } else {
                        0
                    }
                }
            };
            if is_full {
                PushResult::Full(chunk, num_drained)
            } else {
                // If the chunk is empty after being drained, there might be
                // more space in existing chunks. To keep the middle dense, we
                // do not add it here.
                if !chunk.is_empty() {
                    if side == Left && chunk.len() < NODE_SIZE {
                        if let Entry::Nodes(ref mut size, _) = self.children {
                            if let Size::Size(value) = *size {
                                *size = Size::table_from_size(&pool.size_pool, level, value);
                            }
                        }
                    }
                    self.push_size(pool, side, level, chunk.len());
                    self.push_child_node(pool, side, Node::from_chunk(pool, 0, chunk));
                }
                PushResult::Done
            }
        } else {
            let chunk_size = chunk.len();
            let index = match side {
                Right => self.children.len() - 1,
                Left => 0,
            };
            let new_child = {
                let children = self.children.unwrap_nodes_mut(pool);
                let child = &mut children[index];
                match child.push_chunk(pool, level - 1, side, chunk) {
                    PushResult::Done => None,
                    PushResult::Full(chunk, num_drained) => {
                        // Our chunk was too large for `child`, so it could not
                        // be pushed there. However, exactly `num_drained`
                        // elements were added to the child. We need to reflect
                        // that change in the size field of the node.
                        match side {
                            Right => match self.children {
                                Entry::Nodes(Size::Table(ref mut sizes), _) => {
                                    let sizes = PoolRef::make_mut(&pool.size_pool, sizes);
                                    sizes[index] += num_drained;
                                }
                                Entry::Nodes(Size::Size(ref mut size), _) => {
                                    *size += num_drained;
                                }
                                Entry::Values(_) | Entry::Empty => (),
                            },
                            Left => {
                                self.update_size(pool, 0, level, num_drained as isize);
                            }
                        }
                        if is_full {
                            return PushResult::Full(chunk, 0);
                        } else {
                            Some(Node::from_chunk(pool, level - 1, chunk))
                        }
                    }
                }
            };
            match new_child {
                None => {
                    self.update_size(pool, index, level, chunk_size as isize);
                    PushResult::Done
                }
                Some(child) => {
                    if side == Left && chunk_size < NODE_SIZE {
                        if let Entry::Nodes(ref mut size, _) = self.children {
                            if let Size::Size(value) = *size {
                                *size = Size::table_from_size(&pool.size_pool, level, value);
                            }
                        }
                    }
                    self.push_size(pool, side, level, child.len());
                    self.push_child_node(pool, side, child);
                    PushResult::Done
                }
            }
        }
    }

    pub(crate) fn pop_chunk(
        &mut self,
        pool: &RRBPool<A>,
        level: usize,
        side: Side,
    ) -> PopResult<PoolRef<Chunk<A>>> {
        if self.is_empty() {
            return PopResult::Empty;
        }
        if level == 0 {
            // should only get here if the tree is just one leaf node
            match replace(&mut self.children, Empty) {
                Values(chunk) => PopResult::Drained(chunk),
                Empty => panic!("rrb::Node::pop_chunk: non-empty tree with Empty leaf"),
                Nodes(_, _) => panic!("rrb::Node::pop_chunk: branch node at leaf"),
            }
        } else if level == 1 {
            let child_node = self.pop_child_node(pool, side);
            self.pop_size(pool, side, level, child_node.len());
            let chunk = match child_node.children {
                Values(ref chunk) => chunk.clone(),
                Empty => panic!("rrb::Node::pop_chunk: non-empty tree with Empty leaf"),
                Nodes(_, _) => panic!("rrb::Node::pop_chunk: branch node at leaf"),
            };
            if self.is_empty() {
                PopResult::Drained(chunk)
            } else {
                PopResult::Done(chunk)
            }
        } else {
            let index = match side {
                Right => self.children.len() - 1,
                Left => 0,
            };
            let mut drained = false;
            let chunk = {
                let children = self.children.unwrap_nodes_mut(pool);
                let child = &mut children[index];
                match child.pop_chunk(pool, level - 1, side) {
                    PopResult::Empty => return PopResult::Empty,
                    PopResult::Done(chunk) => chunk,
                    PopResult::Drained(chunk) => {
                        drained = true;
                        chunk
                    }
                }
            };
            if drained {
                self.pop_size(pool, side, level, chunk.len());
                self.pop_child_node(pool, side);
                if self.is_empty() {
                    PopResult::Drained(chunk)
                } else {
                    PopResult::Done(chunk)
                }
            } else {
                self.update_size(pool, index, level, -(chunk.len() as isize));
                PopResult::Done(chunk)
            }
        }
    }

    pub(crate) fn split(
        &mut self,
        pool: &RRBPool<A>,
        level: usize,
        drop_side: Side,
        index: usize,
    ) -> SplitResult {
        if index == 0 && drop_side == Side::Left {
            // Dropped nothing
            return SplitResult::Dropped(0);
        }
        if level > 0 && index == 0 && drop_side == Side::Right {
            // Dropped everything
            let dropped = if let Entry::Nodes(ref size, _) = self.children {
                size.size()
            } else {
                panic!("leaf node at non-leaf level!");
            };
            self.children = Entry::Empty;
            return SplitResult::Dropped(dropped);
        }
        let mut dropped;
        if level == 0 {
            let len = self.children.len();
            if index >= len {
                return SplitResult::OutOfBounds;
            }
            let children = self.children.unwrap_values_mut(pool);
            match drop_side {
                Side::Left => children.drop_left(index),
                Side::Right => children.drop_right(index),
            }
            SplitResult::Dropped(match drop_side {
                Left => index,
                Right => len - index,
            })
        } else if let Some(target_idx) = self.index_in(level, index) {
            let size_up_to = self.size_up_to(level, target_idx);
            let (size, children) =
                if let Entry::Nodes(ref mut size, ref mut children) = self.children {
                    (size, PoolRef::make_mut(&pool.node_pool, children))
                } else {
                    unreachable!()
                };
            let child_gone = 0 == {
                let child_node = &mut children[target_idx];
                match child_node.split(pool, level - 1, drop_side, index - size_up_to) {
                    SplitResult::OutOfBounds => return SplitResult::OutOfBounds,
                    SplitResult::Dropped(amount) => dropped = amount,
                }
                child_node.len()
            };
            match drop_side {
                Left => {
                    let mut drop_from = target_idx;
                    if child_gone {
                        drop_from += 1;
                    }
                    children.drop_left(drop_from);
                    if let Size::Size(value) = *size {
                        *size = Size::table_from_size(&pool.size_pool, level, value);
                    }
                    let size_table = if let Size::Table(ref mut size_ref) = size {
                        PoolRef::make_mut(&pool.size_pool, size_ref)
                    } else {
                        unreachable!()
                    };
                    let dropped_size = if target_idx > 0 {
                        size_table[target_idx - 1]
                    } else {
                        0
                    };
                    dropped += dropped_size;
                    size_table.drop_left(drop_from);
                    for i in size_table.iter_mut() {
                        *i -= dropped;
                    }
                }
                Right => {
                    let at_last = target_idx == children.len() - 1;
                    let mut drop_from = target_idx + 1;
                    if child_gone {
                        drop_from -= 1;
                    }
                    if drop_from < children.len() {
                        children.drop_right(drop_from);
                    }
                    match size {
                        Size::Size(ref mut size) if at_last => {
                            *size -= dropped;
                        }
                        Size::Size(ref mut size) => {
                            let size_per_child = NODE_SIZE.pow(level as u32);
                            let remainder = (target_idx + 1) * size_per_child;
                            let new_size = remainder - dropped;
                            if new_size < *size {
                                dropped = *size - new_size;
                                *size = new_size;
                            } else {
                                unreachable!(
                                    "this means node is empty, should be caught at start of method"
                                );
                            }
                        }
                        Size::Table(ref mut size_ref) => {
                            let size_table = PoolRef::make_mut(&pool.size_pool, size_ref);
                            let dropped_size =
                                size_table[size_table.len() - 1] - size_table[target_idx];
                            if drop_from < size_table.len() {
                                size_table.drop_right(drop_from);
                            }
                            if !child_gone {
                                size_table[target_idx] -= dropped;
                            }
                            dropped += dropped_size;
                        }
                    }
                }
            }
            SplitResult::Dropped(dropped)
        } else {
            SplitResult::OutOfBounds
        }
    }

    fn merge_leaves(pool: &RRBPool<A>, mut left: Self, mut right: Self) -> Self {
        if left.children.is_empty_node() {
            // Left is empty, just use right
            Self::single_parent(pool, right)
        } else if right.children.is_empty_node() {
            // Right is empty, just use left
            Self::single_parent(pool, left)
        } else {
            {
                let left_vals = left.children.unwrap_values_mut(pool);
                let left_len = left_vals.len();
                let right_vals = right.children.unwrap_values_mut(pool);
                let right_len = right_vals.len();
                if left_len + right_len <= NODE_SIZE {
                    left_vals.append(right_vals);
                } else {
                    let count = right_len.min(NODE_SIZE - left_len);
                    left_vals.drain_from_front(right_vals, count);
                }
            }
            if right.is_empty() {
                Self::single_parent(pool, left)
            } else {
                Self::join_dense(pool, left, right)
            }
        }
    }

    fn merge_rebalance(
        pool: &RRBPool<A>,
        level: usize,
        left: Self,
        middle: Self,
        right: Self,
    ) -> Self {
        let left_nodes = left.children.nodes().into_iter();
        let middle_nodes = middle.children.nodes().into_iter();
        let right_nodes = right.children.nodes().into_iter();
        let mut subtree_still_balanced = true;
        let mut next_leaf = Chunk::new();
        let mut next_node = Chunk::new();
        let mut next_subtree = Chunk::new();
        let mut root = Chunk::new();

        for subtree in left_nodes.chain(middle_nodes).chain(right_nodes) {
            if subtree.is_empty() {
                continue;
            }
            if subtree.is_completely_dense(level) && subtree_still_balanced {
                root.push_back(subtree);
                continue;
            }
            subtree_still_balanced = false;

            if level == 1 {
                for value in subtree.children.values() {
                    next_leaf.push_back(value);
                    if next_leaf.is_full() {
                        let new_node =
                            Node::from_chunk(pool, 0, PoolRef::new(&pool.value_pool, next_leaf));
                        next_subtree.push_back(new_node);
                        next_leaf = Chunk::new();
                        if next_subtree.is_full() {
                            let new_subtree = Node::parent(pool, level, next_subtree);
                            root.push_back(new_subtree);
                            next_subtree = Chunk::new();
                        }
                    }
                }
            } else {
                for node in subtree.children.nodes() {
                    next_node.push_back(node);
                    if next_node.is_full() {
                        let new_node = Node::parent(pool, level - 1, next_node);
                        next_subtree.push_back(new_node);
                        next_node = Chunk::new();
                        if next_subtree.is_full() {
                            let new_subtree = Node::parent(pool, level, next_subtree);
                            root.push_back(new_subtree);
                            next_subtree = Chunk::new();
                        }
                    }
                }
            }
        }
        if !next_leaf.is_empty() {
            let new_node = Node::from_chunk(pool, 0, PoolRef::new(&pool.value_pool, next_leaf));
            next_subtree.push_back(new_node);
        }
        if !next_node.is_empty() {
            let new_node = Node::parent(pool, level - 1, next_node);
            next_subtree.push_back(new_node);
        }
        if !next_subtree.is_empty() {
            let new_subtree = Node::parent(pool, level, next_subtree);
            root.push_back(new_subtree);
        }
        Node::parent(pool, level + 1, root)
    }

    pub(crate) fn merge(pool: &RRBPool<A>, mut left: Self, mut right: Self, level: usize) -> Self {
        if level == 0 {
            Self::merge_leaves(pool, left, right)
        } else {
            let merged = {
                if level == 1 {
                    // We're going to rebalance all the leaves anyway, there's
                    // no need for a middle at level 1
                    Node::parent(pool, 0, Chunk::new())
                } else {
                    let left_last =
                        if let Entry::Nodes(ref mut size, ref mut children) = left.children {
                            let node = PoolRef::make_mut(&pool.node_pool, children).pop_back();
                            if !node.is_empty() {
                                size.pop(&pool.size_pool, Side::Right, level, node.len());
                            }
                            node
                        } else {
                            panic!("expected nodes, found entries or empty");
                        };
                    let right_first =
                        if let Entry::Nodes(ref mut size, ref mut children) = right.children {
                            let node = PoolRef::make_mut(&pool.node_pool, children).pop_front();
                            if !node.is_empty() {
                                size.pop(&pool.size_pool, Side::Left, level, node.len());
                            }
                            node
                        } else {
                            panic!("expected nodes, found entries or empty");
                        };
                    Self::merge(pool, left_last, right_first, level - 1)
                }
            };
            Self::merge_rebalance(pool, level, left, merged, right)
        }
    }

    #[cfg(any(test, feature = "debug"))]
    pub(crate) fn assert_invariants(&self, level: usize) -> usize {
        // Verifies that the size table matches reality.
        match self.children {
            Entry::Empty => 0,
            Entry::Values(ref values) => {
                // An empty value node is pointless and should never occur.
                assert_ne!(0, values.len());
                // Value nodes should only occur at level 0.
                assert_eq!(0, level);
                values.len()
            }
            Entry::Nodes(ref size, ref children) => {
                // A parent node with no children should never occur.
                assert_ne!(0, children.len());
                // Parent nodes should never occur at level 0.
                assert_ne!(0, level);
                let mut lengths = Vec::new();
                let should_be_dense = matches!(size, Size::Size(_));
                for (index, child) in children.iter().enumerate() {
                    let len = child.assert_invariants(level - 1);
                    if should_be_dense && index < children.len() - 1 {
                        // Assert that non-end nodes without size tables are full.
                        assert_eq!(len, NODE_SIZE.pow(level as u32));
                    }
                    lengths.push(len);
                }
                match size {
                    Size::Size(size) => {
                        let total: usize = lengths.iter().sum();
                        assert_eq!(*size, total);
                    }
                    Size::Table(ref table) => {
                        assert_eq!(table.iter().len(), children.len());
                        for (index, current) in table.iter().enumerate() {
                            let expected: usize = lengths.iter().take(index + 1).sum();
                            assert_eq!(expected, *current);
                        }
                    }
                }
                lengths.iter().sum()
            }
        }
    }

    // pub fn print<W>(&self, f: &mut W, indent: usize, level: usize) -> Result<(), fmt::Error>
    // where
    //     W: fmt::Write,
    //     A: fmt::Debug,
    // {
    //     print_indent(f, indent)?;
    //     if level == 0 {
    //         if self.children.is_empty_node() {
    //             writeln!(f, "Leaf: EMPTY")
    //         } else {
    //             writeln!(f, "Leaf: {:?}", self.children.unwrap_values())
    //         }
    //     } else {
    //         match &self.children {
    //             Entry::Nodes(size, children) => {
    //                 writeln!(f, "Node level {} size_table {:?}", level, size)?;
    //                 for child in children.iter() {
    //                     child.print(f, indent + 4, level - 1)?;
    //                 }
    //                 Ok(())
    //             }
    //             _ => unreachable!(),
    //         }
    //     }
    // }
}

// fn print_indent<W>(f: &mut W, indent: usize) -> Result<(), fmt::Error>
// where
//     W: fmt::Write,
// {
//     for _i in 0..indent {
//         write!(f, " ")?;
//     }
//     Ok(())
// }
