// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Iterators over immutable data.

/// Create an iterator of values using a function to update an owned state
/// value.
///
/// The function is called with the current state as its argument, and should
/// return an [`Option`][std::option::Option] of a tuple of the next value to
/// yield from the iterator and the updated state. If the function returns
/// [`None`][std::option::Option::None], the iterator ends.
///
/// # Examples
/// ```
/// # #[macro_use] extern crate im;
/// # use im::iter::unfold;
/// # use im::vector::Vector;
/// # use std::iter::FromIterator;
/// // Create an infinite stream of numbers, starting at 0.
/// let mut it = unfold(0, |i| Some((i, i + 1)));
///
/// // Make a list out of its first five elements.
/// let numbers = Vector::from_iter(it.take(5));
/// assert_eq!(numbers, vector![0, 1, 2, 3, 4]);
/// ```
///
/// [std::option::Option]: https://doc.rust-lang.org/std/option/enum.Option.html
/// [std::option::Option::None]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
pub fn unfold<F, S, A>(value: S, f: F) -> impl Iterator<Item = A>
where
    F: Fn(S) -> Option<(A, S)>,
{
    let mut value = Some(value);
    std::iter::from_fn(move || {
        f(value.take().unwrap()).map(|(next, state)| {
            value = Some(state);
            next
        })
    })
}
