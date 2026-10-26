//! IDs (u32 slot + u32 generation, written "o12g1") and the slot arena that hands
//! them out. Values sit behind `Arc` so a document clone shares everything and a
//! command copies only what it touches.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// A type stored in an [`Arena`]: its ID prefix and where its own ID lives.
pub trait Keyed: Clone {
    const PREFIX: char;
    fn id(&self) -> Id<Self>;
    fn set_id(&mut self, id: Id<Self>);
    fn name(&self) -> &str;
}

pub struct Id<T> {
    pub slot: u32,
    pub generation: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self {
            slot,
            generation,
            marker: PhantomData,
        }
    }
}

// Manual impls: derives would wrongly require `T: Clone` and friends.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        (self.slot, self.generation) == (other.slot, other.generation)
    }
}
impl<T> Eq for Id<T> {}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.slot, self.generation).hash(state);
    }
}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.slot, self.generation).cmp(&(other.slot, other.generation))
    }
}

impl<T: Keyed> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}g{}", T::PREFIX, self.slot, self.generation)
    }
}
impl<T: Keyed> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<T: Keyed> std::str::FromStr for Id<T> {
    type Err = ();
    /// Parses "o12g1". Anything else is not an ID (callers then try it as a name).
    fn from_str(s: &str) -> Result<Self, ()> {
        let rest = s.strip_prefix(T::PREFIX).ok_or(())?;
        let (slot, generation) = rest.split_once('g').ok_or(())?;
        Ok(Self::new(
            slot.parse().map_err(|_| ())?,
            generation.parse().map_err(|_| ())?,
        ))
    }
}

impl<T: Keyed> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de, T: Keyed> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse()
            .map_err(|_| serde::de::Error::custom(format!("'{s}' is not an ID")))
    }
}

#[derive(Clone)]
struct Slot<T> {
    generation: u32,
    value: Option<Arc<T>>,
}

/// Slot storage. Removing bumps the slot's generation so stale IDs miss.
#[derive(Clone)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T: Keyed> Arena<T> {
    /// Store a value, giving it a fresh ID (written into the value).
    pub fn insert(&mut self, mut value: T) -> Id<T> {
        // ponytail: linear scan for a free slot; a free list if scenes reach 10k+ objects.
        let slot = self.slots.iter().position(|s| s.value.is_none());
        let slot = slot.unwrap_or_else(|| {
            self.slots.push(Slot {
                generation: 0,
                value: None,
            });
            self.slots.len() - 1
        });
        let entry = &mut self.slots[slot];
        entry.generation += 1;
        let id = Id::new(slot as u32, entry.generation);
        value.set_id(id);
        entry.value = Some(Arc::new(value));
        id
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        let slot = self.slots.get(id.slot as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_deref())
            .flatten()
    }

    /// Mutable access; copies the value first if a snapshot still shares it.
    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.slot as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.value.as_mut().map(Arc::make_mut)
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.slot as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.value
            .take()
            .map(|v| Arc::try_unwrap(v).unwrap_or_else(|shared| (*shared).clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().filter_map(|s| s.value.as_deref())
    }

    pub fn ids(&self) -> impl Iterator<Item = Id<T>> + '_ {
        self.iter().map(Keyed::id)
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn by_name(&self, name: &str) -> Option<&T> {
        // ponytail: linear scan; a name index if scenes reach thousands of objects.
        self.iter().find(|v| v.name() == name)
    }

    /// Resolve an agent or UI reference: an ID string first, then a name.
    pub fn find(&self, reference: &str) -> Option<&T> {
        reference
            .parse::<Id<T>>()
            .ok()
            .and_then(|id| self.get(id))
            .or_else(|| self.by_name(reference))
    }

    /// `base` if free, else `base.001`, `base.002` …
    pub fn unique_name(&self, base: &str) -> String {
        if self.by_name(base).is_none() {
            return base.to_string();
        }
        (1..)
            .map(|n| format!("{base}.{n:03}"))
            .find(|name| self.by_name(name).is_none())
            .expect("some suffix is free")
    }

    /// True when both arenas hold the very same values (pointer-equal).
    pub fn shares_all(&self, other: &Self) -> bool {
        self.slots.len() == other.slots.len()
            && self.slots.iter().zip(&other.slots).all(|(a, b)| {
                a.generation == b.generation
                    && match (&a.value, &b.value) {
                        (Some(x), Some(y)) => Arc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            })
    }

    /// Values in `self` that `other` does not share, for history memory accounting.
    pub fn unshared<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a T> + 'a {
        self.slots.iter().enumerate().filter_map(move |(i, a)| {
            let value = a.value.as_ref()?;
            let shared = other
                .slots
                .get(i)
                .and_then(|b| b.value.as_ref())
                .is_some_and(|b| Arc::ptr_eq(value, b));
            (!shared).then_some(value.as_ref())
        })
    }
}

impl<T: Keyed + PartialEq> PartialEq for Arena<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: Keyed + fmt::Debug> fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Stored as a plain list; each value carries its own ID.
impl<T: Keyed + Serialize> Serialize for Arena<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl<'de, T: Keyed + Deserialize<'de>> Deserialize<'de> for Arena<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values = Vec::<T>::deserialize(d)?;
        let mut arena = Self::default();
        for value in values {
            let id = value.id();
            let i = id.slot as usize;
            if arena.slots.len() <= i {
                arena.slots.resize_with(i + 1, || Slot {
                    generation: 0,
                    value: None,
                });
            }
            if arena.slots[i].value.is_some() {
                return Err(serde::de::Error::custom(format!("duplicate ID {id}")));
            }
            // ponytail: empty slots restart at generation 0 after a load, so an ID
            // deleted before saving can come back. Nothing outlives a reload but the
            // agent's memory; store free-slot generations if that ever bites.
            arena.slots[i] = Slot {
                generation: id.generation,
                value: Some(Arc::new(value)),
            };
        }
        Ok(arena)
    }
}
