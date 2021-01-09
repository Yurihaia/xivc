mod event;
pub use event::*;
use radix_heap::RadixHeapMap;
pub mod cooldown;

use std::{cmp::Reverse, collections::HashMap, fmt, hash::{self, Hash}, ops::Deref};

use crate::{
    arena::Arena,
    arena_id,
    enums::{DamageElement, DamageType},
    math::PlayerStats,
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ActionError<J> {
    GlobalCooldown(u32),
    AnimationLock(u32),
    ActionCooldown(u32),
    TargetAbsent,
    Job(J),
}

arena_id! {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
    pub struct ActorId;
}

// The entire sim runtime is based off of an event queue
// The user will essentially create a state machine to progress the queue
// and the Runtime struct only serves to glue all of the parts together
// The Runtime contains a number of Arenas used to register things for easy lookup
// It is expected that the arenas only ever contain a finite amount of objects and that the objects never
// need to be removed
#[derive(Debug)]
pub struct Runtime<E> {
    global_time: u32,
    event_queue: RadixHeapMap<Reverse<u32>, E>,
    actors: Arena<ActorId, Actor>,
}

impl<E> Runtime<E> {
    pub fn new() -> Self {
        Runtime {
            global_time: 0,
            event_queue: RadixHeapMap::new(),
            actors: Arena::new(),
        }
    }

    pub fn add_event(&mut self, event: E, delay: u32) {
        self.event_queue.push(Reverse(self.global_time + delay), event);
    }

    pub fn global(&self) -> u32 {
        self.global_time
    }

    pub fn events(&self) -> usize {
        self.event_queue.len()
    }

    pub fn advance(&mut self) -> Option<(u32, E)> {
        let (gt, e) = self.event_queue.pop()?;
        let dt = gt.0 - self.global_time;
        self.global_time = gt.0;
        for actor in self.actors.iter_mut() {
            actor.effects.retain(|_, v| {
                v.time = v.time.saturating_sub(dt);
                v.time > 0
            });
        }
        Some((dt, e))
    }

    pub fn add_actor(&mut self, actor: Actor) -> ActorId {
        self.actors.push(actor)
    }
    pub fn get_actor_mut(&mut self, actor: ActorId) -> Option<&mut Actor> {
        self.actors.get_mut(actor)
    }
    pub fn get_actor(&self, actor: ActorId) -> Option<&Actor> {
        self.actors.get(actor)
    }
}
impl<E> Default for Runtime<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub name: &'static str,
    pub health: u64,
    pub effects: HashMap<(ActorId, StatusEffect), EffectInstance>,
    pub mirrors: Vec<ActorId>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DamageInstance {
    pub dmg: u64,
    pub ty: DamageType,
    pub el: DamageElement,
}

#[derive(Copy, Clone, Debug)]
pub struct EffectInstance {
    pub effect: StatusEffect,
    pub time: u32,
    pub stack: u8,
}

impl EffectInstance {
    pub fn new(effect: StatusEffect, time: u32, stack: u8) -> Self {
        Self {
            effect,
            time,
            stack,
        }
    }
    pub fn advance(&mut self, time: u32) -> bool {
        self.time = self.time.saturating_sub(time);
        self.time == 0
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ValueModifier<T> {
    pub incoming: Option<T>,
    pub outgoing: Option<T>,
}
impl<T> Default for ValueModifier<T> {
    fn default() -> Self {
        ValueModifier {
            incoming: None,
            outgoing: None,
        }
    }
}
impl<T> ValueModifier<T> {
    pub const fn empty() -> Self {
        Self {
            incoming: None,
            outgoing: None,
        }
    }
    pub const fn outgoing(t: T) -> Self {
        Self {
            incoming: None,
            outgoing: Some(t),
        }
    }
    pub const fn incoming(t: T) -> Self {
        Self {
            incoming: Some(t),
            outgoing: None,
        }
    }
}

pub type DamageModifierFn = fn(e: EffectInstance, damage: DamageInstance) -> DamageInstance;
pub type ProbabilityModifierFn = fn(e: EffectInstance, chance: u64) -> u64;
pub type StatsModifierFn = fn(e: EffectInstance, stats: PlayerStats) -> PlayerStats;

// Outdated prototype for the status effect system
// pub type TickFn = fn(e: EffectInstance, elapsed: u32) -> bool;

#[derive(Copy, Clone)]
pub struct StatusEffect(&'static StatusEffectVTable);

impl StatusEffect {
    pub const fn new(vtable: &'static StatusEffectVTable) -> Self {
        Self(vtable)
    }
}
impl PartialEq for StatusEffect {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}
impl Eq for StatusEffect {}
impl Deref for StatusEffect {
    type Target = StatusEffectVTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl hash::Hash for StatusEffect {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        <usize as hash::Hash>::hash(&(self.0 as *const _ as usize), state);
    }
}
impl fmt::Debug for StatusEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StatusEffect(\"{}\" @ {:?})", self.0.name, self.0 as *const StatusEffectVTable)
    }
}

#[derive(Debug)]
pub struct StatusEffectVTable {
    pub name: &'static str,
    pub damage: ValueModifier<DamageModifierFn>,
    pub crit: ValueModifier<ProbabilityModifierFn>,
    pub dhit: ValueModifier<ProbabilityModifierFn>,
    pub stats: Option<StatsModifierFn>,
}

#[macro_export]
macro_rules! status_effect {
    (
        $name:literal $({
            $(
                $prop:ident { $($t:tt)* }
            )*
        })?
    ) => {
        $crate::sim::StatusEffect::new(&$crate::sim::StatusEffectVTable {
            $($(
                $prop: $crate::__status_effect_inner!(vm $prop { $($t)* }),
            )*)?
            ..$crate::sim::StatusEffectVTable::empty($name)
        })
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __status_effect_inner {
    (vm damage {
        $k:ident = $mul:literal / $div:literal
    }) => {
        $crate::__status_effect_inner!(modfn $k |_, mut d| {
            d.dmg *= $mul;
            d.dmg /= $div;
            d
        })
    };
    (vm crit {
        $k:ident = $add:literal
    }) => {
        $crate::__status_effect_inner!(modfn $k |_, c| c + $add)
    };
    (vm dhit {
        $k:ident = $add:literal
    }) => {
        $crate::__status_effect_inner!(modfn $k |_, c| c + $add)
    };
    (vm $i:ident {
        $k:ident = $e:expr
    }) => {
        $crate::__status_effect_inner!(modfn $k $e)
    };
    (vm stats { $e:expr }) => { Some($e) };
    (modfn in $($t:tt)*) => { $crate::sim::ValueModifier::incoming($($t)*) };
    (modfn out $($t:tt)*) => { $crate::sim::ValueModifier::outgoing($($t)*) };
}

impl StatusEffectVTable {
    const EMPTY: Self = Self {
        name: "",
        damage: ValueModifier::empty(),
        crit: ValueModifier::empty(),
        dhit: ValueModifier::empty(),
        stats: None,
    };

    pub const fn empty(name: &'static str) -> Self {
        StatusEffectVTable {
            name,
            ..Self::EMPTY
        }
    }
}
