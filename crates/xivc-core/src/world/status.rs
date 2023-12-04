use core::{fmt, hash, ops::Deref};

use crate::{
    enums::DamageInstance,
    math::{Buffs, PlayerStats},
};

use super::{ActorId, EventProxy, Actor};

#[derive(Debug, Clone, Copy)]
pub struct StatusInstance {
    pub source: ActorId,
    pub effect: StatusEffect,
    pub time: u32,
    pub stack: u8,
}

impl StatusInstance {
    pub const fn new(source: ActorId, effect: StatusEffect) -> Self {
        Self::new_stack(source, effect, 1)
    }
    pub const fn new_stack(source: ActorId, effect: StatusEffect, stack: u8) -> Self {
        Self {
            source,
            effect,
            stack,
            time: effect.0.duration,
        }
    }
    pub fn remove(&mut self) {
        self.stack = 0;
    }
    pub fn sub_stacks(&mut self, stacks: u8) {
        self.stack = self.stack.saturating_sub(stacks);
    }
    pub fn add_stacks(&mut self, stacks: u8, max: u8) {
        self.stack = (self.stack + stacks).min(max);
    }
    pub fn advance(&mut self, time: u32) {
        if !self.effect.permanent {
            self.time = self.time.saturating_sub(time);
            if self.time == 0 {
                self.stack = 0;
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct StatusEffect(&'static StatusVTable);

impl StatusEffect {
    pub const fn new(vtable: &'static StatusVTable) -> Self {
        Self(vtable)
    }
}
impl PartialEq for StatusEffect {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}
impl Eq for StatusEffect {}
impl Deref for StatusEffect {
    type Target = StatusVTable;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl hash::Hash for StatusEffect {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        <usize as hash::Hash>::hash(&(self.0 as *const _ as usize), state);
    }
}
impl fmt::Debug for StatusEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StatusEffect(\"{}\" @ {:?})",
            self.0.name, self.0 as *const StatusVTable
        )
    }
}

#[derive(Debug)]
pub struct StatusVTable {
    pub name: &'static str,
    pub permanent: bool,
    pub damage: ValueModifier<DamageModifierFn>,
    pub crit: ValueModifier<ProbabilityModifierFn>,
    pub dhit: ValueModifier<ProbabilityModifierFn>,
    pub haste: Option<SpeedModifierFn>,
    pub stats: Option<StatsModifierFn>,
    pub duration: u32,
}

#[derive(Debug)]
pub struct StatusSnapshot<'a> {
    pub source: &'a [StatusInstance],
    pub target: &'a [StatusInstance],
    // this is a list of all of the "status effects" that come from
    // job gauge or trait related things
    // some examples of this include: darkside, army's paeon haste, enochian,
    // AF/UI (i believe it goes here), greased lightning, etc.
    // note that Main & Mend (things that say "base action damage")
    // do NOT apply here, they go in "traits"
    pub source_gauge: &'a [StatusInstance],
}

impl<'a> Buffs for StatusSnapshot<'a> {
    // doing these functions using iterators was less concise because of the incoming vs outgoing difference
    fn damage(&self, base: DamageInstance) -> DamageInstance {
        let mut acc = base;
        for x in self.target {
            if let Some(f) = x.effect.damage.incoming {
                acc = f(*x, acc);
            }
        }
        for x in self.source {
            if let Some(f) = x.effect.damage.outgoing {
                acc = f(*x, acc);
            }
        }
        for x in self.source_gauge {
            if let Some(f) = x.effect.damage.outgoing {
                acc = f(*x, acc);
            }
        }
        acc
    }

    fn crit_chance(&self, base: u64) -> u64 {
        let mut acc = base;
        for x in self.target {
            if let Some(f) = x.effect.crit.incoming {
                acc = f(*x, acc);
            }
        }
        for x in self.source {
            if let Some(f) = x.effect.crit.outgoing {
                acc = f(*x, acc);
            }
        }
        for x in self.source_gauge {
            if let Some(f) = x.effect.crit.outgoing {
                acc = f(*x, acc);
            }
        }
        acc
    }

    fn dhit_chance(&self, base: u64) -> u64 {
        let mut acc = base;
        for x in self.target {
            if let Some(f) = x.effect.dhit.incoming {
                acc = f(*x, acc);
            }
        }
        for x in self.source {
            if let Some(f) = x.effect.dhit.outgoing {
                acc = f(*x, acc);
            }
        }
        for x in self.source_gauge {
            if let Some(f) = x.effect.dhit.outgoing {
                acc = f(*x, acc);
            }
        }
        acc
    }

    fn stats(&self, base: PlayerStats) -> PlayerStats {
        let mut acc = base;
        for x in self.source {
            if let Some(f) = x.effect.stats {
                acc = f(*x, acc);
            }
        }
        for x in self.source_gauge {
            if let Some(f) = x.effect.stats {
                acc = f(*x, acc);
            }
        }
        acc
    }

    // pretty sure this works
    // doubt it will ever matter bc its not like
    // jobs have more than one speed boost
    fn haste(&self, base: u64) -> u64 {
        let mut acc = 100;
        for x in self.source {
            if let Some(f) = x.effect.haste {
                acc *= f(*x);
                acc /= 100;
            }
        }
        for x in self.source_gauge {
            if let Some(f) = x.effect.haste {
                acc *= f(*x);
                acc /= 100;
            }
        }
        base * acc / 100
    }
}

pub type DamageModifierFn = fn(e: StatusInstance, damage: DamageInstance) -> DamageInstance;
// Chance is a scaled by 1000 to get probability (every 1 is 0.1%)
// For instance, Chain adds 100
pub type ProbabilityModifierFn = fn(e: StatusInstance, chance: u64) -> u64;
// for potions and basically potions only?
// technically food but that can usually be put into stats
pub type StatsModifierFn = fn(e: StatusInstance, stats: PlayerStats) -> PlayerStats;
// haste buffs - returns the buff as a multiplier scaled by 100
pub type SpeedModifierFn = fn(e: StatusInstance) -> u64;

#[macro_export]
macro_rules! status_effect {
    (
        $name:literal $ptk:tt $({
            $(
                $prop:ident { $($t:tt)* }
            )*
        })?
    ) => {
        $crate::world::status::StatusEffect::new(&$crate::world::status::StatusVTable {
            permanent: $crate::__status_effect_inner!(ptk $ptk),
            duration: $crate::__status_effect_inner!(ptd $ptk),
            $($(
                $prop: $crate::__status_effect_inner!(vm $prop { $($t)* }),
            )*)?
            ..$crate::world::status::StatusVTable::empty($name)
        })
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __status_effect_inner {
    (ptk permanent) => { true };
    (ptk $dur:expr) => { false };
    (ptd permanent) => { 0 };
    (ptd $dur:expr) => { $dur };
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
    (vm haste { $e:expr }) => { Some(|_| $e) };
    (modfn in $($t:tt)*) => { $crate::world::status::ValueModifier::incoming($($t)*) };
    (modfn out $($t:tt)*) => { $crate::world::status::ValueModifier::outgoing($($t)*) };
}

impl StatusVTable {
    const EMPTY: Self = Self {
        name: "",
        permanent: false,
        damage: ValueModifier::empty(),
        crit: ValueModifier::empty(),
        dhit: ValueModifier::empty(),
        duration: 0,
        haste: None,
        stats: None,
    };

    pub const fn empty(name: &'static str) -> Self {
        StatusVTable {
            name,
            ..Self::EMPTY
        }
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

pub fn consume_status<'w, A: Actor<'w>>(
    actor: &A,
    proxy: &mut impl EventProxy,
    status: StatusEffect,
    time: u32,
) -> bool {
    let present = actor.has_own_status(status);
    if present {
        proxy.event(StatusEvent::remove(status, actor.id()).into(), time);
    }
    present
}

pub fn consume_status_stack<'w, A: Actor<'w>>(
    actor: &A,
    proxy: &mut impl EventProxy,
    status: StatusEffect,
    time: u32,
) -> bool {
    let present = actor.has_own_status(status);
    if present {
        proxy.event(StatusEvent::remove_stacks(status, 1, actor.id()).into(), time);
    }
    present
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusEvent {
    pub status: StatusEffect,
    pub target: ActorId,
    pub kind: StatusEventKind,
}
impl StatusEvent {
    pub const fn apply(status: StatusEffect, stacks: u8, target: ActorId) -> Self {
        Self {
            kind: StatusEventKind::Apply {
                duration: status.0.duration,
                stacks,
            },
            status,
            target,
        }
    }
    pub const fn apply_dot(
        status: StatusEffect,
        potency: u16,
        stacks: u8,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::ApplyDot {
                duration: status.0.duration,
                potency,
                stacks,
            },
            status,
            target,
        }
    }
    pub const fn remove(status: StatusEffect, target: ActorId) -> Self {
        Self {
            kind: StatusEventKind::Remove,
            status,
            target,
        }
    }
    pub const fn remove_stacks(status: StatusEffect, stacks: u8, target: ActorId) -> Self {
        Self {
            kind: StatusEventKind::RemoveStacks { stacks },
            status,
            target,
        }
    }
    pub const fn add_stacks(status: StatusEffect, stacks: u8, max: u8, target: ActorId) -> Self {
        Self {
            kind: StatusEventKind::AddStacks { stacks, max },
            status,
            target,
        }
    }
    pub const fn apply_or_extend(
        status: StatusEffect,
        stacks: u8,
        multiple: u32,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::ApplyOrExtend {
                duration: status.0.duration,
                stacks,
                max: status.0.duration * multiple,
            },
            status,
            target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEventKind {
    Apply {
        duration: u32,
        stacks: u8,
    },
    ApplyDot {
        duration: u32,
        potency: u16,
        stacks: u8,
    },
    Remove,
    RemoveStacks {
        stacks: u8,
    },
    AddStacks {
        stacks: u8,
        max: u8,
    },
    ApplyOrExtend {
        duration: u32,
        stacks: u8,
        max: u32,
    },
}

pub trait StatusEventExt: EventProxy {
    /// Apply a [`status`](StatusEffect) to the specified `target` with a
    /// certain number of `stacks` after a `delay`.
    fn apply_status(&mut self, status: StatusEffect, stacks: u8, target: ActorId, delay: u32) {
        self.event(StatusEvent::apply(status, stacks, target).into(), delay)
    }
    /// Apply a Damage-over-Time [`status`](StatusEffect) to the specified `target` with a
    /// certain `potency` and number of `stacks` after a `delay`.
    fn apply_dot(
        &mut self,
        status: StatusEffect,
        potency: u16,
        stacks: u8,
        target: ActorId,
        time: u32,
    ) {
        self.event(
            StatusEvent::apply_dot(status, potency, stacks, target).into(),
            time,
        )
    }
    /// Remove a [`status`](StatusEffect) from the specified `target` after a `delay`.
    fn remove_status(&mut self, status: StatusEffect, target: ActorId, delay: u32) {
        self.event(StatusEvent::remove(status, target).into(), delay)
    }
    /// Remove a number of `stacks` from the [`status`](StatusEffect)
    /// on the specified `target` after a `delay`.
    fn remove_stacks(&mut self, status: StatusEffect, stacks: u8, target: ActorId, delay: u32) {
        self.event(
            StatusEvent::remove_stacks(status, stacks, target).into(),
            delay,
        )
    }
    /// Add a number of `stacks` from the [`status`](StatusEffect)
    /// on the specified `target` after a `delay`.
    fn add_stacks(
        &mut self,
        status: StatusEffect,
        stacks: u8,
        max: u8,
        target: ActorId,
        delay: u32,
    ) {
        self.event(
            StatusEvent::add_stacks(status, stacks, max, target).into(),
            delay,
        )
    }
    fn apply_or_extend_status(
        &mut self,
        status: StatusEffect,
        stacks: u8,
        multiple: u32,
        target: ActorId,
        delay: u32,
    ) {
        self.event(
            StatusEvent::apply_or_extend(status, stacks, multiple, target).into(),
            delay,
        )
    }
}

impl<E: EventProxy> StatusEventExt for E {}
