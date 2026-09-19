// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::vector::FocusMut;
use rand_core::{RngCore, SeedableRng};
use std::cmp::Ordering;
use std::mem;

fn gen_range<R: RngCore>(rng: &mut R, min: usize, max: usize) -> usize {
    let range = max - min;
    min + (rng.next_u64() as usize % range)
}

// Ported from the Java version at:
//    http://www.cs.princeton.edu/~rs/talks/QuicksortIsOptimal.pdf
// There are a couple of modifications made here to make it more performant on the tree structure of
// the Vector. Instead of moving of handling equal and nonequal items in a single pass we make two
// additional passes to find the exact partition places. This allows us to split the focus into
// three correctly sized parts for less than, equal to and greater than items. As a bonus this
// doesn't need to reorder the equal items to the center of the vector.
fn do_quicksort<A, F, R>(vector: FocusMut<'_, A>, cmp: &F, rng: &mut R)
where
    A: Clone,
    F: Fn(&A, &A) -> Ordering,
    R: RngCore,
{
    if vector.len() <= 1 {
        return;
    }

    // We know there are at least 2 elements here
    let pivot_index = gen_range(rng, 0, vector.len());
    let (mut first, mut rest) = vector.split_at(1);

    if pivot_index > 0 {
        mem::swap(rest.index_mut(pivot_index - 1), first.index_mut(0));
    }
    // Pivot is now always in the first slice
    let pivot_item = first.index(0);

    // Find the exact place to put the pivot or pivot-equal items
    let mut less_count = 0;
    let mut equal_count = 0;

    for index in 0..rest.len() {
        let item = rest.index(index);
        let comp = cmp(item, pivot_item);
        match comp {
            Ordering::Less => less_count += 1,
            Ordering::Equal => equal_count += 1,
            Ordering::Greater => {}
        }
    }

    // If by accident we picked the minimum element as a pivot, we just call sort again with the
    // rest of the vector.
    if less_count == 0 {
        do_quicksort(rest, cmp, rng);
        return;
    }

    // We know here that there is at least one item before the pivot, so we move the minimum to the
    // beginning part of the vector. First, however we swap the pivot to the start of the equal
    // zone.
    less_count -= 1;
    equal_count += 1;
    let first_item = first.index_mut(0);
    mem::swap(first_item, rest.index_mut(less_count));
    for index in 0..rest.len() {
        if index == less_count {
            // This is the position we swapped the pivot to. We can't move it from its position, and
            // we know its not the minimum.
            continue;
        }
        let rest_item = rest.index_mut(index);
        if cmp(rest_item, first_item) == Ordering::Less {
            mem::swap(first_item, rest_item);
        }
    }

    // Split the vector up into less_than, equal to and greater than parts.
    let (remaining, mut greater_focus) = rest.split_at(less_count + equal_count);
    let (mut less_focus, mut equal_focus) = remaining.split_at(less_count);

    let mut less_position = 0;
    let mut equal_position = 0;
    let mut greater_position = 0;

    while less_position != less_focus.len() || greater_position != greater_focus.len() {
        // At start of this loop, equal_position always points to an equal item
        let mut equal_swap_side = None;
        let equal_item = equal_focus.index(equal_position);

        // Advance the less_position until we find an out of place item
        while less_position != less_focus.len() {
            let less_item = less_focus.index(less_position);
            match cmp(less_item, equal_item) {
                Ordering::Equal => {
                    equal_swap_side = Some(Ordering::Less);
                    break;
                }
                Ordering::Greater => {
                    break;
                }
                _ => {}
            }
            less_position += 1;
        }

        // Advance the greater until we find an out of place item
        while greater_position != greater_focus.len() {
            let greater_item = greater_focus.index(greater_position);
            match cmp(greater_item, equal_item) {
                Ordering::Less => break,
                Ordering::Equal => {
                    equal_swap_side = Some(Ordering::Greater);
                    break;
                }
                _ => {}
            }
            greater_position += 1;
        }

        if let Some(swap_side) = equal_swap_side {
            // One of the sides is equal to the pivot, advance the pivot
            let item = if swap_side == Ordering::Less {
                less_focus.index_mut(less_position)
            } else {
                greater_focus.index_mut(greater_position)
            };

            // We are guaranteed not to hit the end of the equal focus
            while cmp(item, equal_focus.index(equal_position)) == Ordering::Equal {
                equal_position += 1;
            }

            // Swap the equal position and the desired side, it's important to note that only the
            // equals focus is guaranteed to have made progress so we don't advance the side's index
            mem::swap(item, equal_focus.index_mut(equal_position));
        } else if less_position != less_focus.len() && greater_position != greater_focus.len() {
            // Both sides are out of place and not equal to the pivot, this can only happen if there
            // is a greater item in the lesser zone and a lesser item in the greater zone. The
            // solution is to swap both sides and advance both side's indices.
            debug_assert_ne!(
                cmp(
                    less_focus.index(less_position),
                    equal_focus.index(equal_position)
                ),
                Ordering::Equal
            );
            debug_assert_ne!(
                cmp(
                    greater_focus.index(greater_position),
                    equal_focus.index(equal_position)
                ),
                Ordering::Equal
            );
            mem::swap(
                less_focus.index_mut(less_position),
                greater_focus.index_mut(greater_position),
            );
            less_position += 1;
            greater_position += 1;
        }
    }

    // Now we have partitioned both sides correctly, we just have to recurse now
    do_quicksort(less_focus, cmp, rng);
    if !greater_focus.is_empty() {
        do_quicksort(greater_focus, cmp, rng);
    }
}

pub(crate) fn quicksort<A, F>(vector: FocusMut<'_, A>, cmp: &F)
where
    A: Clone,
    F: Fn(&A, &A) -> Ordering,
{
    let mut rng = rand_xoshiro::Xoshiro256Plus::seed_from_u64(0);
    do_quicksort(vector, cmp, &mut rng);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::is_sorted;
    use crate::vector::proptest::vector;
    use ::proptest::num::i32;
    use ::proptest::proptest;

    proptest! {
        #[test]
        fn test_quicksort(ref input in vector(i32::ANY, 0..10000)) {
            let mut vec = input.clone();
            let len = vec.len();
            if len > 1 {
                quicksort(vec.focus_mut(), &Ord::cmp);
            }
            assert!(is_sorted(vec));
        }
    }
}
