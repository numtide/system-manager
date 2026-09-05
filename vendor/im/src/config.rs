// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use typenum::*;

/// The branching factor of RRB-trees
pub(crate) type VectorChunkSize = U64;

/// The branching factor of B-trees
pub(crate) type OrdChunkSize = U64; // Must be an even number!

/// The level size of HAMTs, in bits
/// Branching factor is 2 ^ HashLevelSize.
pub(crate) type HashLevelSize = U5;

/// The size of per-instance memory pools if the `pool` feature is enabled.
/// This is set to 0, meaning you have to opt in to using a pool by constructing
/// with eg. `Vector::with_pool(pool)` even if the `pool` feature is enabled.
pub(crate) const POOL_SIZE: usize = 0;
