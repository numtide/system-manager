# bitmaps

A fixed size compact boolean array in Rust.

## Overview

This crate provides a convenient and efficient way of declaring and working with
fixed size bitmaps in Rust. It was originally split out from the [sized-chunks]
crate and its primary purpose is to support it, but the `Bitmap` type has proven
to be generally useful enough that it was split off into a separate crate.

## Example

```rust
use bitmaps::Bitmap;
use typenum::U10;

fn main() {
    let mut bitmap = Bitmap::<U10>::new();
    assert_eq!(bitmap.set(5, true), false);
    assert_eq!(bitmap.set(5, true), true);
    assert_eq!(bitmap.get(5), true);
    assert_eq!(bitmap.get(6), false);
    assert_eq!(bitmap.len(), 1);
    assert_eq!(bitmap.set(3, true), false);
    assert_eq!(bitmap.len(), 2);
    assert_eq!(bitmap.first_index(), Some(3));
}
```

## Documentation

* [API docs](https://docs.rs/bitmaps)

## Licence

Copyright 2019 Bodil Stokke

This software is subject to the terms of the Mozilla Public
License, v. 2.0. If a copy of the MPL was not distributed with this
file, You can obtain one at http://mozilla.org/MPL/2.0/.

## Code of Conduct

Please note that this project is released with a [Contributor Code of
Conduct][coc]. By participating in this project you agree to abide by its
terms.

[sized-chunks]: https://github.com/bodil/sized-chunks
[coc]: https://github.com/bodil/bitmaps/blob/master/CODE_OF_CONDUCT.md
