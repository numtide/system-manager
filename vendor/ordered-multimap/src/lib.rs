//! This crate provides a type [`ListOrderedMultimap`] which is a multimap that maintains insertion order across all
//! keys and values.
//!
//! See the type documentation for more information.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod list_ordered_multimap;

pub use self::list_ordered_multimap::ListOrderedMultimap;

#[cfg(feature = "serde")]
mod serde;
