use bitmaps::{Bitmap, Bits, Iter as BitmapIter};

use super::SparseChunk;
use crate::types::ChunkLength;

/// An iterator over references to the elements of a `SparseChunk`.
pub struct Iter<'a, A, N: Bits + ChunkLength<A>> {
    pub(crate) indices: BitmapIter<'a, N>,
    pub(crate) chunk: &'a SparseChunk<A, N>,
}

impl<'a, A, N: Bits + ChunkLength<A>> Iterator for Iter<'a, A, N> {
    type Item = &'a A;

    fn next(&mut self) -> Option<Self::Item> {
        self.indices.next().map(|index| &self.chunk.values()[index])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(SparseChunk::<A, N>::CAPACITY))
    }
}

/// An iterator over mutable references to the elements of a `SparseChunk`.
pub struct IterMut<'a, A, N: Bits + ChunkLength<A>> {
    pub(crate) bitmap: Bitmap<N>,
    pub(crate) chunk: &'a mut SparseChunk<A, N>,
}

impl<'a, A, N: Bits + ChunkLength<A>> Iterator for IterMut<'a, A, N> {
    type Item = &'a mut A;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.bitmap.first_index() {
            self.bitmap.set(index, false);
            unsafe {
                let p: *mut A = &mut self.chunk.values_mut()[index];
                Some(&mut *p)
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(SparseChunk::<A, N>::CAPACITY))
    }
}

/// A draining iterator over the elements of a `SparseChunk`.
///
/// "Draining" means that as the iterator yields each element, it's removed from
/// the `SparseChunk`. When the iterator terminates, the chunk will be empty.
pub struct Drain<A, N: Bits + ChunkLength<A>> {
    pub(crate) chunk: SparseChunk<A, N>,
}

impl<'a, A, N: Bits + ChunkLength<A>> Iterator for Drain<A, N> {
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        self.chunk.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.chunk.len();
        (len, Some(len))
    }
}

/// An iterator over `Option`s of references to the elements of a `SparseChunk`.
///
/// Iterates over every index in the `SparseChunk`, from zero to its full capacity,
/// returning an `Option<&A>` for each index.
pub struct OptionIter<'a, A, N: Bits + ChunkLength<A>> {
    pub(crate) index: usize,
    pub(crate) chunk: &'a SparseChunk<A, N>,
}

impl<'a, A, N: Bits + ChunkLength<A>> Iterator for OptionIter<'a, A, N> {
    type Item = Option<&'a A>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < N::USIZE {
            let result = self.chunk.get(self.index);
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            SparseChunk::<A, N>::CAPACITY - self.index,
            Some(SparseChunk::<A, N>::CAPACITY - self.index),
        )
    }
}

/// An iterator over `Option`s of mutable references to the elements of a `SparseChunk`.
///
/// Iterates over every index in the `SparseChunk`, from zero to its full capacity,
/// returning an `Option<&mut A>` for each index.
pub struct OptionIterMut<'a, A, N: Bits + ChunkLength<A>> {
    pub(crate) index: usize,
    pub(crate) chunk: &'a mut SparseChunk<A, N>,
}

impl<'a, A, N: Bits + ChunkLength<A>> Iterator for OptionIterMut<'a, A, N> {
    type Item = Option<&'a mut A>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < N::USIZE {
            let result = if self.chunk.map.get(self.index) {
                unsafe {
                    let p: *mut A = &mut self.chunk.values_mut()[self.index];
                    Some(Some(&mut *p))
                }
            } else {
                Some(None)
            };
            self.index += 1;
            result
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            SparseChunk::<A, N>::CAPACITY - self.index,
            Some(SparseChunk::<A, N>::CAPACITY - self.index),
        )
    }
}

/// A draining iterator over `Option`s of the elements of a `SparseChunk`.
///
/// Iterates over every index in the `SparseChunk`, from zero to its full capacity,
/// returning an `Option<A>` for each index.
pub struct OptionDrain<A, N: Bits + ChunkLength<A>> {
    pub(crate) index: usize,
    pub(crate) chunk: SparseChunk<A, N>,
}

impl<'a, A, N: Bits + ChunkLength<A>> Iterator for OptionDrain<A, N> {
    type Item = Option<A>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < N::USIZE {
            let result = self.chunk.remove(self.index);
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            SparseChunk::<A, N>::CAPACITY - self.index,
            Some(SparseChunk::<A, N>::CAPACITY - self.index),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::iter::FromIterator;
    use typenum::U64;

    #[test]
    fn iter() {
        let vec: Vec<Option<usize>> =
            Vec::from_iter((0..64).map(|i| if i % 2 == 0 { Some(i) } else { None }));
        let chunk: SparseChunk<usize, U64> = vec.iter().cloned().collect();
        let vec: Vec<usize> = vec
            .iter()
            .cloned()
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect();
        assert!(vec.iter().eq(chunk.iter()));
    }

    #[test]
    fn iter_mut() {
        let vec: Vec<Option<usize>> =
            Vec::from_iter((0..64).map(|i| if i % 2 == 0 { Some(i) } else { None }));
        let mut chunk: SparseChunk<_, U64> = vec.iter().cloned().collect();
        let mut vec: Vec<usize> = vec
            .iter()
            .cloned()
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect();
        assert!(vec.iter_mut().eq(chunk.iter_mut()));
    }

    #[test]
    fn drain() {
        let vec: Vec<Option<usize>> =
            Vec::from_iter((0..64).map(|i| if i % 2 == 0 { Some(i) } else { None }));
        let chunk: SparseChunk<_, U64> = vec.iter().cloned().collect();
        let vec: Vec<usize> = vec
            .iter()
            .cloned()
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect();
        assert!(vec.into_iter().eq(chunk.into_iter()));
    }

    #[test]
    fn option_iter() {
        let vec: Vec<Option<usize>> =
            Vec::from_iter((0..64).map(|i| if i % 2 == 0 { Some(i) } else { None }));
        let chunk: SparseChunk<_, U64> = vec.iter().cloned().collect();
        assert!(vec
            .iter()
            .cloned()
            .eq(chunk.option_iter().map(|v| v.cloned())));
    }

    #[test]
    fn option_iter_mut() {
        let vec: Vec<Option<usize>> =
            Vec::from_iter((0..64).map(|i| if i % 2 == 0 { Some(i) } else { None }));
        let mut chunk: SparseChunk<_, U64> = vec.iter().cloned().collect();
        assert!(vec
            .iter()
            .cloned()
            .eq(chunk.option_iter_mut().map(|v| v.cloned())));
    }

    #[test]
    fn option_drain() {
        let vec: Vec<Option<usize>> =
            Vec::from_iter((0..64).map(|i| if i % 2 == 0 { Some(i) } else { None }));
        let chunk: SparseChunk<_, U64> = vec.iter().cloned().collect();
        assert!(vec.iter().cloned().eq(chunk.option_drain()));
    }
}
