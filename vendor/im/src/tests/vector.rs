#![allow(clippy::unit_arg)]

use std::fmt::{Debug, Error, Formatter, Write};

use crate::Vector;

use proptest::proptest;
use proptest_derive::Arbitrary;

#[derive(Arbitrary, Debug)]
enum Action<A> {
    PushFront(A),
    PushBack(A),
    PopFront,
    PopBack,
    Insert(usize, A),
    Remove(usize),
    JoinLeft(Vec<A>),
    JoinRight(Vec<A>),
    SplitLeft(usize),
    SplitRight(usize),
}

#[derive(Arbitrary)]
struct Actions<A>(Vec<Action<A>>)
where
    A: Clone;

impl<A> Debug for Actions<A>
where
    A: Debug + Clone,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut out = String::new();
        let mut expected = vec![];
        writeln!(out, "let mut vec = Vector::new();")?;
        for action in &self.0 {
            match action {
                Action::PushFront(ref value) => {
                    expected.insert(0, value.clone());
                    writeln!(out, "vec.push_front({:?});", value)?
                }
                Action::PushBack(ref value) => {
                    expected.push(value.clone());
                    writeln!(out, "vec.push_back({:?});", value)?
                }
                Action::PopFront => {
                    if !expected.is_empty() {
                        expected.remove(0);
                    }
                    writeln!(out, "vec.pop_front();")?
                }
                Action::PopBack => {
                    expected.pop();
                    writeln!(out, "vec.pop_back();")?
                }
                Action::Insert(ref index, ref value) => {
                    let index = cap_index(expected.len(), *index);
                    expected.insert(index, value.clone());
                    writeln!(out, "vec.insert({:?}, {:?});", index, value)?
                }
                Action::Remove(ref index) => {
                    if !expected.is_empty() {
                        let index = cap_index(expected.len(), *index);
                        expected.remove(index);
                        writeln!(out, "vec.remove({:?})", index)?
                    } else {
                        continue;
                    }
                }
                Action::JoinLeft(ref vec) => {
                    let mut vec_new = vec.clone();
                    vec_new.append(&mut expected);
                    expected = vec_new;
                    writeln!(
                        out,
                        "let mut vec_new = Vector::from(vec!{:?}); // size {:?}",
                        vec,
                        vec.len()
                    )?;
                    writeln!(out, "vec_new.append(vec);")?;
                    writeln!(out, "vec = vec_new;")?
                }
                Action::JoinRight(ref vec) => {
                    expected.append(&mut vec.clone());
                    writeln!(
                        out,
                        "vec.append(Vector::from(vec!{:?})); // size {:?}",
                        vec,
                        vec.len()
                    )?
                }
                Action::SplitLeft(ref index) => {
                    let index = cap_index(expected.len(), *index);
                    expected.truncate(index);
                    writeln!(out, "vec.split_off({:?});", index)?
                }
                Action::SplitRight(ref index) => {
                    let index = cap_index(expected.len(), *index);
                    expected = expected.split_off(index);
                    writeln!(out, "vec = vec.split_off({:?});", index)?
                }
            }
            writeln!(out, "// len = {:?}", expected.len())?;
        }
        writeln!(out, "let expected = vec!{:?};", expected)?;
        writeln!(out, "assert_eq!(Vector::from(expected), vec);")?;
        write!(f, "{}", super::code_fmt(&out))
    }
}

fn cap_index(len: usize, index: usize) -> usize {
    if len == 0 {
        0
    } else {
        index % len
    }
}

proptest! {
    #[test]
    fn comprehensive(actions: Actions<u8>) {
        let mut vec = Vector::new();
        let mut nat = Vec::new();
        vec.assert_invariants();
        for action in actions.0 {
            match action {
                Action::PushFront(value) => {
                    let len = vec.len();
                    nat.insert(0, value);
                    vec.push_front(value);
                    assert_eq!(len + 1, vec.len());
                }
                Action::PushBack(value) => {
                    let len = vec.len();
                    nat.push(value);
                    vec.push_back(value);
                    assert_eq!(len + 1, vec.len());
                }
                Action::PopFront => {
                    if vec.is_empty() {
                        assert_eq!(None, vec.pop_front());
                    } else {
                        let len = vec.len();
                        assert_eq!(nat.remove(0), vec.pop_front().unwrap());
                        assert_eq!(len - 1, vec.len());
                    }
                }
                Action::PopBack => {
                    if vec.is_empty() {
                        assert_eq!(None, vec.pop_back());
                    } else {
                        let len = vec.len();
                        assert_eq!(nat.pop(), vec.pop_back());
                        assert_eq!(len - 1, vec.len());
                    }
                }
                Action::Insert(index, value) => {
                    let index = cap_index(vec.len(), index);
                    let len = vec.len();
                    nat.insert(index, value);
                    vec.insert(index, value);
                    assert_eq!(len + 1, vec.len());
                }
                Action::Remove(index) => {
                    if vec.is_empty() {
                        continue;
                    }
                    let index = cap_index(vec.len(), index);
                    let len = vec.len();
                    assert_eq!(nat.remove(index), vec.remove(index));
                    assert_eq!(len - 1, vec.len());
                }
                Action::JoinLeft(mut new_nat) => {
                    let mut new_vec = new_nat.iter().cloned().collect::<Vector<_>>();
                    let add_len = new_nat.len();
                    let len = vec.len();
                    new_vec.append(vec);
                    vec = new_vec;
                    new_nat.append(&mut nat);
                    nat = new_nat;
                    assert_eq!(len + add_len, vec.len());
                }
                Action::JoinRight(mut new_nat) => {
                    let new_vec = new_nat.iter().cloned().collect::<Vector<_>>();
                    let add_len = new_nat.len();
                    let len = vec.len();
                    vec.append(new_vec);
                    nat.append(&mut new_nat);
                    assert_eq!(len + add_len, vec.len());
                }
                Action::SplitLeft(index) => {
                    let index = cap_index(vec.len(), index);
                    let len = vec.len();
                    let vec_right = vec.split_off(index);
                    let nat_right = nat.split_off(index);
                    assert_eq!(index, vec.len());
                    assert_eq!(len - index, vec_right.len());
                    assert_eq!(nat_right.iter().cloned().collect::<Vector<_>>(), vec_right);
                }
                Action::SplitRight(index) => {
                    let index = cap_index(vec.len(), index);
                    let len = vec.len();
                    let vec_right = vec.split_off(index);
                    let nat_right = nat.split_off(index);
                    assert_eq!(index, vec.len());
                    assert_eq!(len - index, vec_right.len());
                    assert_eq!(nat.iter().cloned().collect::<Vector<_>>(), vec);
                    vec = vec_right;
                    nat = nat_right;
                }
            }
            vec.assert_invariants();
            assert_eq!(nat.len(),vec.len());
            assert_eq!(nat.iter().cloned().collect::<Vector<_>>(), vec);
        }
    }
}

#[test]
fn test_inserts() {
    const N: usize = 2000;
    let mut v = Vector::new();
    for i in 0..N {
        v.insert(v.len() / 2, i);
    }
    let mut rv: Vec<usize> = Vec::new();
    rv.extend((0..N).skip(1).step_by(2));
    rv.extend((0..N).step_by(2).rev());
    assert_eq!(rv.iter().cloned().collect::<Vector<_>>(), v);
}
