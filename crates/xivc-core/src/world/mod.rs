//! The global state of an XIVC simulation.
//!
//! This module contains the proxy traits [`World`] an [`Actor`],
//! which can be used to interact with the global state and actor state
//! respectively.
//!
//! The [`ActorId`] is an opaque handle for an actor inside the world.
//! It is expected that world implementations will not reuse [`ActorId`]s
//! over the course of the simulation.
//!
//! Also of note is the [`status`] submodule. This module contains
//! all of the logic for status effect handling.

pub mod status;

use crate::{job, timing::DurationInfo};

use self::status::{StatusEffect, StatusEvent, StatusInstance};

/// The global state of the world for a simulation.
///
/// This trait is primarily used for the [`actor`][acfn] function,
/// which is used to turn an [`ActorId`] into an [`Actor`].
///
/// [acfn]: World::actor
pub trait World {
    /// The type of the actor proxy this world uses.
    type Actor<'w>: Actor<'w, World = Self>
    where
        Self: 'w;
    /// The type of the iterator used for status instances on an actor.
    type StatusIter<'w>: Iterator<Item = StatusInstance>
    where
        Self: 'w;
    /// The type of the iterator used for iterating through a list of actors.
    type ActorIter<'w>: Iterator<Item = &'w Self::Actor<'w>>
    where
        Self: 'w;
    /// The [`DurationInfo`] that each actor can return.
    type DurationInfo: DurationInfo;

    /// Returns the actor with the specified [`id`], or [`None`]
    /// if no actor with the id exists.
    ///
    /// [`id`]: ActorId
    fn actor(&self, id: ActorId) -> Option<&Self::Actor<'_>>;
}

/// An actor is the world.
///
/// This is often proxy trait for manipulating
/// an actor in the world, not nescessarily a
/// reference to the actual actor.
pub trait Actor<'w>: 'w {
    /// The World type that this actor is from.
    type World: World<Actor<'w> = Self>;

    /// Returns the [`Id`] of the actor.
    ///
    /// [`Id`]: ActorId
    fn id(&self) -> ActorId;
    /// Returns the [`World`] the actor is part of.
    fn world(&self) -> &'w Self::World;

    /// Returns an iterator that contains the
    /// [status effects] present on the actor.
    ///
    /// [status effects]: status::StatusInstance
    fn statuses(&self) -> <Self::World as World>::StatusIter<'w>;
    /// Returns `true` if the actor has an `effect` applied by a `source` actor.
    fn has_status(&self, effect: StatusEffect, source: ActorId) -> bool {
        // r-a chokes on this for some reason
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
    fn target(&self) -> Option<&'w Self>;
    /// Returns an iterator of the actors in a certain [`Faction`] that will
    /// be hit by an action with the specified [`ActionTargetting`].
    fn actors_for_action(
        &self,
        faction: Option<Faction>,
        targetting: ActionTargetting,
    ) -> <Self::World as World>::ActorIter<'w>;

    /// Returns the [`Faction`] the actor is part of.
    fn faction(&self) -> Faction;
    /// Returns `true` if a [`Positional`] requirement would be met
    /// on the specified `actor`.
    fn check_positional(&self, positional: Positional, actor: ActorId) -> bool;
    /// Returns `true` if the actor is currently in combat.
    fn in_combat(&self) -> bool;
    /// Returns the [`DurationInfo`] for this actor.
    fn duration_info(&self) -> <Self::World as World>::DurationInfo;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
/// An ID of an [`Actor`].
///
/// While the value inside this struct is public,
/// it should be treated as an opaque type,
/// in the sense that a [`World`] may store data
/// in it however it wants to.
pub struct ActorId(pub u16);

/// A sink for events and errors.
pub trait EventProxy {
    /// Submits an error into the event proxy.
    fn error(&mut self, error: EventError);
    /// Submits an event into the event proxy to be executed after a specified delay.
    fn event(&mut self, event: Event, delay: u32);
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

#[derive(Debug, Clone)]
#[allow(missing_docs)]
/// An event that can be submitted to the event queue.
pub enum Event {
    Damage(DamageEvent),
    Status(StatusEvent),
    Job(job::JobEvent),
    MpTick(ActorId),
    DotTick(ActorId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A damage application event.
pub struct DamageEvent {
    /// The potency of the damage.
    pub potency: u16,
    /// Whether the damage automatically critical hits.
    pub force_ch: bool,
    /// Whether the damage automatically direct hits.
    pub force_dh: bool,
    /// The target receiving the damage.
    pub target: ActorId,
}
impl DamageEvent {
    /// Deals damage with a certain `potency` to the specified `target`.
    pub const fn new(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: false,
            force_dh: false,
            target,
        }
    }
    /// Deals damage that will always critical hit with a certain `potency`
    /// to the specified `target`.
    pub const fn new_ch(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: true,
            force_dh: false,
            target,
        }
    }
    /// Deals damage that will always critical & direct hit with a certain `potency`
    /// to the specified `target`.
    pub const fn new_cdh(potency: u16, target: ActorId) -> Self {
        Self {
            potency,
            force_ch: true,
            force_dh: true,
            target,
        }
    }
    /// Deals damage that will always direct hit with a certain `potency`
    /// to the specified `target`.
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

/// A helper trait for easily submitting damage events on to an event proxy.
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
