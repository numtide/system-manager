use core::iter::FusedIterator;

use super::Chunk;
use crate::types::ChunkLength;

/// A consuming iterator over the elements of a `Chunk`.
pub struct Iter<A, N>
where
    N: ChunkLength<A>,
{
    pub(crate) chunk: Chunk<A, N>,
}

impl<A, N> Iterator for Iter<A, N>
where
    N: ChunkLength<A>,
{
    type Item = A;
    fn next(&mut self) -> Option<Self::Item> {
        if self.chunk.is_empty() {
            None
        } else {
            Some(self.chunk.pop_front())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.chunk.len(), Some(self.chunk.len()))
    }
}

impl<A, N> DoubleEndedIterator for Iter<A, N>
where
    N: ChunkLength<A>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.chunk.is_empty() {
            None
        } else {
            Some(self.chunk.pop_back())
        }
    }
}

impl<A, N> ExactSizeIterator for Iter<A, N> where N: ChunkLength<A> {}

impl<A, N> FusedIterator for Iter<A, N> where N: ChunkLength<A> {}

/// A draining iterator over the elements of a `Chunk`.
///
/// "Draining" means that as the iterator yields each element, it's removed from
/// the `Chunk`. When the iterator terminates, the chunk will be empty. This is
/// different from the consuming iterator `Iter` in that `Iter` will take
/// ownership of the `Chunk` and discard it when you're done iterating, while
/// `Drain` leaves you still owning the drained `Chunk`.
pub struct Drain<'a, A, N>
where
    N: ChunkLength<A>,
{
    pub(crate) chunk: &'a mut Chunk<A, N>,
}

impl<'a, A, N> Iterator for Drain<'a, A, N>
where
    A: 'a,
    N: ChunkLength<A> + 'a,
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.chunk.is_empty() {
            None
        } else {
            Some(self.chunk.pop_front())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.chunk.len(), Some(self.chunk.len()))
    }
}

impl<'a, A, N> DoubleEndedIterator for Drain<'a, A, N>
where
    A: 'a,
    N: ChunkLength<A> + 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.chunk.is_empty() {
            None
        } else {
            Some(self.chunk.pop_back())
        }
    }
}

impl<'a, A, N> ExactSizeIterator for Drain<'a, A, N>
where
    A: 'a,
    N: ChunkLength<A> + 'a,
{
}

impl<'a, A, N> FusedIterator for Drain<'a, A, N>
where
    A: 'a,
    N: ChunkLength<A> + 'a,
{
}
