# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/) and this project
adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [0.6.5] - 2021-04-16

-   When `InlineArray` cannot hold any values because of misalignment, report it as capacity 0
    instead of panicking at runtime. (#22)

## [0.6.4] - 2021-02-17

### FIXED

-   `InlineArray` can be used in recursive types again.

### CHANGED

-   `InlineArray::new()` now panics when it can't store elements with large alignment (this was UB
    prior to 0.6.3). Alignments of `usize` and smaller are always supported. Larger alignments are
    supported if the capacity-providing type has sufficient alignment.

## [0.6.3] - 2021-02-14

### FIXED

-   Multilple soundness fixes: `InlineArray` handles large alignment, panic safety in `Chunk`'s
    `clone` and `from_iter`, capacity checks in `unit()`, `pair()` and `from()`.
-   `InlineArray` can now handle zero sized values. This relies on conditionals in const functions,
    a feature which was introduced in Rust 1.46.0, which means this is now the minimum Rust version
    this crate will work on.

## [0.6.2] - 2020-05-15

### FIXED

-   This release exists for no other purpose than to bump the `refpool` optional dependency.

## [0.6.1] - 2020-03-26

### ADDED

-   The crate now has a `std` feature flag, which is on by default, and will make the crate `no_std`
    if disabled.

### FIXED

-   Fixed a compilation error if you had the `arbitrary` feature flag enabled without the
    `ringbuffer` flag.

## [0.6.0] - 2020-03-24

### CHANGED

-   `RingBuffer` and its accompanying slice types `Slice` and `SliceMut` now implement `Array` and
    `ArrayMut` from [`array-ops`](http://docs.rs/array-ops), giving them most of the methods that
    would be available on primitive slice types and cutting down on code duplication in the
    implementation, but at the price of having to pull `Array` et al into scope when you need them.
    Because this means adding a dependency to `array-ops`, `RingBuffer` has now been moved behind
    the `ringbuffer` feature flag. `Chunk` and `InlineArray` don't and won't implement `Array`,
    because they are both able to implement `Deref<[A]>`, which provides the same functionality more
    efficiently.

### ADDED

-   The `insert_from` and `insert_ordered` methods recently added to `Chunk` have now also been
    added to `RingBuffer`.
-   `RingBuffer`'s `Slice` and `SliceMut` now also have the three `binary_search` methods regular
    slices have.
-   `SparseChunk`, `RingBuffer`, `Slice` and `SliceMut` now have unsafe `get_unchecked` and
    `get_unchecked_mut` methods.
-   `PartialEq` implementations allowing you to compare `RingBuffer`s, `Slice`s and `SliceMut`s
    interchangeably have been added.

### FIXED

-   Fixed an aliasing issue in `RingBuffer`'s mutable iterator, as uncovered by Miri. Behind the
    scenes, the full non-fuzzing unit test suite is now able to run on Miri without crashing it
    (after migrating the last Proptest tests away from the test suite into the fuzz targets), and
    this has been included in its CI build. (#6)

## [0.5.3] - 2020-03-11

### FIXED

-   Debug only assertions made it into the previous release by accident, and this change has been
    reverted. (#7)

## [0.5.2] - 2020-03-10

### ADDED

-   `Chunk` now has an `insert_from` method for inserting multiple values at an index in one go.
-   `Chunk` now also has an `insert_ordered` method for inserting values into a sorted chunk.
-   `SparseChunk` now has the methods `option_iter()`, `option_iter_mut()` and `option_drain()` with
    their corresponding iterators to iterate over a chunk as if it were an array of `Option`s.
-   [`Arbitrary`](https://docs.rs/arbitrary/latest/arbitrary/trait.Arbitrary.html) implementations
    for all data types have been added behind the `arbitrary` feature flag.

### FIXED

-   Internal consistency assertions are now only performed in debug mode (like with
    `debug_assert!`). This means `sized_chunks` will no longer cause panics in release mode when you
    do things like pushing to a full chunk, but do bad and undefined things instead. It also means a
    very slight performance gain.

## [0.5.1] - 2019-12-12

### ADDED

-   `PoolDefault` and `PoolClone` implementations, from the
    [`refpool`](https://crates.io/crates/refpool) crate, are available for `Chunk`, `SparseChunk`
    and `RingBuffer`, behind the `refpool` feature flag.

## [0.5.0] - 2019-09-09

### CHANGED

-   The `Bitmap` type (and its helper type, `Bits`) has been split off into a separate crate, named
    `bitmaps`. If you need it, it's in that crate now. `sized-chunks` does not re-export it. Of
    course, this means `sized-chunks` has gained `bitmaps` as its second hard dependency.

## [0.4.0] - 2019-09-02

### CHANGED

-   The 0.3.2 release increased the minimum rustc version required, which should have been a major
    version bump, so 0.3.2 is being yanked and re-tagged as 0.4.0.

## [0.3.2] - 2019-08-29

### ADDED

-   Chunk/bitmap sizes up to 1024 are now supported.

### FIXED

-   Replaced `ManuallyDrop` in implementations with `MaybeUninit`, along with a general unsafe code
    cleanup. (#3)

## [0.3.1] - 2019-08-03

### ADDED

-   Chunk sizes up to 256 are now supported.

## [0.3.0] - 2019-05-18

### ADDED

-   A new data structure, `InlineArray`, which is a stack allocated array matching the size of a
    given type, intended for optimising for the case of very small vectors.
-   `Chunk` has an implementation of `From<InlineArray>` which is considerably faster than going via
    iterators.

## [0.2.2] - 2019-05-10

### ADDED

-   `Slice::get` methods now return references with the lifetime of the underlying `RingBuffer`
    rather than the lifetime of the slice.

## [0.2.1] - 2019-04-15

### ADDED

-   A lot of documentation.
-   `std::io::Read` implementations for `Chunk<u8>` and `RingBuffer<u8>` to match their `Write`
    implementations.

## [0.2.0] - 2019-04-14

### CHANGED

-   The `capacity()` method has been replacied with a `CAPACITY` const on each type.

### ADDED

-   There is now a `RingBuffer` implementation, which should be nearly a drop-in replacement for
    `SizedChunk` but is always O(1) on push and cannot be dereferenced to slices (but it has a set
    of custom slice-like implementations to make that less of a drawback).
-   The `Drain` iterator for `SizedChunk` now implements `DoubleEndedIterator`.

### FIXED

-   `SizedChunk::drain_from_front/back` will now always panic if the iterator underflows, instead of
    only doing it in debug mode.

## [0.1.3] - 2019-04-12

### ADDED

-   `SparseChunk` now has a default length of `U64`.
-   `Chunk` now has `PartialEq` defined for anything that can be borrowed as a slice.
-   `SparseChunk<A>` likewise has `PartialEq` defined for `BTreeMap<usize, A>` and
    `HashMap<usize, A>`. These are intended for debugging and aren't optimally `efficient.
-   `Chunk` and `SparseChunk` now have a new method `capacity()` which returns its maximum capacity
    (the number in the type) as a usize.
-   Added an `entries()` method to `SparseChunk`.
-   `SparseChunk` now has a `Debug` implementation.

### FIXED

-   Extensive integration tests were added for `Chunk` and `SparseChunk`.
-   `Chunk::clear` is now very slightly faster.

## [0.1.2] - 2019-03-11

### FIXED

-   Fixed an alignment issue in `Chunk::drain_from_back`. (#1)

## [0.1.1] - 2019-02-19

### FIXED

-   Some 2018 edition issues.

## [0.1.0] - 2019-02-19

Initial release.
