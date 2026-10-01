# sized-chunks

Various fixed length array data types, designed for [immutable.rs].

## Overview

This crate provides the core building blocks for the immutable data structures
in [immutable.rs]: a sized array with O(1) amortised double ended push/pop and
smarter insert/remove performance (used by `im::Vector` and `im::OrdMap`), and a
fixed size sparse array (used by `im::HashMap`).

In a nutshell, this crate contains the unsafe bits from [immutable.rs], which
may or may not be useful to anyone else, and have been split out for ease of
auditing.

## Documentation

* [API docs](https://docs.rs/sized-chunks)

## Licence

Copyright 2019 Bodil Stokke

This software is subject to the terms of the Mozilla Public
License, v. 2.0. If a copy of the MPL was not distributed with this
file, You can obtain one at http://mozilla.org/MPL/2.0/.

## Code of Conduct

Please note that this project is released with a [Contributor Code of
Conduct][coc]. By participating in this project you agree to abide by its
terms.

[immutable.rs]: https://immutable.rs/
[coc]: https://github.com/bodil/sized-chunks/blob/master/CODE_OF_CONDUCT.md
