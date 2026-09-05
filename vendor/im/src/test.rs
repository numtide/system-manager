// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use metrohash::MetroHash64;
use std::hash::{BuildHasher, Hasher};
use std::marker::PhantomData;
use typenum::{Unsigned, U64};

pub(crate) fn is_sorted<A, I>(l: I) -> bool
where
    I: IntoIterator<Item = A>,
    A: Ord,
{
    let mut it = l.into_iter().peekable();
    loop {
        match (it.next(), it.peek()) {
            (_, None) => return true,
            (Some(ref a), Some(b)) if a > b => return false,
            _ => (),
        }
    }
}

pub(crate) struct LolHasher<N: Unsigned = U64> {
    state: u64,
    shift: usize,
    size: PhantomData<N>,
}

impl<N: Unsigned> LolHasher<N> {
    fn feed_me(&mut self, byte: u8) {
        self.state ^= u64::from(byte) << self.shift;
        self.shift += 8;
        if self.shift >= 64 {
            self.shift = 0;
        }
    }
}

impl<N: Unsigned> Hasher for LolHasher<N> {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.feed_me(*byte)
        }
    }

    fn finish(&self) -> u64 {
        if N::USIZE == 64 {
            self.state
        } else {
            self.state & ((1 << N::USIZE) - 1)
        }
    }
}

impl<N: Unsigned> Default for LolHasher<N> {
    fn default() -> Self {
        LolHasher {
            state: 0,
            shift: 0,
            size: PhantomData,
        }
    }
}

pub(crate) struct MetroHashBuilder {
    seed: u64,
}

impl MetroHashBuilder {
    pub(crate) fn new(seed: u64) -> Self {
        MetroHashBuilder { seed }
    }

    pub(crate) fn seed(&self) -> u64 {
        self.seed
    }
}

impl BuildHasher for MetroHashBuilder {
    type Hasher = MetroHash64;
    fn build_hasher(&self) -> Self::Hasher {
        MetroHash64::with_seed(self.seed)
    }
}
