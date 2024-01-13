//! Interacting with Jobs.
//!
//! This module contains all of the logic for every job in the game.
//! The most important part is the [`check_cast`] and [`cast_snap`] functions
//! on [`Job`]. These functions are how you execute various actions.
//!
//! [`check_cast`]: Job::check_cast
//! [`cast_snap`]: Job::cast_snap

use core::{
    fmt::{self, Debug, Display},
    hash::Hash,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    enums::ActionCategory,
    world::{status::JobEffect, ActorId, Event, EventSink, WorldRef},
};

/// Utilities for role actions.
pub mod role;

/// Logic and types for Bard.
pub mod brd;
/// Logic and types for Dancer.
pub mod dnc;
/// Logic and types for Reaper.
pub mod rpr;
/// Logic and types for Samurai.
pub mod sam;

/// A set of logic for working with jobs in a uniform way.
///
/// This trait exposes everything needed to make a job function
/// in the event loop. It is always implemented on a ZST, and can be
/// considered the "main export" of the various job modules.
///
/// It is not fully intended to be used as an actual trait, but also
/// as an organizational guideline when interacting with specific jobs.
pub trait Job: 'static {
    /// The actions this job can cast.
    type Action: JobAction + 'static;
    /// The job gauge state, action cooldowns, and active combos for this job.
    type State: JobState + 'static;
    /// A custom error for an action that cannot be cast.
    ///
    /// This error should contain things like:
    /// * A gauge cost not being fulfilled.
    /// * An "(Action) Ready" status not being present.
    /// or various other requirements action have to be cast.
    type CastError: Display + Debug + 'static;
    /// A custom event this job can use.
    ///
    /// This will typically be used to schedule things
    /// that need to happen after a set duration, for example
    /// Bard "Repertoire" procs will need to use this.
    ///
    /// This should be `()` for any job that does not need a custom event.
    type Event: Clone + Debug + 'static;
    /// The a set of values associated with each of this job's cooldown groups.
    type CdMap<T>;
    /// The cooldown groups for this job's actions.
    type CdGroup: Copy + Debug + 'static;

    /// Checks that a certain action may be casted, and returns
    /// cooldown information for that action.
    ///
    /// This function should be infallible, and the returned [`CastInitInfo`]
    /// should be on a best-effort basis if errors are encountered.
    fn check_cast<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
        action: Self::Action,
        state: &Self::State,
        world: &'w W,
        event_sink: &mut E,
    ) -> CastInitInfo<Self::CdGroup>;

    /// Executes the specified action.
    ///
    /// This is when the action cast "snapshots". Changes to the job gauge
    /// and combos will happen here, as well as the submission
    /// of [damage] and [status effect] events.
    ///
    /// This function should be infallible, and try to make a best-effort
    /// update in the presence of errors. Gauge changes should be done
    /// in a manner similar to [`u8::saturating_sub`].
    ///
    /// The one exception to this
    /// is if the action must be executed with a target and a valid target cannot
    /// be found. In this case, the an error should be submitted and the function
    /// short-circuited. This is often done through [`need_target!`].
    ///
    /// [damage]: crate::world::DamageEvent
    /// [status effect]: crate::world::status::StatusEvent
    /// [`need_target!`]: crate::need_target
    fn cast_snap<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
        action: Self::Action,
        state: &mut Self::State,
        world: &'w W,
        event_sink: &mut E,
    );

    /// Reacts to an event.
    ///
    /// This function serves two purposes. The first is to be used
    /// in conjunction with a custom [`Event`]. The second is to implement
    /// actions like "Arcane Circle" or the Esprit gauge on Dancer,
    /// which apply some state change in response to the actions
    /// of other players.
    ///
    /// [`Event`]: Job::Event
    #[allow(unused_variables)]
    fn event<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
        state: &mut Self::State,
        world: &'w W,
        event: &Event,
        event_sink: &mut E,
    ) {
        // don't require an impl
    }

    /// Returns a [`JobEffect`] associated with the job, or `None` if the job
    /// has no job effects.
    ///
    /// This should be used to implement things such as the "Army's Paeon" haste on Bard,
    /// the Darkside gauge on Dark Knight, Enochian on Black Mage, etc.
    #[allow(unused_variables)]
    fn effect<'a>(state: &'a Self::State) -> Option<&'a (dyn JobEffect + 'a)> {
        None
    }
}

/// A trait that job actions need to implement.
pub trait JobAction: Copy + Debug + Eq + Hash {
    /// Returns the action category the action belongs to.
    fn category(&self) -> ActionCategory;
    /// Returns true if the action is a GCD.
    fn gcd(&self) -> bool;
}

/// A trait that all job states need to implement.
///
/// While not strictly nescessary, it provides a uniform API
/// to interact with.
pub trait JobState: Clone + Debug + Default {
    /// Advances the state forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    fn advance(&mut self, time: u32);
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A collection of timing information to be applied when an action is cast.
pub struct CastInitInfo<C: 'static> {
    /// The GCD to apply.
    ///
    /// This should be `0` if the action is not a GCD.
    pub gcd: u16,
    /// The cast lock to apply.
    ///
    /// This serves a dual purpose of being the amount of time
    /// it takes to cast an action, as well as animation lock for
    /// instantly cast actions.
    pub lock: u16,
    /// The amount of time before the cast snapshots.
    pub snap: u16,
    /// The MP cost of the action.
    pub mp: u16,
    /// The cooldown for the action to apply.
    ///
    /// The items in this tuple are:
    /// 1. The cooldown group to apply the cooldown to.
    /// 2. The cooldown of the action to be applied.
    /// 3. The maximum number of charges the action can hold.
    ///
    /// Note that the charges is **not** the number of charges to consume.
    /// This value is part of the cooldown to apply.
    pub cd: Option<(C, u32, u8)>,
    /// The alternate cooldown group for the action to apply.
    ///
    /// The items in the tuple are the same as the [`cd`] field.
    /// This field is used for the 1s cooldown between uses of a charged action.
    ///
    /// [`cd`]: CastInitInfo::cd
    pub alt_cd: Option<(C, u32, u8)>,
}

impl<C: 'static> CastInitInfo<C> {
    fn map_cd_group<T>(self, f: impl Fn(C) -> T) -> CastInitInfo<T> {
        CastInitInfo {
            gcd: self.gcd,
            lock: self.lock,
            snap: self.snap,
            mp: self.mp,
            cd: self.cd.map(|v| (f(v.0), v.1, v.2)),
            alt_cd: self.alt_cd.map(|v| (f(v.0), v.1, v.2)),
        }
    }
}

macro_rules! helper {
    (
        $(
            $enum_name:ident $evfn:ident $var_name:ident $job:ty { $job_name:literal }
        )*
    ) => {
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        /// A particular [`Job`].
        pub enum DynJob {
            $(
                /// The job
                #[doc = concat!("\"", $job_name, "\".")]
                $var_name,
            )*
        }
        impl DynJob {
            /// Checks that a certain action may be casted, and returns
            /// cooldown information for that action.
            ///
            /// See [`Job::check_cast`] for more information.
            pub fn check_cast<'w, E: EventSink<'w, W>, W: WorldRef<'w>>(
                &self,
                action: Action,
                state: &State,
                world: &'w W,
                event_sink: &mut E,
            ) -> CastInitInfo<CdGroup> {
                match (self, action, state) {
                    $(
                        (
                            Self::$var_name,
                            Action::$var_name(action),
                            State::$var_name(state),
                        ) => <$job>::check_cast(action, state, world, event_sink)
                                .map_cd_group(CdGroup::$var_name),
                    )*
                    _ => panic!("`action` and `state` do not match job type.")
                }
            }

            /// Executes the specified action.
            ///
            /// See [`Job::cast_snap`] for more information.
            pub fn cast_snap<'w, E: EventSink<'w, W>, W: WorldRef<'w>>(
                &self,
                action: Action,
                state: &mut State,
                world: &'w W,
                event_sink: &mut E,
            ) {
                match (self, action, state) {
                    $(
                        (
                            Self::$var_name,
                            Action::$var_name(action),
                            State::$var_name(state),
                        ) => <$job>::cast_snap(action, state, world, event_sink),
                    )*
                    _ => panic!("`action` and `state` do not match job type.")
                }
            }

            /// Reacts to an event.
            ///
            /// See [`Job::event`] for more information.
            pub fn event<'w, E: EventSink<'w, W>, W: WorldRef<'w>>(
                &self,
                state: &mut State,
                world: &'w W,
                event: &Event,
                event_sink: &mut E,
            ) {
                match (self, state) {
                    $(
                        (
                            Self::$var_name,
                            State::$var_name(state),
                        ) => <$job>::event(state, world, event, event_sink),
                    )*
                    _ => panic!("`state` does not match job type.")
                }
            }

            /// Returns a [`JobEffect`] associated with the job, or `None` if the job
            /// has no job effects.
            ///
            /// See [`Job::effect`] for more information.
            pub fn effect<'a>(&self, state: &'a State) -> Option<&'a (dyn JobEffect + 'a)> {
                match (self, state) {
                    $(
                        (
                            Self::$var_name,
                            State::$var_name(state)
                        ) => <$job>::effect(state),
                    )*
                    _ => panic!("`state` does not match job type.")
                }
            }

            /// Returns the job associated with the enum variant.
            pub fn job(&self) -> $crate::enums::Job {
                match self {
                    $(Self::$var_name => $crate::enums::Job::$enum_name,)*
                }
            }

            /// Returns the enum variant associated with the job.
            ///
            /// Panics if the job type is not yet implemented.
            pub fn from_job(job: $crate::enums::Job) -> Self {
                match job {
                    $($crate::enums::Job::$enum_name => Self::$var_name,)*
                    _ => unimplemented!("Job '{}' not yet implemented.", job),
                }
            }
        }
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
            /// An action for a particular [`Job`].
            enum Action, $(
                /// The action for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::Action, $job, $var_name
            )*
        );
        impl Action {
            /// Returns the [`ActionCategory`] for this action.
            pub fn category(&self) -> ActionCategory {
                match self {
                    $(
                        Self::$var_name(v) => JobAction::category(v),
                    )*
                }
            }
            /// Returns `true` if the action is a GCD.
            pub fn gcd(&self) -> bool {
                match self {
                    $(
                        Self::$var_name(v) => JobAction::gcd(v),
                    )*
                }
            }
        }
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Clone, Debug)]
            /// An error specific to some particular [`Job`].
            enum CastError, $(
                /// The error for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::CastError, $job, $var_name
            )*
        );
        impl fmt::Display for CastError {
            fn fmt(
                &self,
                f: &mut fmt::Formatter<'_>
            ) -> Result<(), fmt::Error> {
                match self {
                    $(
                        Self::$var_name(v) => write!(
                            f,
                            "{}: {}",
                            $crate::enums::Job::$enum_name.name(),
                            v,
                        ),
                    )*
                }
            }
        }
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Clone, Debug, PartialEq, Eq)]
            /// The state of a particular [`Job`].
            enum State, $(
                /// The state for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::State, $job, $var_name
            )*
        );
        impl State {
            /// Advances the state for the job by a certain amount of time.
            pub fn advance(&mut self, time: u32) {
                match self {
                    $(
                        Self::$var_name(v) => v.advance(time),
                    )*
                }
            }
            /// Returns the default state for some specific job.
            pub fn default_for(job: $crate::enums::Job) -> Self {
                match job {
                    $(
                        $crate::enums::Job::$enum_name => Self::$var_name(Default::default()),
                    )*
                    _ => unimplemented!("Job '{}' not yet implemented.", job),
                }
            }
        }
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Copy, Clone, Debug)]
            /// The cooldown groups of a particular [`Job`].
            enum CdGroup, $(
                /// The cooldown group for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::CdGroup, $job, $var_name
            )*
        );

        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[derive(Clone, Debug)]
        /// A map of values to the cooldown groups of a particular [`Job`].
        pub enum CdMap<T> {
            $(
                /// The cooldown map for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $var_name(<$job as $crate::job::Job>::CdMap<T>),
            )*
        }

        impl<T> CdMap<T> {
            /// Returns the job associated with the enum variant.
            pub fn job(&self) -> $crate::enums::Job {
                match self {
                    $(Self::$var_name(_) => $crate::enums::Job::$enum_name,)*
                }
            }
            /// Returns a reference tothe value associated
            /// with the cooldown `group`, or [`None`] if the jobs do not match.
            pub fn get(&self, group: CdGroup) -> Option<&T> {
                Some(match (self, group) {
                    $(
                        (Self::$var_name(v), CdGroup::$var_name(g)) => v.get(g),
                    )*
                    _ => return None
                })
            }
            /// Returns a mutable reference tothe value associated
            /// with the cooldown `group`, or [`None`] if the jobs do not match.
            pub fn get_mut(&mut self, group: CdGroup) -> Option<&mut T> {
                Some(match (self, group) {
                    $(
                        (Self::$var_name(v), CdGroup::$var_name(g)) => v.get_mut(g),
                    )*
                    _ => return None
                })
            }
            /// Returns an iterator over the values in this cooldown map.
            pub fn iter(&self) -> $crate::timing::CdMapIter<'_, T> {
                match self {
                    $(Self::$var_name(v) => v.iter(),)*
                }
            }
            /// Returns a mutable iterator over the values in this cooldown map.
            pub fn iter_mut(&mut self) -> $crate::timing::CdMapIterMut<'_, T> {
                match self {
                    $(Self::$var_name(v) => v.iter_mut(),)*
                }
            }
            /// Returns the default state for some specific job.
            pub fn default_for(job: $crate::enums::Job) -> Self
            where
                T: Default,
            {
                match job {
                    $(
                        $crate::enums::Job::$enum_name => Self::$var_name(Default::default()),
                    )*
                    _ => unimplemented!("Job '{}' not yet implemented.", job),
                }
            }
        }
        $(
            impl<T> From<<$job as $crate::job::Job>::CdMap<T>> for CdMap<T> {
                fn from(val: <$job as $crate::job::Job>::CdMap<T>) -> Self {
                    Self::$var_name(val)
                }
            }
        )*

        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Clone, Debug)]
            /// An event specific to some particular [`Job`].
            enum JobEvent, nofrom, $(
                /// An event for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::Event, $job, $var_name
            )*
        );
        impl JobEvent {
            $(
                /// Creates an [`Event`] from the event for the job
                #[doc = concat!("\"", $job_name, "\".")]
                pub fn $evfn (event: <$job as $crate::job::Job>::Event, actor: ActorId) -> Event {
                    Event::Job(Self::$var_name(event), actor)
                }
            )*
        }
    };
    (
        $(#[$m:meta])*
        enum $whole_name:ident, nofrom, $(
            $(#[$vmeta:meta])*
            $enum_name:ident,
            $job_type:ty,
            $job:ty,
            $var_name:ident
        )*
    ) => {
        $(#[$m])*
        pub enum $whole_name {
            $(
                $(#[$vmeta])*
                $var_name($job_type),
            )*
        }

        impl $whole_name {
            /// Returns the job associated with the enum variant.
            pub fn job(&self) -> $crate::enums::Job {
                match self {
                    $(Self::$var_name(_) => $crate::enums::Job::$enum_name,)*
                }
            }
        }

        #[cfg(feature = "serde")]
        impl $whole_name {
            /// Deserializes a specific variant based on the provided job.
            pub fn deserialize_for<'de, D>(
                job: $crate::enums::Job,
                deserializer: D
            ) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                match job {
                    $(
                        $crate::enums::Job::$enum_name => Ok(Self::$var_name(
                            <$job_type as serde::Deserialize>::deserialize(deserializer)?
                        )),
                    )*
                    _ => Err(<D::Error as serde::de::Error>::custom("Job not yet implemented.")),
                }
            }
        }
    };
    (
        $(#[$m:meta])*
        enum $whole_name:ident, $(
            $(#[$vmeta:meta])*
            $enum_name:ident,
            $job_type:ty,
            $job:ty,
            $var_name:ident
        )*
    ) => {
        helper!(
            $(#[$m])*
            enum $whole_name, nofrom, $($(#[$vmeta])* $enum_name, $job_type, $job, $var_name)*
        );
        $(
            impl From<$job_type> for $whole_name {
                fn from(val: $job_type) -> Self {
                    Self::$var_name(val)
                }
            }
        )*
    };
}

helper! {
    BRD brd Brd brd::BrdJob { "Bard" }
    SAM sam Sam sam::SamJob { "Samurai" }
    DNC dnc Dnc dnc::DncJob { "Dancer" }
    RPR rpr Rpr rpr::RprJob { "Reaper" }
}
