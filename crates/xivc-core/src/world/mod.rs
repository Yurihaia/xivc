//! The global state of an XIVC simulation.
//!
//! This module contains the traits [`WorldRef`] and [`ActorRef`], which can be used to
//! read global state and actor state respectively.
//!
//! The [`ActorId`] is an opaque handle for an actor inside the world.
//! It is expected that world implementations will not reuse [`ActorId`]s
//! over the course of the simulation.
//!
//! Also of note is the [`status`] submodule. This module contains
//! all of the logic for status effect handling.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub mod status;

pub mod queue;

use rand::distributions::Distribution;

use crate::{
    enums::{ActionCategory, DamageInstance},
    job,
    math::{EotSnapshot, HitTypeHandle, SpeedStat},
    timing::DurationInfo,
};

use self::status::{StatusEffect, StatusEvent, StatusInstance};

/// The global state of the world for a simulation.
///
/// This trait is primarily used for the [`actor`] function,
/// which is used to turn an [`ActorId`] into an [`ActorRef`].
///
/// [`actor`]: WorldRef::actor
pub trait WorldRef<'w>: Clone + Sized {
    /// The type of the actor proxy this world uses.
    type Actor: ActorRef<'w, World = Self>;
    /// The [`DurationInfo`] that each actor can return.
    type DurationInfo: DurationInfo;

    /// Returns the actor with the specified [`id`], or [`None`]
    /// if no actor with the id exists.
    ///
    /// [`id`]: ActorId
    fn actor(&self, id: ActorId) -> Option<Self::Actor>;
}

/// A reference to an actor in the world.
///
/// This is often a proxy trait for manipulating
/// an actor in the world, not nescessarily a
/// reference to the actual actor.
///
/// This trait only exposes an immutable API.
/// To make changes to actors and the world, submit events to an [`EventSink`].
pub trait ActorRef<'w>: Clone + Sized {
    /// The World type that this actor is from.
    type World: WorldRef<'w, Actor = Self>;

    /// Returns the [`Id`] of the actor.
    ///
    /// [`Id`]: ActorId
    fn id(&self) -> ActorId;
    /// Returns the [`WorldRef`] the actor is part of.
    fn world(&self) -> Self::World;
    /// Returns the calculated damage of an attack.
    fn attack_damage<R>(&self, damage: DamageInstance, target: ActorId, rng: &mut R) -> u64
    where
        R: EventRng;
    /// Returns the snapshot for a damage over time effect.
    fn dot_damage_snapshot(
        &self,
        damage: DamageInstance,
        stat: SpeedStat,
        target: ActorId,
    ) -> EotSnapshot;
    /// Returns the calculated damage for an auto attack.
    fn auto_damage<R>(&self, target: ActorId, rng: &mut R) -> u64
    where
        R: EventRng;

    /// Returns an iterator that contains the
    /// [status effects] present on the actor.
    ///
    /// [status effects]: status::StatusInstance
    fn statuses(&self) -> impl Iterator<Item = StatusInstance>;
    /// Returns `true` if the actor has an `effect` applied by a `source` actor.
    fn has_status(&self, effect: StatusEffect, source: ActorId) -> bool {
        self.statuses()
            .any(|v| v.effect == effect && v.source == source)
    }
    /// Returns a status instance applied by this actor of the specified effect.
    fn get_own_status(&self, effect: StatusEffect) -> Option<StatusInstance> {
        let id = self.id();
        self.statuses()
            .find(|v| v.effect == effect && v.source == id)
    }
    /// Returns `true` if this actor has a status instance applied by
    /// this actor of the specified effect.
    fn has_own_status(&self, effect: StatusEffect) -> bool {
        self.get_own_status(effect).is_some()
    }

    /// Returns the current target of the actor, or [`None`] if the actor has no target.
    fn target(&self) -> Option<Self>;
    /// Returns an iterator of the actors in a certain [`Faction`] that will
    /// be hit by an action with the specified [`ActionTargetting`].
    fn actors_for_action(
        &self,
        faction: Option<Faction>,
        targetting: ActionTargetting,
    ) -> impl Iterator<Item = Self>;

    /// Returns `true` if the other actor is within the specified action targetting range.
    fn within_range(&self, other: ActorId, targetting: ActionTargetting) -> bool;

    /// Returns the amount of MP a player actor has.
    fn mp(&self) -> u16;

    /// Returns the [`Faction`] the actor is part of.
    fn faction(&self) -> Faction;
    /// Returns `true` if a [`Positional`] requirement would be met
    /// on the specified `actor`.
    fn check_positional(&self, positional: Positional, actor: ActorId) -> bool;
    /// Returns `true` if the actor is currently in combat.
    fn in_combat(&self) -> bool;
    /// Returns the [`DurationInfo`] for this actor.
    fn duration_info(&self) -> <Self::World as WorldRef<'w>>::DurationInfo;
}

#[derive(Debug, Clone)]
/// An error that happens due to an event.
pub enum EventError {
    /// The GCD is still active.
    Gcd,
    /// The cast lock is still active.
    Lock,
    /// The action is still on cooldown.
    Cooldown(job::Action),
    /// A job specific error.
    Job(job::CastError),
    /// The actor is not in combat.
    InCombat,
    /// No target exists.
    NoTarget,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
/// An ID of an [`ActorRef`].
///
/// While the value inside this struct is public,
/// it should be treated as an opaque type,
/// in the sense that a [`WorldRef`] may store data
/// in it however it wants to.
pub struct ActorId(pub u16);

/// A sink for events and errors.
pub trait EventSink<'w, W: WorldRef<'w>> {
    /// The source of randomness for this `EventSink`.
    type Rng: EventRng;
    /// Returns the source actor that the events will come from.
    fn source(&self) -> W::Actor;
    /// Submits an error into the event sink.
    fn error(&mut self, error: EventError);
    /// Submits an event into the event sink to be executed after a specified delay.
    ///
    /// <div class="warning" id="orderwarning">
    ///
    /// The order in which events with the same `delay` will be executed is the
    /// opposite of the order they were submitted in. You must be careful if the effect
    /// of an event depends on other events.
    /// Consider using [`events_ordered`] in this situation
    /// to make the ordering of events explicit.
    ///
    /// However, this ordering may be useful. For example, the [`Job::event`] function can
    /// submit events with a delay of `0`, and those events will be guaranteed to be executed
    /// before any events currently awaiting execution.
    ///
    /// </div>
    ///
    /// [`events_ordered`]: EventSink::events_ordered
    /// [`Job::event`]: crate::job::Job::event
    fn event(&mut self, event: Event, delay: u32);
    /// Submits a sequence of events to be executed in order.
    ///
    /// Because of the [warning] in [`event`], the default implementation
    /// is to insert these events in the reverse order, hence the [`DoubleEndedIterator`]
    /// bound. However, note that the ordering between multiple calls to this function
    /// will result in the latter call's events being executed first.
    ///
    /// [warning]: EventSink#orderwarning
    /// [`event`]: EventSink::event
    fn events_ordered<I>(&mut self, events: I, delay: u32)
    where
        I: IntoIterator<Item = Event>,
        I::IntoIter: DoubleEndedIterator,
    {
        for event in events.into_iter().rev() {
            self.event(event, delay);
        }
    }
    /// Returns the source of randomness for this `EventSink`. See [`EventRng`]'s documentation for more.
    fn rng(&mut self) -> &mut Self::Rng;

    /// A convenience function that forwards the call to the [`random()`] function on `Self::Rng`.
    /// See that function's documentation for more.
    ///
    /// [`random()`]: EventRng::random
    fn random<D, T>(&mut self, distr: D) -> T
    where
        D: Distribution<T> + 'static,
        T: 'static,
    {
        self.rng().random(distr)
    }
}

/// A controllable source of randomness for the simulation.
pub trait EventRng {
    /// Returns a random value of type `T` from the distribution `D`.
    ///
    /// This API is made this way to give `EventRng` implementers the ability
    /// to fabricate results from an RNG.
    ///
    /// Implementors should make sure that, while *statistically* the returned value doesn't
    /// have to match the distribution, the returned value should not fall outside the range
    /// of values that the distribution could produce.
    ///
    /// Both `D` and `T` are `'static` to allow the use of [`TypeId`]. This allows implementors
    /// to override specific instances of distributions with other custom values.
    /// Because of this, it is recommended that users of this method create custom [`Distribution`]
    /// implementations for the various sources of randomness within a job.
    ///
    /// [`TypeId`]: core::any::TypeId
    fn random<D, T>(&mut self, distr: D) -> T
    where
        D: Distribution<T> + 'static,
        T: 'static;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The faction an actor is in.
///
/// This enum is used to control the targetting of actions.
pub enum Faction {
    /// The actor is an enemy.
    Enemy,
    /// The actor is in the current player's party.
    Party,
    /// The actor is friendly, but not in the
    /// current player's party.
    Friendly,
}

/// An event that can be submitted to the event queue.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub enum Event {
    Damage(DamageEvent),
    Status(StatusEvent),
    Job(job::JobEvent, ActorId),
    AdvCd(job::CdGroup, u32, ActorId),
    AddMp(u16, ActorId),
    MpTick(ActorId),
    ActorTick(ActorId),
}

/// A damage application event.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageEvent {
    /// The damage of the event.
    pub damage: u64,
    /// The actor dealing the damage.
    pub source: ActorId,
    /// The target receiving the damage.
    pub target: ActorId,
    /// The action dealing the damage.
    pub action: Action,
}
impl DamageEvent {
    /// Creates a new damage event.
    pub const fn new(damage: u64, source: ActorId, target: ActorId, action: Action) -> Self {
        Self {
            damage,
            source,
            target,
            action,
        }
    }
}
impl From<DamageEvent> for Event {
    fn from(value: DamageEvent) -> Self {
        Event::Damage(value)
    }
}

/// A random event determining whether an instance of damage
/// will critically hit.
pub struct CriticalHit {
    chance: u16,
}

impl CriticalHit {
    /// Creates a new instance of this `struct` with the specified `chance`
    /// of a critical hit occuring. This `chance` is a probability scaled by `1000`.
    pub const fn new(chance: u16) -> Self {
        Self {
            chance: if chance > 1000 { 1000 } else { chance },
        }
    }
}

impl Distribution<bool> for CriticalHit {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> bool {
        rng.gen_ratio(self.chance as u32, 1000)
    }
}
impl Distribution<HitTypeHandle> for CriticalHit {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> HitTypeHandle {
        HitTypeHandle::new(rng.sample(self))
    }
}

/// A random event determining whether an instance of damage
/// will direct hit.
pub struct DirectHit {
    chance: u16,
}

impl DirectHit {
    /// Creates a new instance of this `struct` with the specified `chance`
    /// of a direct hit occuring. This `chance` is a probability scaled by `1000`.
    pub const fn new(chance: u16) -> Self {
        Self {
            chance: if chance > 1000 { 1000 } else { chance },
        }
    }
}

impl Distribution<bool> for DirectHit {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> bool {
        rng.gen_ratio(self.chance as u32, 1000)
    }
}
impl Distribution<HitTypeHandle> for DirectHit {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> HitTypeHandle {
        HitTypeHandle::new(rng.sample(self))
    }
}

/// A random event determining the +/-5% damage variance of
/// an attack.
pub struct DamageVariance(());

impl DamageVariance {
    /// Creates a new instance of this `struct`.
    pub const fn new() -> Self {
        Self(())
    }
}

impl Distribution<u64> for DamageVariance {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> u64 {
        rng.gen_range(9500..=10500)
    }
}

/// A helper trait for easily submitting damage events on to an event sink.
pub trait DamageEventExt<'w, W: WorldRef<'w>>: EventSink<'w, W> {
    /// Deals damage to the target after the specified delay.
    fn damage<'a>(
        &mut self,
        action: impl Into<Action>,
        damage: DamageInstance,
        target: ActorId,
        delay: u32,
    ) {
        let actor = self.source();
        let damage = actor.attack_damage(damage, target, self.rng());
        self.event(
            DamageEvent::new(damage, actor.id(), target, action.into()).into(),
            delay,
        )
    }
}
impl<'w, W: WorldRef<'w>, E: EventSink<'w, W>> DamageEventExt<'w, W> for E {}

/// An action that an actor can cast.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// An action used by a job.
    Job(job::Action),
}

impl Action {
    /// Returns the [`ActionCategory`] for this action.
    pub fn category(&self) -> ActionCategory {
        match self {
            Self::Job(v) => v.category(),
        }
    }
    /// Returns `true` if this action is a GCD.
    pub fn gcd(&self) -> bool {
        match self {
            Self::Job(v) => v.gcd(),
        }
    }
    /// Returns the name of the action.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Job(v) => v.name(),
        }
    }
}

impl From<StatusEvent> for Event {
    fn from(value: StatusEvent) -> Self {
        Self::Status(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
/// The kinds of targetting that an action can have.
pub enum ActionTargetting {
    /// A single target action with a certain maximum `range`.
    Single { range: u8 },
    /// A circle centered on the executing actor with a certain `radius`.
    Circle { radius: u8 },
    /// A circle centered on the actor's target with a certain `range` and `radius`.
    TargetCircle { range: u8, radius: u8 },
    /// A line directed at the actor's target with a certain `range`.
    Line {
        range: u8,
        // always 2.5y on either side?
        // width: u8,
    },
    /// A cone directed at the actor's target with a certain `range` and an `angle`.
    ///
    /// The angle will be in degrees.
    Cone { range: u8, angle: u8 },
}

impl ActionTargetting {
    /// Creates a new single target action with a certain maximum `range`.
    pub const fn single(range: u8) -> Self {
        Self::Single { range }
    }
    /// Creates a new circle action centered on the actor with a certain `radius`.
    pub const fn circle(radius: u8) -> Self {
        Self::Circle { radius }
    }
    /// Creates a new circle action centered on the actor's target with a certain `range` and `radius`.
    pub const fn target_circle(radius: u8, range: u8) -> Self {
        Self::TargetCircle { radius, range }
    }
    /// Creates a new line action directed at the actor's target with a certain `range`.
    pub const fn line(range: u8) -> Self {
        Self::Line { range }
    }
    /// Creates a new cone action directed at the actor's target with a certain `range` and an `angle`.
    ///
    /// The angle will be in degrees.
    pub const fn cone(range: u8, deg: u8) -> Self {
        Self::Cone { range, angle: deg }
    }
    /// Returns `true` if the action targetting requires a target.
    pub const fn requires_target(self) -> bool {
        matches!(
            self,
            Self::Single { .. } | Self::Line { .. } | Self::Cone { .. } | Self::TargetCircle { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// An action positional.
pub enum Positional {
    /// The front of the target.
    Front,
    /// The two flanks of the target.
    Flank,
    /// The rear of the target.
    Rear,
}
