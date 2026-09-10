// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::config::POOL_SIZE;
use crate::nodes::chunk::Chunk;
use crate::nodes::rrb::Node;
use crate::util::Pool;

/// A memory pool for `Vector`.
pub struct RRBPool<A> {
    pub(crate) node_pool: Pool<Chunk<Node<A>>>,
    pub(crate) value_pool: Pool<Chunk<A>>,
    pub(crate) size_pool: Pool<Chunk<usize>>,
}

impl<A> RRBPool<A> {
    /// Create a new memory pool with the given size.
    pub fn new(size: usize) -> Self {
        Self::with_sizes(size, size, size)
    }

    /// Create a new memory pool with the given sizes for each subpool.
    pub fn with_sizes(
        node_pool_size: usize,
        leaf_pool_size: usize,
        size_table_pool_size: usize,
    ) -> Self {
        Self {
            node_pool: Pool::new(node_pool_size),
            value_pool: Pool::new(leaf_pool_size),
            size_pool: Pool::new(size_table_pool_size),
        }
    }

    /// Fill the memory pool with preallocated chunks.
    pub fn fill(&self) {
        self.node_pool.fill();
        self.value_pool.fill();
        self.size_pool.fill();
    }

    /// Get the size of the node subpool.
    pub fn node_pool_size(&self) -> usize {
        self.node_pool.get_pool_size()
    }

    /// Get the size of the leaf node subpool.
    pub fn leaf_pool_size(&self) -> usize {
        self.value_pool.get_pool_size()
    }

    /// Get the size of the size table subpool.
    pub fn size_table_pool_size(&self) -> usize {
        self.size_pool.get_pool_size()
    }
}

impl<A> Default for RRBPool<A> {
    /// Construct a pool with a reasonable default pool size.
    fn default() -> Self {
        Self::new(POOL_SIZE)
    }
}

impl<A> Clone for RRBPool<A> {
    fn clone(&self) -> Self {
        Self {
            node_pool: self.node_pool.clone(),
            value_pool: self.value_pool.clone(),
            size_pool: self.size_pool.clone(),
        }
    }
}
