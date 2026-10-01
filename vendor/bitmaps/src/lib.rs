// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![forbid(rust_2018_idioms)]
#![deny(nonstandard_style)]
#![warn(unreachable_pub)]
#![allow(clippy::missing_safety_doc)]
#![cfg_attr(not(feature = "std"), no_std)]

//! This crate provides the [`Bitmap`][Bitmap] type as a convenient and
//! efficient way of declaring and working with fixed size bitmaps in Rust.
//!
//! # Examples
//!
//! ```rust
//! # #[macro_use] extern crate bitmaps;
//! # use bitmaps::Bitmap;
//! # use typenum::U10;
//! let mut bitmap: Bitmap<U10> = Bitmap::new();
//! assert_eq!(bitmap.set(5, true), false);
//! assert_eq!(bitmap.set(5, true), true);
//! assert_eq!(bitmap.get(5), true);
//! assert_eq!(bitmap.get(6), false);
//! assert_eq!(bitmap.len(), 1);
//! assert_eq!(bitmap.set(3, true), false);
//! assert_eq!(bitmap.len(), 2);
//! assert_eq!(bitmap.first_index(), Some(3));
//! ```
//!
//! # X86 Arch Support
//!
//! On `x86` and `x86_64` architectures, [`Bitmap`][Bitmap]s of size 256, 512,
//! 768 and 1024 gain the [`load_m256i()`][load_m256i] method, which reads the
//! bitmap into an [`__m256i`][m256i] or an array of [`__m256i`][m256i] using
//! [`_mm256_loadu_si256()`][loadu_si256].  [`Bitmap`][Bitmap]s of size 128 as
//! well as the previous gain the [`load_m128i()`][load_m128i] method, which
//! does the same for [`__m128i`][m128i].
//!
//! In addition, [`Bitmap<U128>`][Bitmap] and [`Bitmap<U256>`][Bitmap] will have
//! `From` and `Into` implementations for [`__m128i`][m128i] and
//! [`__m256i`][m256i] respectively.
//!
//! Note that alignment is unaffected - your bitmaps will be aligned
//! appropriately for `u128`, not [`__m128i`][m128i] or [`__m256i`][m256i],
//! unless you arrange for it to be otherwise. This may affect the performance
//! of SIMD instructions.
//!
//! [Bitmap]: struct.Bitmap.html
//! [load_m128i]: struct.Bitmap.html#method.load_m128i
//! [load_m256i]: struct.Bitmap.html#method.load_m256i
//! [m128i]: https://doc.rust-lang.org/core/arch/x86_64/struct.__m128i.html
//! [m256i]: https://doc.rust-lang.org/core/arch/x86_64/struct.__m256i.html
//! [loadu_si256]: https://doc.rust-lang.org/core/arch/x86_64/fn._mm256_loadu_si256.html

mod bitmap;
mod types;

#[doc(inline)]
pub use crate::bitmap::{Bitmap, Iter};
#[doc(inline)]
pub use crate::types::{BitOps, Bits};
