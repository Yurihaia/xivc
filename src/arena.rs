use std::marker::PhantomData;

// I love arenas :')

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct Arena<I, E>{
    vec: Vec<Node<E>>,
    empty: Option<usize>,
    _pd: PhantomData<fn() -> I>
}

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
struct Node<E> {
    gen: usize,
    value: Entry<E>
}

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
enum Entry<E> {
    Item(E),
    Empty(Option<usize>)
}

impl<I, E> Arena<I, E> {

    // Workaround because const fn's cannot have function pointers in them
    const NEW_INST: Self = Arena {
        vec: Vec::new(),
        empty: None,
        _pd: PhantomData
    };

    pub const fn new() -> Self {
        Self::NEW_INST
    }

    pub fn push(&mut self, value: E) -> I where I: From<RawId> {
        let i = self.vec.len();
        let gen = if let Some(n) = self.empty {
            let x = &mut self.vec[n];
            match std::mem::replace(&mut x.value, Entry::Item(value)) {
                Entry::Empty(u) => self.empty = u,
                _ => panic!("Inconsistent internal state: Empty head pointed to non-empty node")
            }
            x.gen
        } else {
            self.vec.push(Node {
                gen: 0,
                value: Entry::Item(value)
            });
            0
        };
        I::from(RawId {
            gen,
            i
        })
    }

    pub fn get(&self, id: I) -> Option<&E> where I: Into<RawId> {
        let rid: RawId = id.into();
        let v = &self.vec[rid.i];
        if rid.gen == v.gen {
            match v.value {
                Entry::Item(ref e) => Some(e),
                _ => None
            }
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, id: I) -> Option<&mut E> where I: Into<RawId> {
        let rid: RawId = id.into();
        let v = &mut self.vec[rid.i];
        if rid.gen == v.gen {
            match v.value {
                Entry::Item(ref mut e) => Some(e),
                _ => None
            }
        } else {
            None
        }
    }

    pub fn remove(&mut self, id: I) -> Option<E> where I: Into<RawId> {
        let rid: RawId = id.into();
        let v = &mut self.vec[rid.i];
        if rid.gen != v.gen {
            return None;
        }
        v.gen += 1;
        match std::mem::replace(&mut v.value, Entry::Empty(self.empty)) {
            Entry::Item(e) => {
                self.empty = Some(rid.i);
                Some(e)
            },
            _ => panic!("Inconsistent internal state: Removed empty node with existing generation")
        }
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut E> {
        self.vec.iter_mut().filter_map(|v| match &mut v.value {
            Entry::Item(e) => Some(e),
            _ => None
        })
    }
}

#[macro_export]
macro_rules! arena_id {
    ($($(#[$m:meta])? $v:vis struct $id:ident;)*) => {
        $(
            $(#[$m])?
            $v struct $id($crate::arena::RawId);

            impl From<$crate::arena::RawId> for $id {
                fn from(i: $crate::arena::RawId) -> Self {
                    Self(i)
                }
            }
            
            impl From<$id> for $crate::arena::RawId {
                fn from(i: $id) -> Self {
                    i.0
                }
            }
        )*
    };
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct RawId {
    i: usize,
    gen: usize,
}

#[cfg(test)]
impl RawId {
    pub const fn dummy_id(i: usize) -> RawId {
        RawId {
            i,
            gen: 0
        }
    }
}