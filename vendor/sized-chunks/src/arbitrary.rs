// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use bitmaps::Bits;

use ::arbitrary::{size_hint, Arbitrary, Result, Unstructured};

use crate::{types::ChunkLength, Chunk, InlineArray, SparseChunk};

#[cfg(feature = "ringbuffer")]
use crate::RingBuffer;

impl<'a, A, N> Arbitrary<'a> for Chunk<A, N>
where
    A: Arbitrary<'a>,
    N: ChunkLength<A> + 'static,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        u.arbitrary_iter()?.take(Self::CAPACITY).collect()
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
        u.arbitrary_take_rest_iter()?.take(Self::CAPACITY).collect()
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        size_hint::recursion_guard(depth, |depth| {
            let (_, upper) = A::size_hint(depth);
            (0, upper.map(|upper| upper * Self::CAPACITY))
        })
    }
}

#[cfg(feature = "ringbuffer")]
impl<'a, A, N> Arbitrary<'a> for RingBuffer<A, N>
where
    A: Arbitrary<'a>,
    N: ChunkLength<A> + 'static,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        u.arbitrary_iter()?.take(Self::CAPACITY).collect()
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
        u.arbitrary_take_rest_iter()?.take(Self::CAPACITY).collect()
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        size_hint::recursion_guard(depth, |depth| {
            let (_, upper) = A::size_hint(depth);
            (0, upper.map(|upper| upper * Self::CAPACITY))
        })
    }
}

impl<'a, A, N> Arbitrary<'a> for SparseChunk<A, N>
where
    A: Clone,
    Option<A>: Arbitrary<'a>,
    N: ChunkLength<A> + Bits + 'static,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        u.arbitrary_iter()?.take(Self::CAPACITY).collect()
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
        u.arbitrary_take_rest_iter()?.take(Self::CAPACITY).collect()
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        size_hint::recursion_guard(depth, |depth| {
            let (_, upper) = Option::<A>::size_hint(depth);
            (0, upper.map(|upper| upper * Self::CAPACITY))
        })
    }
}

impl<'a, A, T> Arbitrary<'a> for InlineArray<A, T>
where
    A: Arbitrary<'a>,
    T: 'static,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        u.arbitrary_iter()?.take(Self::CAPACITY).collect()
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
        u.arbitrary_take_rest_iter()?.take(Self::CAPACITY).collect()
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        size_hint::recursion_guard(depth, |depth| {
            let (_, upper) = A::size_hint(depth);
            (0, upper.map(|upper| upper * Self::CAPACITY))
        })
    }
}
