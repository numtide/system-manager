//! Proptest strategies.
//!
//! These are only available when using the `proptest` feature flag.

use crate::{HashMap, HashSet, OrdMap, OrdSet, Vector};
use ::proptest::collection::vec;
use ::proptest::strategy::{BoxedStrategy, Strategy, ValueTree};
use std::hash::Hash;
use std::iter::FromIterator;
use std::ops::Range;

/// A strategy for generating a [`Vector`][Vector] of a certain size.
///
/// # Examples
///
/// ```rust,no_run
/// # use ::proptest::proptest;
/// proptest! {
///     #[test]
///     fn proptest_a_vector(ref l in vector(".*", 10..100)) {
///         assert!(l.len() < 100);
///         assert!(l.len() >= 10);
///     }
/// }
/// ```
///
/// [Vector]: ../struct.Vector.html
pub fn vector<A: Strategy + 'static>(
    element: A,
    size: Range<usize>,
) -> BoxedStrategy<Vector<<A::Tree as ValueTree>::Value>>
where
    <A::Tree as ValueTree>::Value: Clone,
{
    vec(element, size).prop_map(Vector::from_iter).boxed()
}

/// A strategy for an [`OrdMap`][OrdMap] of a given size.
///
/// # Examples
///
/// ```rust,no_run
/// # use ::proptest::proptest;
/// proptest! {
///     #[test]
///     fn proptest_works(ref m in ord_map(0..9999, ".*", 10..100)) {
///         assert!(m.len() < 100);
///         assert!(m.len() >= 10);
///     }
/// }
/// ```
///
/// [OrdMap]: ../struct.OrdMap.html
pub fn ord_map<K: Strategy + 'static, V: Strategy + 'static>(
    key: K,
    value: V,
    size: Range<usize>,
) -> BoxedStrategy<OrdMap<<K::Tree as ValueTree>::Value, <V::Tree as ValueTree>::Value>>
where
    <K::Tree as ValueTree>::Value: Ord + Clone,
    <V::Tree as ValueTree>::Value: Clone,
{
    ::proptest::collection::vec((key, value), size.clone())
        .prop_map(OrdMap::from)
        .prop_filter("OrdMap minimum size".to_owned(), move |m| {
            m.len() >= size.start
        })
        .boxed()
}

/// A strategy for an [`OrdSet`][OrdSet] of a given size.
///
/// # Examples
///
/// ```rust,no_run
/// # use ::proptest::proptest;
/// proptest! {
///     #[test]
///     fn proptest_a_set(ref s in ord_set(".*", 10..100)) {
///         assert!(s.len() < 100);
///         assert!(s.len() >= 10);
///     }
/// }
/// ```
///
/// [OrdSet]: ../struct.OrdSet.html
pub fn ord_set<A: Strategy + 'static>(
    element: A,
    size: Range<usize>,
) -> BoxedStrategy<OrdSet<<A::Tree as ValueTree>::Value>>
where
    <A::Tree as ValueTree>::Value: Ord + Clone,
{
    ::proptest::collection::vec(element, size.clone())
        .prop_map(OrdSet::from)
        .prop_filter("OrdSet minimum size".to_owned(), move |s| {
            s.len() >= size.start
        })
        .boxed()
}

/// A strategy for a [`HashMap`][HashMap] of a given size.
///
/// # Examples
///
/// ```rust,no_run
/// # use ::proptest::proptest;
/// proptest! {
///     #[test]
///     fn proptest_works(ref m in hash_map(0..9999, ".*", 10..100)) {
///         assert!(m.len() < 100);
///         assert!(m.len() >= 10);
///     }
/// }
/// ```
///
/// [HashMap]: ../struct.HashMap.html
pub fn hash_map<K: Strategy + 'static, V: Strategy + 'static>(
    key: K,
    value: V,
    size: Range<usize>,
) -> BoxedStrategy<HashMap<<K::Tree as ValueTree>::Value, <V::Tree as ValueTree>::Value>>
where
    <K::Tree as ValueTree>::Value: Hash + Eq + Clone,
    <V::Tree as ValueTree>::Value: Clone,
{
    ::proptest::collection::vec((key, value), size.clone())
        .prop_map(HashMap::from)
        .prop_filter("Map minimum size".to_owned(), move |m| {
            m.len() >= size.start
        })
        .boxed()
}

/// A strategy for a [`HashSet`][HashSet] of a given size.
///
/// # Examples
///
/// ```rust,no_run
/// # use ::proptest::proptest;
/// proptest! {
///     #[test]
///     fn proptest_a_set(ref s in hash_set(".*", 10..100)) {
///         assert!(s.len() < 100);
///         assert!(s.len() >= 10);
///     }
/// }
/// ```
///
/// [HashSet]: ../struct.HashSet.html
pub fn hash_set<A: Strategy + 'static>(
    element: A,
    size: Range<usize>,
) -> BoxedStrategy<HashSet<<A::Tree as ValueTree>::Value>>
where
    <A::Tree as ValueTree>::Value: Hash + Eq + Clone,
{
    ::proptest::collection::vec(element, size.clone())
        .prop_map(HashSet::from)
        .prop_filter("HashSet minimum size".to_owned(), move |s| {
            s.len() >= size.start
        })
        .boxed()
}
