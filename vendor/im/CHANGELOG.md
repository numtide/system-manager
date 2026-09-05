# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/) and this project
adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [15.1.0] - 2022-04-29

### Added

-   `HashSet` now implements `From<Vector<A>>` and `From<&Vector<A>> where A: Clone`.

### Fixed

-   Fixed a long standing crash bug in `OrdMap`/`OrdSet`. (#154, #143, #152, #124)
-   The `union` method on maps/sets will now prefer to mutate the larger set (which leads to less
    work) rather than the first set. (#163)
-   Ensure `TreeFocus` only implements `Send`/`Sync` when the underlying type does. (#157, #158)
-   There was an issue where nodes in very large `OrdMap`s could overflow when removing an element
    and cause a panic, which has now been fixed. (#141)
-   Assorted doc cleanup. (#150, #173, #186, #194)

## [15.0.0] - 2020-05-15

### Changed

-   Map iterators now return `(&K, &V)` and `(&K, &mut V)` respectively, to be consistent with
    `std::collections`'s API. `DiffIter` for `OrdMap` has also changed in the same manner. (#121)

### Removed

-   The `pool` feature flag has been removed from the `im` version of the crate, as `refpool` no
    longer supports threadsafe pools.
-   `HashSet::iter_mut()` has been removed, because if you modify the hashed values in a hash set,
    you break the hash set.

### Added

-   The `pool` feature flag was missing from the `im-rc` version of the crate, which is the version
    where it's actually useful. It's been added now.
-   `DiffIter` now has a `Debug` implementation.
-   There is now a `Vector::is_inline()` method to determine whether a `Vector` is currently
    inlined. (#129)

### Fixed

-   A smarter implementation of the sorting algorithm for `Vector` has improved the performance of
    `Vector::sort` by approximately 2x. (#126)

## [14.3.0] - 2020-03-03

### Changed

-   `proptest` strategies have been moved to `im::proptest`. The previous locations of the
    strategies (`im::vector::proptest` etc) are still available, but have been deprecated.

### Added

-   `OrdSet` and `OrdMap` now have `get_prev` and `get_next` methods (with equivalent `get_prev_mut`
    and `get_next_mut` methods for `OrdMap`) which will return the closest key match to the
    requested key in the specified direction if the key isn't in the set. (#95)
-   The `retain` method, inexplicably missing from `HashMap` but not `HashSet`, has been added.
    (#120)
-   The `get_mut` method on `OrdMap` was, equally inexplicably, private. It has now been made
    public.

## [14.2.0] - 2020-01-17

### Added

-   Both map types now have the `get_key_value()` method, corresponding to the equivalent
    [additions](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#method.get_key_value)
    to the standard library.
-   The `ptr_eq` method has been added to all data types, allowing you to test whether two values
    refer to the same content in memory, by testing for pointer equality. (#117)
-   `HashMap` had lost its `Arbitrary` implementation for the `quickcheck` feature flag. It's now
    been restored. (#118)
-   Implementations for `Arbitrary` from the [`arbitrary`](https://crates.io/crates/arbitrary/)
    crate have been added behind the `arbitrary` feature flag.

### Fixed

-   Fixed a bug when reversing a consuming iterator over a `Vector` by replacing the consuming
    iterator with a much simpler and slightly more efficient version. (#116)

## [14.1.0] - 2019-12-16

### Added

-   If you enable the `pool` feature flag, im now supports constructing data types using
    [`refpool`](https://crates.io/crates/refpool) to speed up chunk allocation. The performance
    boost will vary between use cases and operating systems, but generally at least a 10% speedup
    can be expected when constructing a data type from an iterator, and the more complex an
    operation is, the more likely it is to benefit from being able to quickly reallocate chunks.
    Note that in order to use this feature, you have to construct your data types using the
    `with_pool(&pool)` constructor, it's not enough just to enable the feature flag.

## [14.0.0] - 2019-11-19

### Changed

-   As `sized-chunks` now requires a slightly more recent version of `rustc` to compile,
    specifically version 1.36.0, so does `im`. This is a breaking change, but will of course only
    affect your code if you're using an older `rustc`.

### Fixed

-   Fixed a quadratic time worst case scenario in the quicksort implementation for `Vector`. (#101)
-   Fixed an edge case bug when splitting and joining large `Vector`s. (#105, #107)

## [13.0.0] - 2019-05-18

The minimum supported Rust version is now 1.34.0.

### Changed

-   `im::iter::unfold` now gives you the owned state value rather than an immutable reference to it,
    which makes it a little more useful.

### Removed

-   The deprecated `singleton` constructors have been removed. Please use `unit` instead.
-   The deprecated methods `Vector::chunks` and `Vector::chunks_mut` have been removed in favour of
    `Vector::leaves` and `Vector::leaves_mut` respectively. (#50)
-   The deprecated reference to [`sized-chunks`](https://crates.io/crates/sized-chunks) has been
    removed. If you need it, please use the `sized-chunks` crate directly.
-   `im::iter::unfold_mut` has been removed, as there's no meaningful difference between it and
    rust-std 1.34.0's `std::iter::from_fn` with a captured state variable.

### Fixed

-   `Vector` now uses
    [`sized_chunks::InlineArray`](https://docs.rs/sized-chunks/0.3.0/sized_chunks/inline_array/struct.InlineArray.html)
    instead of an `Empty` enum case to avoid allocation at very small sizes, letting you store a
    handful of elements on the stack before needing to grow into a full chunk. This has a beneficial
    effect on performance as well, as there's no pointer into the heap to dereference, making it
    faster than `std::vec::Vec` in this configuration.
-   Some complexity timings have been added and corrected. (#87)
-   `OrdSet::is_subset(&self, other)` now returns immediately when `self` is larger than `other` and
    thus could not possibly be a subset of it. (#87)

## [12.3.4] - 2019-04-08

### Changed

-   `Clone` constraints have been further relaxed on maps and sets, so that you can now lookup and
    iterate over them without requiring a `Clone` constraint (though you do still need `Clone` to
    actually insert data into them to lookup or iterate over). (#81)

### Fixed

-   Enforces the latest bugfix release of sized-chunks. (#78)
-   Another edge case bugfix to `Vector`'s size table handling. (#79)

## [12.3.3] - 2019-03-11

### Fixed

-   A number of issues were fixed where `Vector`'s size table would get out of sync with the node
    structure if exercised too much and cause erroneous behaviour. (#72, #74)
-   Comprehensive generative tests were added to test all data structures through more unexpected
    code paths.

## [12.3.2] - 2019-03-05

### Changed

-   `Clone` constraints on all data structures, as well as relevant constraints on maps and sets,
    have been relaxed where possible, so that you can now construct empty instances and call most
    query methods without requiring values implement `Clone` etc. (#63)

### Fixed

-   Constructing an empty `Vector` will not allocate any heap memory, instead deferring allocation
    until you perform an operation that would increase its length. (#65)
-   Some bugs arising when using `Vector::append` repeatedly were fixed. (#67, #70)

## [12.3.1] - 2019-02-19

### Changed

-   Unsafe chunks have been separated out into the `sized-chunks` crate, which is now a dependency
    of `im`.

## [12.3.0] - 2019-01-15

### Added

-   `singleton` methods have been deprecated and renamed to `unit`.
-   `Vector::chunks` and `Vector::chunks_mut` have been deprecated and renamed to `leaves` and
    `leaves_mut` to avoid confusion with `Vec::chunks`. (#50)

### Fixed

-   Fixed an issue where the `HashMap` draining iterator might access uninitialised memory leading
    to undefined behaviour. (#60)
-   Fixed multiple issues in `Vector::split_off` and `Vector::append` that would cause lookup errors
    and unexpectedly unbalanced trees. (#55).

## [12.2.0] - 2018-10-12

### Added

-   `OrdMap` and `OrdSet` now have a `range()` method which makes an iterator over a bounded subset
    of the values. The improved iterator implementation is also considerably more efficient than the
    previous (about an order of magnitude faster for nontrivial data sets). `iter()` has been
    updated to take advantage of this, and is now just an alias for `range(..)`. (#27)
-   `FocusMut` now has an `unmut` method to turn it into an immutable `Focus`, releasing its
    exclusive hold on the underlying `Vector`.
-   `Focus` now implements `Clone`.

## [12.1.0] - 2018-09-25

### Added

-   Maps and sets now have the `clear` method just like `Vector`. (#46)

### Changed

-   Single chunk `Vector`s are no longer allocated directly on the stack, meaning that they're now
    comparable in performance to `std::vec::Vec` rather than slightly faster, but they also won't
    eat up your stack space quite as quickly, and they'll clone without copying and share structure
    with clones as you'd expect.

## [12.0.0] - 2018-08-30

Starting with this release, the `arc` flag is gone, in favour of publishing `im` as two separate
crates: `im` (using `Arc`) and `im-rc` (using `Rc`). They're identical (and built from the same
code), except that `im` is thread safe and `im-rc` is a little bit more performant.

This is a major release as a consequence, but there should be no breaking code changes other than
the new default choice of reference counter.

### Added

-   The `Chunk` datatype that's used to build `Vector` and `OrdMap` has been exposed and made
    generally usable. It's somewhere between a
    [`GenericArray`](https://crates.io/crates/generic-array) and a ring buffer, offers O(1)\* push
    in either direction, and is generally hyperoptimised for its purpose of serving as nodes for
    Bagwell tries, but it's also a powered up version of
    [`GenericArray`](https://crates.io/crates/generic-array) that might be useful to others, hence
    the public API.
-   `Vector` now has `Focus` and `FocusMut` APIs for caching index lookups, yielding huge
    performance gains when performing multiple adjacent index lookups. `Vector::iter` has been
    reimplemented using this API, and is now much simpler and about twice as fast as a result, and
    `Vector::iter_mut` now runs nearly an order of magnitude faster. Likewise, `Vector::sort` and
    `Vector::retain` are now using `FocusMut` and run considerably faster as a result.
-   `Focus` and `FocusMut` can also be used as stand ins for subslices through the `narrow` and
    `split_at` methods. You can also iterate over foci, making this the most efficient way to
    iterate over a subset of a `Vector`.
-   `Vector` now implements [Rayon](https://crates.io/crates/rayon)'s parallel iterators behind the
    `rayon` feature flag.

### Changed

-   As `std::ops::RangeBounds` is now stabilised in Rust 1.28, the `Vector::slice` method is now
    unconditionally available on the stable channel.
-   Union/difference/intersection/is_submap methods on `HashMap` and `OrdMap` that take functions
    now take `FnMut` instead of `Fn`. This should not affect any existing code. (#34)
-   `Vector::split_off` can now take an index equal to the length of the vector, yielding an empty
    vector as the split result. (#33)
-   `Vector::set` now returns the replaced value.

### Fixed

-   `Vector` is now represented as a single inline chunk until it grows larger than the chunk size,
    making it even faster than `Vec` at small sizes, though `clone` could now be slower if the clone
    is expensive (it's still absurdly fast for `A: Copy`).

## [11.0.1] - 2018-07-23

### Fixed

-   Various performance improvements, amounting to a 5-10% speedup for both kinds of map/set.
-   Fixed an edge case bug in `sort::quicksort`.

## [11.0.0] - 2018-07-10

### Changed

This is a major release with many breaking changes, and is intended to stabilise the API more than
to denote that the rewrite is now production ready. You should expect future releases with
significant performance improvements as well as additional APIs, but there should be no further
major release with breaking changes in the immediate future, barring very serious unforeseen issues.

Specifically, you should expect imminent minor releases with performance improvements for `Vector`
and `OrdMap`, for which I have a number of known optimisations that remain unimplemented.

#### No More `Arc`

All data structures have been reworked to take values of `A: Clone` instead of `Arc<A>`, meaning
that there's less performance overhead (as well as mental overhead) when using values that clone
cheaply. The performance gain when values are `A: Copy` is a factor of two or more. It's expected
that users should wrap values in `Arc` themselves when using values which are expensive to clone.

Data structures still use reference counters internally to reference nodes, but values are stored
directly in the nodes with no further indirection. This is also good for cache locality.

Data structures now use `Rc` instead of `Arc` by default to do reference counting. If you need a
thread safe version that implements `Send` and `Sync`, you can enable the `arc` feature on the
package to compile with `Arc` instead.

#### `std::collections` Compatible API

The API has been reworked to align more closely with `std::collections`, favouring mutable
operations by default, so that operations that were previously suffixed with `_mut` are now the
standard operations, and immutable operations which return a modified copy have been given different
names altogether. In short, all your code using previous versions of this library will no longer
work, and if it was relying heavily on immutable operations, it's recommended that you rewrite it to
be mutable by preference, but you should generally be able to make it work again by using the new
method names for the immutable operations.

Here is a list of the most notable changed method names for maps and sets:

| Previous immutable | Current immutable | Previous mutable | Current mutable |
| ------------------ | ----------------- | ---------------- | --------------- |
| `insert`           | `update`          | `insert_mut`     | `insert`        |
| `remove`           | `without`         | `remove_mut`     | `remove`        |
| `pop`              | `extract`         | `pop_mut`        | `remove`        |

You should expect to be able to rewrite code using `std::collections::HashMap` and
`std::collections::BTreeMap` with minimal or no changes using `im::HashMap` and `im::OrdMap`
respectively.

`Vector` has been completely rewritten and has an API that aligns closely with
`std::collections::VecDeque`, with very few immutable equivalents. It's expected that you should use
`Vector::clone()` to take a snapshot when you need it rather than cause an implicit clone for each
operation. (It's still O(1) and practically instant.)

I'm considering adding back some of the immutable operations if I can come up with good names for
them, but for now, just `clone` it if you need it.

#### RRB Vector

`Vector` is now implemented as an
[RRB tree](https://infoscience.epfl.ch/record/213452/files/rrbvector.pdf) with
[smart head/tail chunking](http://gallium.inria.fr/~rainey/chunked_seq.pdf), obsoleting the previous
[Hickey trie](https://hypirion.com/musings/understanding-persistent-vector-pt-1) implementation.

RRB trees have generally similar performance characteristics to the Hickey trie, with the added
benefit of having O(log n) splitting and concatenation.

| Operation       | RRB tree | Hickey trie | Vec    | VecDeque |
| --------------- | -------- | ----------- | ------ | -------- |
| Push front      | O(1)\*   | O(log n)    | O(n)   | O(1)\*   |
| Push back       | O(1)\*   | O(log n)    | O(1)\* | O(1)\*   |
| Pop front       | O(1)\*   | O(log n)    | O(n)   | O(1)\*   |
| Pop back        | O(1)\*   | O(log n)    | O(1)   | O(1)\*   |
| Lookup by index | O(log n) | O(log n)    | O(1)   | O(1)     |
| Split           | O(log n) | O(log n)    | O(n)   | O(n)     |
| Join            | O(log n) | O(n)        | O(n)   | O(n)     |

(Please note that the timings above are for the `im` version of the Hickey trie, based on the
[Immutable.js](https://facebook.github.io/immutable-js/) implementation, which performs better than
the original Clojure version on splits and push/pop front, but worse on push/pop back).

The RRB tree is the most generally efficient list like data structure currently known, to my
knowledge, but obviously it does not and cannot perform as well as a simple `Vec` on certain
operations. It makes up for that by having no operations you need to worry about the performance
complexity of: nothing you can do to an RRB tree is going to be more expensive than just iterating
over it. For larger data sets, being able to concatenate (and, by extension, insert and remove at
arbitrary locations) several orders of magnitude faster than `Vec` could also be considered a
selling point.

#### No More `CatList` And `ConsList`

`CatList` has been superseded by `Vector`, and `ConsList` was generally not very useful except in
the more peculiar edge cases where memory consumption matters more than performance, and keeping it
in line with current API changes wasn't practical.

#### No More Funny Words

Though it breaks my heart, words like `cons`, `snoc`, `car`, `cdr` and `uncons` are no longer used
in the `im` API, to facilitiate closer alignment with `std::collections`. Even the `head`/`tail`
pair is gone, though `head` and `last` remain as aliases for `front` and `back`.

## [10.2.0] - 2018-04-15

### Added

-   Map/set methods which accept references to keys will now also take any value that's borrowable
    to the key's type, ie. it will take a reference to a type `Borrowable` where the key implements
    `Borrow<Borrowable>`. This is particularly handy for types such as `String` because you can now
    pass `&str` to key lookups instead of `&String`. So, instead of the incredibly cumbersome
    `map.get(&"foo".to_string())` you can just do `map.get("foo")` when looking up a mapping for a
    string literal.

## [10.1.0] - 2018-04-12

### Added

-   `Vector`, `OrdMap` and `HashMap` now implement `Index` and `IndexMut`, allowing for syntax like
    `map[key] = value`.
-   Added `cons`, `snoc`, `uncons` and `unsnoc` aliases where they were missing.
-   Everything now implements `Sum` and `Extend` where possible.

### Changed

-   Generalised `OrdMap`/`OrdSet`'s internal nodes so `OrdSet` now only needs to store pointers to
    its values, not pairs of pointers to value and `Unit`. This has caused `OrdMap/Set`'s type
    constraints to tighten somewhat - in particular, iteration over maps/sets whose keys don't
    implement `Ord` is no longer possible, but as you would only have been able to create empty
    instances of these, no sensible code should break because of this.
-   `HashMap`/`HashSet` now also cannot be iterated over unless they implement `Hash + Eq`, with the
    same note as above.
-   Constraints on single operations that take closures on `HashMap` and `OrdMap` have been relaxed
    from `Fn` to `FnOnce`. (Fixes #7.)

### Fixed

-   Hashes are now stored in `HashMap`s along with their associated values, removing the need to
    recompute the hash when a value is reordered inside the tree.

## [10.0.0] - 2018-03-25

### Added

This is the first release to be considered reasonably stable. No changelog has been kept until now.
