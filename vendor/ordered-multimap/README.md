# ordered-multimap-rs

[![Crates.io](https://img.shields.io/crates/v/ordered-multimap.svg)](https://crates.io/crates/ordered-multimap)
[![Docs.rs](https://docs.rs/ordered-multimap/badge.svg)](https://docs.rs/ordered-multimap)
[![CI](https://github.com/sgodwincs/ordered-multimap-rs/workflows/CI/badge.svg)](https://github.com/sgodwincs/ordered-multimap-rs/actions)

Currently, this crate contains a single type `ListOrderedMultimap`. This is a multimap meaning that
multiple values can be associated with a given key, but it also maintains insertion order across all
keys and values.

[Documentation](https://docs.rs/ordered-multimap/)

## Performance

Basic benchmarks show that the performance of this crate is on par with that of the
[multimap](https://crates.io/crates/multimap) crate which does not maintain insertion order.

## Features

 - `std` (default) enables usage of the standard library. Disabling this features allows this crate to be used in `no_std` environments.
 - `serde` for (de)serialization.

## TODO

It is planned that a corresponding `SetOrderedMultimap` will also be included in this crate which
will provide the same insertion order guarantees, but the set of values associated to a given key
will be an actual set instead of a list.

## License

Licensed under MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT).

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you
shall be licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
