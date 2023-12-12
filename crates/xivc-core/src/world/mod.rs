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

#[cfg(feature = "alloc")]
pub mod queue;

use rand_core::RngCore;

use crate::{enums::DamageInstance, job, math::EotSnapshot, timing::DurationInfo};

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
    /// Returns the calculated damage of an attack.
    fn attack_damage(&self, damage: DamageInstance, target: ActorId) -> u64;
    /// Returns the snapshot for a damage over time effect.
    fn dot_damage_snapshot(&self, damage: DamageInstance, target: ActorId) -> EotSnapshot;
    /// Returns the calculated damage for an auto attack.
    fn auto_damage(&self, target: ActorId) -> u64;

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
    /// 
    /// <div class="warning" id="orderwarning">
    /// 
    /// The order in which events with the same `delay` will be executed is the
    /// opposite of the order they were submitted in. You must be careful if the effect
    /// of an event depends on other events.
    /// Consider using [`events_ordered`] in this situation
    /// to make the ordering of events explicit.<br><br>
    /// However, this ordering may be useful. For example, the [`Job::event`] function can
    /// submit events with a delay of `0`, and those events will be guaranteed to be executed
    /// before any events currently awaiting execution.
    /// 
    /// </div>
    /// 
    /// [`events_ordered`]: EventProxy::events_ordered
    /// [`Job::event`]: crate::job::Job::event
    fn event(&mut self, event: Event, delay: u32);
    /// Submits a sequence of events to be executed in order.
    /// 
    /// Because of the [warning] in [`event`], the default implementation
    /// is to insert these events in the reverse order, hence the [`DoubleEndedIterator`]
    /// bound. However, note that the ordering between multiple calls to this function
    /// will result in the latter call's events being executed first.
    /// 
    /// [warning]: EventProxy#orderwarning
    /// [`event`]: EventProxy::event
    fn events_ordered<I>(&mut self, events: I, delay: u32)
    where
        I: IntoIterator<Item = Event>,
        I::IntoIter: DoubleEndedIterator,
    {
        for event in events.into_iter().rev() {
            self.event(event, delay);
        }
    }
    /// Returns an RNG for use in event processing.
    /// 
    /// This RNG may return completely fabricated results, and as such
    /// the output should not be relied upon to have any specific property.
    fn rng(&mut self) -> &mut impl RngCore;
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
    /// The damage of the event.
    pub damage: u64,
    /// The target receiving the damage.
    pub target: ActorId,
}
impl DamageEvent {
    /// Creates a new damage event.
    pub const fn new(damage: u64, target: ActorId) -> Self {
        Self { damage, target }
    }
}
impl From<DamageEvent> for Event {
    fn from(value: DamageEvent) -> Self {
        Event::Damage(value)
    }
}

/// A helper trait for easily submitting damage events on to an event proxy.
pub trait DamageEventExt: EventProxy {
    /// Deals damage to the target after the specified delay.
    fn damage<'a>(
        &mut self,
        actor: &impl Actor<'a>,
        damage: DamageInstance,
        target: ActorId,
        delay: u32,
    ) {
        let damage = actor.attack_damage(damage, target);
        self.event(DamageEvent::new(damage, target).into(), delay)
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
