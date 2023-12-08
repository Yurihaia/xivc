//! Timing utilities.
//! 
//! This module contains various utilities for working
//! with durations and cooldowns.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::math::SpeedStat;

/// A trait that can scale a [`ScaleTime`] based off of
/// the stats of a player and the [`StatusEffect`]s affecting them.
/// 
/// [`StatusEffect`]: crate::world::status::StatusEffect
pub trait DurationInfo {
    /// Returns the cast lock and cast snapshot time for a specific [`ScaleTime`].
    /// 
    /// The `lock` parameter is the animation lock if the action is an instant cast.<br>
    /// This function returns a tuple of `(lock, snapshot)``.
    fn get_cast(&self, base: ScaleTime, lock: u16) -> (u16, u16) {
        let cast = self.get_duration(base) as u16;
        (
            match cast {
                0 => lock + self.extra_ani_lock(),
                v => v + 10,
            },
            match cast {
                0 => 0,
                v => v.saturating_sub(50),
            },
        )
    }
    /// Returns the extra animation delay for instant cast actions.
    fn extra_ani_lock(&self) -> u16;
    /// Returns the scaled duration of some [`ScaleTime`].
    fn get_duration(&self, duration: ScaleTime) -> u64;
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default)]
/// A cooldown for a job cooldown group.
/// 
/// Most of the functions on this struct have a parameter named `charges`.
/// This is **not** the number of charges that are being used up,
/// but the total number of charges the action can hold.
pub struct ActionCd {
    cd: u32,
}

impl ActionCd {
    /// Applies a cooldown to the cooldown group.
    pub fn apply(&mut self, cd: u32, charges: u8) {
        self.cd = (self.cd + cd).min(cd * charges as u32)
    }
    /// Returns `true` if an action in the cooldown group can be used.
    pub fn available(&self, cd: u32, charges: u8) -> bool {
        self.cd <= (cd - 1) * charges as u32
    }
    /// Advances the cooldown forward by a certain amount of time.
    /// 
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        self.cd = self.cd.saturating_sub(time)
    }
    /// Returns the time until the cooldown group can be used.
    pub fn cd_until(&self, cd: u32, charges: u8) -> u32 {
        ((cd - 1) * charges as u32).saturating_sub(self.cd)
    }
}

/// A helper macro to create the cooldown struct and cooldown group enum
/// for a job's actions.
/// 
/// # Examples
/// ```
/// # use xivc_core::job_cd_struct;
/// pub enum ExampleJobActions {
///     Action1,
///     Action2,
///     Action3,
///     Action4,
///     Action5,
///     Action6,
///     Action7,
///     Action8,
/// }
/// 
/// job_cd_struct! {
///     // this first section is the type of the Job's Actions enum.
///     ExampleJobActions =>
///     
///     // this is the definition for the cd struct.
///     // it can have any visibility and attributes
///     // (as well as doc comments) attached to it.
///     #[derive(Clone, Debug, Default)]
///     /// The active cooldowns for Example Job actions.
///     pub ExampleJobCds
/// 
///     // this is the definition for the cd group enum.
///     // like the cd struct, it can have a custom visibility
///     // and attributes.
///     #[derive(Copy, Clone, Debug)]
///     /// The various cooldown groups an Example Job action can be part of.
///     pub ExampleJobCdGroup
///     
///     // each of these next lines is the definition of a cooldown group
///     "Action1, Action2, and Action3" // this string literal should be the description
///                                     // of the cooldown group's actions.
///                                     // It is used for documentation
///     // these next two identifiers are the names
///     // of the cd struct field and cd group enum variant respectively
///     cd_group_a CdGroupA:
///     // After that comes a list of the Actions that are part of the cd group.
///     Action1 Action2 Action3;
///     // often if actions are named similar things, you can put A/B in the
///     // group description. For example, "Lemure's Slice/Scythe".
///     "Action4/6"
///     cd_group_b CdGroupB: Action4 Action6;
/// }
/// ```
/// 
/// For an example of what this macro will generate, see [`RprCds`] and [`RprCdGroup`].
/// 
/// [`RprCds`]: crate::job::rpr::RprCds
/// [`RprCdGroup`]: crate::job::rpr::RprCdGroup
#[macro_export]
macro_rules! job_cd_struct {
    (
        $acty:ty =>

        $(#[$cds_meta:meta])*
        $cds_vis:vis $cds_id:ident

        $(#[$cdg_meta:meta])*
        $cdg_vis:vis $cdg_id:ident

        $(
            $cd_names:literal
            $(#[$cdsf_meta:meta])*
            $cdsf_name:ident
            $(#[$cdgv_meta:meta])*
            $cdgv_name:ident: $($aci:ident)+;
        )*
    ) => {
        $(#[$cds_meta])*
        $cds_vis struct $cds_id {
            $(
                $(#[$cdsf_meta])*
                /// The cooldown of
                #[doc = concat!($cd_names, '.')]
                $cdsf_name: $crate::timing::ActionCd,
            )*
        }

        $(#[$cdg_meta])*
        $cdg_vis enum $cdg_id {
            $(
                $(#[$cdgv_meta])*
                /// The cooldown group for
                #[doc = concat!($cd_names, '.')]
                $cdgv_name,
            )*
        }

        impl $cds_id {
            /// Applies a cooldown to the specified [cooldown group].
            /// 
            /// [cooldown group]:
            #[doc = stringify!($cdg_id)]
            pub fn apply(&mut self, cdg: $cdg_id, cooldown: u32, charges: u8) {
                match cdg {
                    $(
                        $cdg_id::$cdgv_name => self.$cdsf_name.apply(cooldown, charges),
                    )*
                }
            }
            
            /// Checks if the specified [cooldown group] is available.
            /// 
            /// [cooldown group]:
            #[doc = stringify!($cdg_id)]
            pub fn available(&self, cdg: $cdg_id, cooldown: u32, charges: u8) -> bool {
                match cdg {
                    $(
                        $cdg_id::$cdgv_name => self.$cdsf_name.available(cooldown, charges),
                    )*
                }
            }
            
            /// Advances the cooldowns forward by a certain amount of time.
            /// 
            /// See TODO: Advance Functions for more information.
            pub fn advance(&mut self, time: u32) {
                $(self.$cdsf_name.advance(time);)*
            }
            
            /// Gets the cooldown until the specified [cooldown group] can be used.
            /// 
            /// [cooldown group]:
            #[doc = stringify!($cdg_id)]
            pub fn cd_until(&self, group: $cdg_id, cooldown: u32, charges: u8) -> u32 {
                match group {
                    $(
                        $cdg_id::$cdgv_name => self.$cdsf_name.cd_until(cooldown, charges),
                    )*
                }
            }
        }

        impl $acty {
            /// Gets the [cooldown group] that this action is part of.
            /// 
            /// Returns `None` if this action does not have a cooldown.
            /// 
            /// [cooldown group]:
            #[doc = stringify!($cdg_id)]
            pub fn cd_group(self) -> Option<$cdg_id> {
                Some(match self {
                    $(
                        $(Self::$aci)|+ => $cdg_id::$cdgv_name,
                    )*
                    _ => return None
                })
            }
        }
    };
}

/// A utility for effect cascading.
/// 
/// In FFXIV, most effects "cascade" when they hit multiple targets,
/// each target being hit 1/30th of a second after the last. This struct
/// provides a simple interface to start a cascade from a specified delay.
pub struct EventCascade {
    amount: u32,
    time: u32,
}

impl EventCascade {
    /// Creates a new cascade with the specified starting time and the
    /// default (`133ms`) cascade amount.
    pub const fn new(start: u32) -> Self {
        Self {
            time: start,
            amount: 133,
        }
    }
    /// Creates a new cascade with the specified starting time and 
    /// a custom cascaade amount in milliseconds.
    pub const fn with_amount(start: u32, amount: u32) -> Self {
        Self {
            time: start,
            amount,
        }
    }

    #[allow(clippy::should_implement_trait)]
    /// Returns the next time the cascade will activate at.
    pub fn next(&mut self) -> u32 {
        let out = self.time;
        self.time += self.amount;
        out
    }
}

#[derive(Clone, Copy, Debug, Default)]
/// A duration that can be scaled by haste buffs and stats.
pub struct ScaleTime(pub u32, pub Option<SpeedStat>, pub bool);

impl ScaleTime {
    /// Returns a [`ScaleTime`] with a duration of `0`.
    pub const fn zero() -> Self {
        Self(0, None, false)
    }
    /// Returns a [`ScaleTime`] with the specified duration.
    /// 
    /// This scale time will scale off of [`SkillSpeed`] and
    /// will be affected by haste buffs
    /// 
    /// [`SkillSpeed`]: SpeedStat::SkillSpeed
    pub const fn skill(duration: u32) -> Self {
        Self(duration, Some(SpeedStat::SkillSpeed), true)
    }
    /// Returns a [`ScaleTime`] with the specified duration.
    /// 
    /// This scale time will scale off of [`SpellSpeed`] and
    /// will be affected by haste buffs
    /// 
    /// [`SpellSpeed`]: SpeedStat::SpellSpeed
    pub const fn spell(duration: u32) -> Self {
        Self(duration, Some(SpeedStat::SpellSpeed), true)
    }
    /// Returns a [`ScaleTime`] with the specified duration.
    /// 
    /// This scale time will not scale off of any stat and
    /// will not be affected by haste buffs.
    pub const fn none(duration: u32) -> Self {
        Self(duration, None, false)
    }
    /// Returns the base duration of this scale time.
    pub const fn duration(&self) -> u32 {
        self.0
    }
    /// Returns the [`SpeedStat`] this scale time scales off of.
    pub const fn stat(&self) -> Option<SpeedStat> {
        self.1
    }
    /// Returns `true` if this scale time can be affected by
    /// haste status effects.
    pub const fn haste(&self) -> bool {
        self.2
    }
    /// Returns `true` if this scale time has a duration of `0`.
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
}
