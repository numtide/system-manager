# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/) and this project
adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [2.1.0] - 2020-03-26

### ADDED

-   There is now a `std` feature flag, on by default, which you can disable to get a `no_std` crate.

## [2.0.0] - 2019-09-09

### CHANGED

-   `Bits` now does a lot less work, which is now being done instead by the `BitOps` trait on its
    storage type. This turns out to improve compilation time quite considerably. If you were using
    methods on `Bits` directly, they will have moved to `BitOps`.
-   `Debug` now prints a single hex value for the entire bitmap, rather than deferring to the
    storage type.
-   `Iter` now takes a reference instead of a copy, which is more sensible for larger bitmaps.

### ADDED

-   `Bitmap` now implements `BitAnd`, `BitOr`, `BitXor`, their equivalent assignation traits, and
    `Not`, meaning you can now use bitwise operators on them, even the very big array-of-u128 ones.
-   A `Bitmap::mask()` constructor has been added, to construct bitmasks more efficiently, now that
    there are bitwise operators to use them with.

## [1.0.0] - 2019-09-06

Initial release.
