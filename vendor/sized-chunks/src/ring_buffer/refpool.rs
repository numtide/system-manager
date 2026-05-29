use core::mem::MaybeUninit;

use ::refpool::{PoolClone, PoolDefault};

use crate::ring_buffer::index::RawIndex;
use crate::types::ChunkLength;
use crate::RingBuffer;

impl<A, N> PoolDefault for RingBuffer<A, N>
where
    N: ChunkLength<A>,
{
    unsafe fn default_uninit(target: &mut MaybeUninit<Self>) {
        let ptr = target.as_mut_ptr();
        let origin_ptr: *mut RawIndex<N> = &mut (*ptr).origin;
        let length_ptr: *mut usize = &mut (*ptr).length;
        origin_ptr.write(0.into());
        length_ptr.write(0);
    }
}

impl<A, N> PoolClone for RingBuffer<A, N>
where
    A: Clone,
    N: ChunkLength<A>,
{
    unsafe fn clone_uninit(&self, target: &mut MaybeUninit<Self>) {
        let ptr = target.as_mut_ptr();
        let origin_ptr: *mut RawIndex<N> = &mut (*ptr).origin;
        let length_ptr: *mut usize = &mut (*ptr).length;
        let data_ptr: *mut _ = &mut (*ptr).data;
        let data_ptr: *mut A = (*data_ptr).as_mut_ptr().cast();
        origin_ptr.write(self.origin);
        length_ptr.write(self.length);
        for index in self.range() {
            data_ptr
                .add(index.to_usize())
                .write((*self.ptr(index)).clone());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ::refpool::{Pool, PoolRef};
    use std::iter::FromIterator;

    #[test]
    fn default_and_clone() {
        let pool: Pool<RingBuffer<usize>> = Pool::new(16);
        let mut ref1 = PoolRef::default(&pool);
        {
            let chunk = PoolRef::make_mut(&pool, &mut ref1);
            chunk.push_back(1);
            chunk.push_back(2);
            chunk.push_back(3);
        }
        let ref2 = PoolRef::cloned(&pool, &ref1);
        let ref3 = PoolRef::clone_from(&pool, &RingBuffer::from_iter(1..=3));
        assert_eq!(RingBuffer::<usize>::from_iter(1..=3), *ref1);
        assert_eq!(RingBuffer::<usize>::from_iter(1..=3), *ref2);
        assert_eq!(RingBuffer::<usize>::from_iter(1..=3), *ref3);
        assert_eq!(ref1, ref2);
        assert_eq!(ref1, ref3);
        assert_eq!(ref2, ref3);
        assert!(!PoolRef::ptr_eq(&ref1, &ref2));
    }
}
