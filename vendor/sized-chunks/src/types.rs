// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Helper types for chunks.

use core::marker::PhantomData;

use typenum::*;

// Chunk sizes

/// A trait used to decide the size of an array.
///
/// `<N as ChunkLength<A>>::SizedType` for a type level integer N will have the
/// same size as `[A; N]`.
pub trait ChunkLength<A>: Unsigned {
    /// A `Sized` type matching the size of an array of `Self` elements of `A`.
    type SizedType;
}

impl<A> ChunkLength<A> for UTerm {
    type SizedType = ();
}

#[doc(hidden)]
#[allow(dead_code)]
pub struct SizeEven<A, B> {
    parent1: B,
    parent2: B,
    _marker: PhantomData<A>,
}

#[doc(hidden)]
#[allow(dead_code)]
pub struct SizeOdd<A, B> {
    parent1: B,
    parent2: B,
    data: A,
}

impl<A, N> ChunkLength<A> for UInt<N, B0>
where
    N: ChunkLength<A>,
{
    type SizedType = SizeEven<A, N::SizedType>;
}

impl<A, N> ChunkLength<A> for UInt<N, B1>
where
    N: ChunkLength<A>,
{
    type SizedType = SizeOdd<A, N::SizedType>;
}
