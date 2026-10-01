use core::{
  fmt::{self, Formatter},
  marker::PhantomData,
};

use serde::{
  de::{
    value::SeqDeserializer, Deserialize, Deserializer, Error, IntoDeserializer, SeqAccess, Visitor,
  },
  ser::{Serialize, SerializeSeq, Serializer},
};

use crate::VecList;

impl<T: Serialize> Serialize for VecList<T> {
  fn serialize<U: Serializer>(&self, serializer: U) -> Result<U::Ok, U::Error> {
    let mut seq = serializer.serialize_seq(Some(self.len()))?;

    for value in self.iter() {
      seq.serialize_element(value)?;
    }

    seq.end()
  }
}

struct VecListVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for VecListVisitor<T> {
  type Value = VecList<T>;

  fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    write!(formatter, "a sequence")
  }

  fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
  where
    A: SeqAccess<'de>,
  {
    let mut list = VecList::with_capacity(access.size_hint().unwrap_or_default());

    while let Some(value) = access.next_element()? {
      let _ = list.push_back(value);
    }

    Ok(list)
  }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for VecList<T> {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_seq(VecListVisitor(PhantomData))
  }
}

impl<'de, T, E> IntoDeserializer<'de, E> for VecList<T>
where
  T: IntoDeserializer<'de, E>,
  E: Error,
{
  type Deserializer = SeqDeserializer<<Self as IntoIterator>::IntoIter, E>;

  fn into_deserializer(self) -> Self::Deserializer {
    SeqDeserializer::new(self.into_iter())
  }
}

#[allow(unused_results)]
#[cfg(test)]
mod test {
  use coverage_helper::test;
  use serde_test::{assert_de_tokens_error, assert_tokens, Token};

  use super::*;

  #[test]
  fn test_de_error() {
    assert_de_tokens_error::<VecList<u32>>(
      &[Token::Map { len: Some(0) }],
      "invalid type: map, expected a sequence",
    );
  }

  #[test]
  fn test_ser_de_empty() {
    let list = VecList::<u32>::new();

    assert_tokens(&list, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
  }

  #[test]
  fn test_ser_de() {
    let mut list = VecList::new();
    list.push_back(0);
    list.push_back(1);
    list.push_back(2);
    list.push_back(3);
    list.push_back(4);

    assert_tokens(
      &list,
      &[
        Token::Seq { len: Some(5) },
        Token::I32(0),
        Token::I32(1),
        Token::I32(2),
        Token::I32(3),
        Token::I32(4),
        Token::SeqEnd,
      ],
    );
  }
}
