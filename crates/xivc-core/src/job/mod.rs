//! Interacting with Jobs.
//! 
//! This module contains all of the logic for every job in the game.
//! The most important part is the [`check_cast`] and [`cast_snap`] functions
//! on [`Job`]. These functions are how you execute various actions.
//! 
//! [`check_cast`]: Job::check_cast
//! [`cast_snap`]: Job::cast_snap

use core::fmt::{self, Debug, Display};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::world::{status::JobEffect, ActorId, Event, EventProxy, World};

// retain the specific ordering used in game.
#[rustfmt::skip]
/// Logic and types for Samurai.
pub mod sam;
/// Logic and types for Reaper.
pub mod rpr;

/// A set of logic for working with jobs in a uniform way.
///
/// This trait exposes everything needed to make a job function
/// in the event loop. It is always implemented on a ZST, and can be
/// considered the "main export" of the various job modules.
///
/// It is not fully intended to be used as an actual trait, but also
/// as an organizational guideline when interacting with specific jobs.
pub trait Job {
    /// The actions this job can cast.
    type Action: Copy + Debug + 'static;
    /// The job gauge state, action cooldowns, and active combos for this job.
    type State: JobState + 'static;
    /// A custom error for an action that cannot be cast.
    ///
    /// This error should contain things like:
    /// * A gauge cost not being fulfilled.
    /// * An "(Action) Ready" status not being present.
    /// or various other requirements action have to be cast.
    type CastError: Display + Debug;
    /// A custom event this job can use.
    ///
    /// This will typically be used to schedule things
    /// that need to happen after a set duration, for example
    /// Bard "Repertoire" procs will need to use this.
    ///
    /// This should be `()` for any job that does not need a custom event.
    type Event: Clone + Debug;
    /// The cooldown groups for this job's actions.
    type CdGroup: Copy + Debug + 'static;

    /// Checks that a certain action may be casted, and returns
    /// cooldown information for that action.
    ///
    /// This function should be infallible, and the returned [`CastInitInfo`]
    /// should be on a best-effort basis if errors are encountered.
    fn check_cast<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &Self::State,
        world: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    ) -> CastInitInfo<Self::CdGroup>;

    /// Sets the cooldown of a certain cooldown group.
    ///
    /// The final three parameters of this function correspond to the `cd` field
    /// of the [`CastInitInfo`] returned by [`check_cast`]. This function
    /// will typically be implemented with a single function call to the `apply`
    /// function of a [job cooldown struct].
    ///
    /// [`check_cast`]: Job::check_cast
    /// [job cooldown struct]: crate::job_cd_struct!
    fn set_cd(state: &mut Self::State, group: Self::CdGroup, cooldown: u32, charges: u8);

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
    fn cast_snap<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &mut Self::State,
        world: &'w W,
        src: &'w W::Actor<'w>,
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
    fn event<E: EventProxy, W: World>(
        state: &mut Self::State,
        world: &W,
        event: &Event,
        src: Option<ActorId>,
        event_sink: &mut E,
    ) {
        // don't require an impl
    }

    /// Returns a list of [`JobEffect`]s that should be active based
    /// on the job state.
    ///
    /// This should be used to implement things such as the "Army's Paeon" haste on Bard,
    /// the Darkside gauge on Dark Knight, Enochian on Black Mage, etc.
    #[allow(unused_variables)]
    fn effects<'a>(state: &'a Self::State) -> impl AsRef<[&'a dyn JobEffect]> {
        &[]
    }
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
    /// 
    /// This should be `0` for instant cast actions,
    /// and almost always `lock - 60` for cast actions.
    pub snap: u16,
    // i'm like 99% sure there are no actions
    // that don't trigger more than 1 cd group.
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
}

macro_rules! helper {
    (
        $(
            $enum_name:ident $var_name:ident $job:ty { $job_name:literal }
        )*
    ) => {
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Copy, Clone, Debug)]
            /// An action for a particular [`Job`].
            enum Action, $(
                /// The action for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::Action, $job, $var_name
            )*
        );
        helper!(
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
            #[derive(Clone, Debug)]
            /// The state of a particular [`Job`].
            enum State, $(
                /// The state for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::State, $job, $var_name
            )*
        );
        helper!(
            #[derive(Clone, Debug)]
            /// An event specific to some particular [`Job`].
            enum JobEvent, nofrom, $(
                /// An event for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::Event, $job, $var_name
            )*
        );
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
    }
}

helper! {
    SAM Sam sam::SamJob { "Samurai" }
    RPR Rpr rpr::RprJob { "Reaper" }
}
