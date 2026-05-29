// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub(crate) mod btree;
pub(crate) mod hamt;
pub(crate) mod rrb;

pub(crate) mod chunk {
    use crate::config::VectorChunkSize;
    use sized_chunks as sc;
    use typenum::Unsigned;

    pub(crate) type Chunk<A> = sc::sized_chunk::Chunk<A, VectorChunkSize>;
    pub(crate) const CHUNK_SIZE: usize = VectorChunkSize::USIZE;
}
