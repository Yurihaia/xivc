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
    ///
    type Action: JobAction + 'static;
    type State: JobState + 'static;
    type Error: Display + Debug;
    type Event: Clone + Debug;
    type CdGroup: Copy + Debug + 'static;

    fn check_cast<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &Self::State,
        world: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    ) -> CastInitInfo<Self::CdGroup>;

    fn set_cd(state: &mut Self::State, group: Self::CdGroup, cooldown: u32, charges: u8);

    // fn init_cast<'w, E: EventProxy, W: World>(
    //     action: Self::Action,
    //     state: &mut Self::State,
    //     world: &'w W,
    //     src: &'w W::Actor<'w>,
    //     event_sink: &mut E,
    // );

    fn cast_snap<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &mut Self::State,
        world: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    );

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

    #[allow(unused_variables)]
    fn effects<'a>(state: &Self::State) -> &'a [&'a dyn JobEffect] {
        &[]
    }
}

pub trait JobAction: Copy + Debug {}

pub trait JobState: Clone + Debug + Default {
    fn advance(&mut self, time: u32);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastInitInfo<C: 'static> {
    pub gcd: u16,
    pub lock: u16,
    pub snap: u16,
    // i'm like 99% sure there are no actions
    // that don't trigger more than 1 cd group.
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
            enum Error, $(
                /// The error for the job
                #[doc = concat!("\"", $job_name, "\".")]
                $enum_name, <$job as $crate::job::Job>::Error, $job, $var_name
            )*
        );
        impl fmt::Display for Error {
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
