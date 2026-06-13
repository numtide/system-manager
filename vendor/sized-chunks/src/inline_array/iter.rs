use core::iter::FusedIterator;

use crate::InlineArray;

/// A consuming iterator over the elements of an `InlineArray`.
pub struct Iter<A, T> {
    pub(crate) array: InlineArray<A, T>,
}

impl<A, T> Iterator for Iter<A, T> {
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        self.array.remove(0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.array.len(), Some(self.array.len()))
    }
}

impl<A, T> DoubleEndedIterator for Iter<A, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.array.pop()
    }
}

impl<A, T> ExactSizeIterator for Iter<A, T> {}

impl<A, T> FusedIterator for Iter<A, T> {}

/// A draining iterator over the elements of an `InlineArray`.
///
/// "Draining" means that as the iterator yields each element, it's removed from
/// the `InlineArray`. When the iterator terminates, the array will be empty.
/// This is different from the consuming iterator `Iter` in that `Iter` will
/// take ownership of the `InlineArray` and discard it when you're done
/// iterating, while `Drain` leaves you still owning the drained `InlineArray`.
pub struct Drain<'a, A, T> {
    pub(crate) array: &'a mut InlineArray<A, T>,
}

impl<'a, A, T> Iterator for Drain<'a, A, T> {
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        self.array.remove(0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.array.len(), Some(self.array.len()))
    }
}

impl<'a, A, T> DoubleEndedIterator for Drain<'a, A, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.array.pop()
    }
}

impl<'a, A, T> ExactSizeIterator for Drain<'a, A, T> {}

impl<'a, A, T> FusedIterator for Drain<'a, A, T> {}
