pub mod status;

use crate::{
    job,
    math::{Buffs, SpeedStat, XivMath},
    timing::DurationInfo,
};

use self::status::{StatusEffect, StatusEvent, StatusInstance};

pub trait World {
    type Actor<'w>: Actor<'w, World = Self>
    where
        Self: 'w;
    type StatusIter<'w>: Iterator<Item = StatusInstance>
    where
        Self: 'w;
    type ActorIter<'w>: Iterator<Item = &'w Self::Actor<'w>>
    where
        Self: 'w;
    type DurationInfo: DurationInfo;

    /// Returns the actor with the specified [`id`](ActorId), or [`None`]
    /// if no actor with the id exists.
    fn actor(&self, id: ActorId) -> Option<&Self::Actor<'_>>;

    fn duration_info(&self) -> &Self::DurationInfo;
}

pub trait Actor<'w>: 'w {
    type World: World;

    /// Returns the [`Id`](ActorId) of the actor.
    fn id(&self) -> ActorId;
    /// Returns the [`World`] the actor is part of.
    fn world(&self) -> &'w Self::World;

    /// Returns an iterator that contains the
    /// [status effects](status::StatusInstance) present on the actor.
    fn statuses(&self) -> <Self::World as World>::StatusIter<'w>;
    /// Returns `true` if the actor has an `effect` applied by a `source` actor.
    fn has_status(&self, effect: StatusEffect, source: ActorId) -> bool {
        // r-a chokes on this for some reason
        self.statuses()
            .any(|v| v.effect == effect && v.source == source)
    }

    fn get_own_status(&self, effect: StatusEffect) -> Option<StatusInstance> {
        let id = self.id();
        self.statuses()
            .find(|v| v.effect == effect && v.source == id)
    }

    fn has_own_status(&self, effect: StatusEffect) -> bool {
        self.get_own_status(effect).is_some()
    }

    /// Returns the current target of the actor, or [`None`] if the actor has no target.
    fn target(&self) -> Option<&'w Self>;
    /// Returns an iterator of the actors in a certain [`faction`](Faction) that will
    /// be hit by an action with the specified [`targetting`](ActionTargetting).
    fn actors_for_action(
        &self,
        targetting: ActionTargetting,
    ) -> <Self::World as World>::ActorIter<'w>;

    /// Returns the [`Faction`] the actor is part of.
    fn faction(&self) -> Faction;
    /// Returns `true` if a [`positional`](Positional) requirement would be met
    /// on the specified `actor`.
    fn check_positional(&self, positional: Positional, actor: ActorId) -> bool;
    /// Returns `true` if the actor is currently in combat.
    fn in_combat(&self) -> bool;
}

#[derive(Debug, Clone)]
pub enum EventError {
    Gcd,
    Lock,
    Cooldown(job::Action),
    Job(job::Error),
    InCombat,
    NoTarget,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ActorId(pub u16);

// the event/error sink
pub trait EventProxy {
    fn error(&mut self, error: EventError);

    fn event(&mut self, event: Event, time: u32);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Faction {
    Enemy,
    Friendly,
}

#[derive(Debug, Clone)]
pub enum Event {
    Damage(DamageEvent),
    Status(StatusEvent),
    CastSnap(CastSnapEvent),
    Job(job::JobEvent),
    MpTick(ActorId),
    DotTick(ActorId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageEvent {
    pub potency: u16,
    pub force_ch: bool,
    pub force_dh: bool,
    pub target: ActorId,
}
impl DamageEvent {
    pub const fn new(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: false,
            force_dh: false,
            target,
        }
    }
    pub const fn new_ch(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: true,
            force_dh: false,
            target,
        }
    }
    pub const fn new_cdh(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: true,
            force_dh: true,
            target,
        }
    }
    pub const fn new_dh(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: false,
            force_dh: true,
            target,
        }
    }
}
impl From<DamageEvent> for Event {
    fn from(value: DamageEvent) -> Self {
        Event::Damage(value)
    }
}
pub trait DamageEventExt: EventProxy {
    /// Deals damage with a certain `potency` to the specified `target` after a delay.
    fn damage(&mut self, potency: u16, target: ActorId, delay: u32) {
        self.event(DamageEvent::new(potency, target).into(), delay)
    }
    /// Deals damage that will always critical hit with a certain `potency`
    /// to the specified `target` after a delay.
    fn damage_ch(&mut self, potency: u16, target: ActorId, delay: u32) {
        self.event(DamageEvent::new_ch(potency, target).into(), delay)
    }
    /// Deals damage that will always direct hit with a certain `potency`
    /// to the specified `target` after a delay.
    fn damage_dh(&mut self, potency: u16, target: ActorId, delay: u32) {
        self.event(DamageEvent::new_dh(potency, target).into(), delay)
    }
    /// Deals damage that will always critical & direct hit with a certain `potency`
    /// to the specified `target` after a delay.
    fn damage_cdh(&mut self, potency: u16, target: ActorId, delay: u32) {
        self.event(DamageEvent::new_cdh(potency, target).into(), delay)
    }
}
impl<E: EventProxy> DamageEventExt for E {}

impl From<StatusEvent> for Event {
    fn from(value: StatusEvent) -> Self {
        Self::Status(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CastSnapEvent {
    pub action: job::Action,
}
impl CastSnapEvent {
    pub fn new(action: impl Into<job::Action>) -> Self {
        Self {
            action: action.into(),
        }
    }
}
impl From<CastSnapEvent> for Event {
    fn from(value: CastSnapEvent) -> Self {
        Self::CastSnap(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// doubt this will actually ever be useful lol
pub enum ActionTargetting {
    Single {
        range: u8,
    },
    Circle {
        radius: u8,
    },
    TargetCircle {
        range: u8,
        radius: u8,
    },
    Line {
        range: u8,
        // always 2.5y on either side?
        // width: u8,
    },
    Cone {
        range: u8,
        deg: u8,
    },
}

impl ActionTargetting {
    pub const fn single(range: u8) -> Self {
        Self::Single { range }
    }
    pub const fn circle(radius: u8) -> Self {
        Self::Circle { radius }
    }
    pub const fn target_circle(radius: u8, range: u8) -> Self {
        Self::TargetCircle { radius, range }
    }
    pub const fn line(range: u8) -> Self {
        Self::Line { range }
    }
    pub const fn cone(range: u8, deg: u8) -> Self {
        Self::Cone { range, deg }
    }
    pub const fn requires_target(self) -> bool {
        matches!(
            self,
            Self::Single { .. } | Self::Line { .. } | Self::Cone { .. } | Self::TargetCircle { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Positional {
    Front,
    Flank,
    Rear,
}

#[derive(Debug, Clone, Copy)]
pub struct StartCastInfo {
    pub cty: CastType,
    pub gcd: u64,
    pub stat: Option<SpeedStat>,
}

#[derive(Debug, Clone, Copy)]
pub enum CastType {
    Instant { lock: u64 },
    Cast { cast: u64, stat: SpeedStat },
    // literally just iaijutsu. thanks square
    FixCast { cast: u64 },
}

impl CastType {
    pub const fn cast(cast: u64, stat: SpeedStat) -> Self {
        Self::Cast { cast, stat }
    }
    pub const fn instant(lock: u64) -> Self {
        Self::Instant { lock }
    }
    pub const fn fix_cast(cast: u64) -> Self {
        Self::FixCast { cast }
    }
    pub fn get_lock(self, math: &XivMath, buffs: &impl Buffs) -> u64 {
        match self {
            Self::Instant { lock } => lock + math.ex_lock as u64,
            Self::Cast { cast, stat } => math.action_cast_length(cast, stat, buffs) + 10,
            Self::FixCast { cast } => cast + 10,
        }
    }
    pub fn get_snap(self, math: &XivMath, buffs: &impl Buffs) -> u64 {
        match self {
            Self::Instant { .. } => 0,
            Self::Cast { cast, stat } => math
                .action_cast_length(cast, stat, buffs)
                .saturating_sub(50),
            Self::FixCast { cast } => cast.saturating_sub(50),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ComboProc<I> {
    pub combo: I,
    pub time: u32,
}
