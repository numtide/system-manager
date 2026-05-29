use core::{
  fmt::{self, Formatter},
  hash::{BuildHasher, Hash},
  marker::PhantomData,
};

use serde::{
  de::{Deserialize, Deserializer, SeqAccess, Visitor},
  ser::{Serialize, SerializeSeq, Serializer},
};

use crate::ListOrderedMultimap;

impl<K, V, S> Serialize for ListOrderedMultimap<K, V, S>
where
  K: Clone + Eq + Hash + Serialize,
  V: Serialize,
  S: BuildHasher,
{
  fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
  where
    T: Serializer,
  {
    let mut seq = serializer.serialize_seq(Some(self.values_len()))?;

    for (key, value) in self.into_iter() {
      seq.serialize_element(&(key, value))?;
    }

    seq.end()
  }
}

struct ListOrderedMultimapVisitor<K, V, S>(PhantomData<(K, V, S)>);

impl<'de, K, V, S> Visitor<'de> for ListOrderedMultimapVisitor<K, V, S>
where
  K: Deserialize<'de> + Eq + Hash,
  V: Deserialize<'de>,
  S: BuildHasher + Default,
{
  type Value = ListOrderedMultimap<K, V, S>;

  fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    write!(formatter, "a sequence")
  }

  fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
  where
    A: SeqAccess<'de>,
  {
    let mut map = ListOrderedMultimap::with_capacity_and_hasher(
      access.size_hint().unwrap_or_default(),
      access.size_hint().unwrap_or_default(),
      S::default(),
    );

    while let Some((key, value)) = access.next_element()? {
      let _ = map.append(key, value);
    }

    Ok(map)
  }
}

impl<'de, K, V, S> Deserialize<'de> for ListOrderedMultimap<K, V, S>
where
  K: Deserialize<'de> + Eq + Hash,
  V: Deserialize<'de>,
  S: BuildHasher + Default,
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_seq(ListOrderedMultimapVisitor(PhantomData))
  }
}

#[allow(unused_results)]
#[cfg(all(test, feature = "std"))]
mod test {
  use coverage_helper::test;
  use serde_test::{assert_de_tokens_error, assert_tokens, Token};

  use super::*;

  #[test]
  fn test_de_error() {
    assert_de_tokens_error::<ListOrderedMultimap<char, u32>>(
      &[Token::Map { len: Some(0) }],
      "invalid type: map, expected a sequence",
    );
  }

  #[test]
  fn test_ser_de_empty() {
    let map = ListOrderedMultimap::<char, u32>::new();

    assert_tokens(&map, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
  }

  #[test]
  fn test_ser_de() {
    let mut map = ListOrderedMultimap::new();
    map.append('b', 20);
    map.append('a', 10);
    map.append('c', 30);
    map.append('b', 30);

    assert_tokens(
      &map,
      &[
        Token::Seq { len: Some(4) },
        Token::Tuple { len: 2 },
        Token::Char('b'),
        Token::I32(20),
        Token::TupleEnd,
        Token::Tuple { len: 2 },
        Token::Char('a'),
        Token::I32(10),
        Token::TupleEnd,
        Token::Tuple { len: 2 },
        Token::Char('c'),
        Token::I32(30),
        Token::TupleEnd,
        Token::Tuple { len: 2 },
        Token::Char('b'),
        Token::I32(30),
        Token::TupleEnd,
        Token::SeqEnd,
      ],
    );
  }
}
