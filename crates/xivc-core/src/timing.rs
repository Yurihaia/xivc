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
/// This will usually get acquired through [`Actor::duration_info`].
///
/// [`StatusEffect`]: crate::world::status::StatusEffect
/// [`Actor::duration_info`]: crate::world::Actor
pub trait DurationInfo {
    /// Returns the cast lock and cast snapshot time for a specific [`ScaleTime`].
    ///
    /// The `lock` parameter is the animation lock if the action is an instant cast.<br>
    /// This function returns a tuple of `(lock, snapshot)`.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::{ScaleTime, DurationInfo};
    /// # fn example(duration_info: &impl DurationInfo) {
    /// // Create a scalable duration
    /// let scale_time = ScaleTime::spell(150);
    /// // Get the scaled duration
    /// let scaled_time = duration_info.get_duration(scale_time) as u16;
    /// // Get the lock and snapshot for scale_time as a cast
    /// let (lock, snapshot) = duration_info.get_cast(scale_time, 600);
    ///
    /// // scale_time is a non-instant cast, so the lock is the cast time + 10ms
    /// assert_eq!(lock, scaled_time + 10);
    /// // scale_time is a non-instant cast, so the snapshot is 50ms before the cast ends
    /// assert_eq!(snapshot, scaled_time - 50);
    /// # }
    /// ```
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
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::{ScaleTime, DurationInfo};
    /// # fn example(duration_info: &impl DurationInfo) {
    /// // create a scaled time for a weaponskill with a recast of 2.50s
    /// let scaled_one = duration_info.get_duration(ScaleTime::skill(2500));
    /// // create a scaled time for a weaponskill with a recast of 5.00s
    /// let scaled_two = duration_info.get_duration(ScaleTime::skill(5000));
    ///
    /// // .get_duration() will always scale uniformly
    /// assert!(scaled_one < scaled_two);
    /// # }
    /// ```
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
    /// Creates a new [`ActionCd`].
    pub const fn new() -> Self {
        Self { cd: 0 }
    }
    /// Applies a cooldown to the cooldown group.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ActionCd;
    /// // Create a new ActionCd.
    /// let mut action_cd = ActionCd::new();
    ///
    /// // The cd and charges for the cooldown group.
    /// let cd = 15000;
    /// let charges = 2;
    ///
    /// // Apply an instance of the cooldown to the ActionCd.
    /// action_cd.apply(cd, charges);
    /// // There are two charges, so one should be left.
    /// assert!(action_cd.available(cd, charges));
    ///
    /// // Apply an instance of the cooldown to the ActionCd again.
    /// action_cd.apply(cd, charges);
    /// // Now there should be no charges left.
    /// assert!(!action_cd.available(cd, charges));
    /// ```
    pub fn apply(&mut self, cd: u32, charges: u8) {
        self.cd = (self.cd + cd).min(cd * charges as u32)
    }
    /// Returns `true` if an action in the cooldown group can be used.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ActionCd;
    /// // Create a new ActionCd.
    /// let mut action_cd = ActionCd::new();
    ///
    /// // The cd and charges for the cooldown group.
    /// let cd = 1000;
    /// let charges = 1;
    ///
    /// // The ActionCd hasnt been put on cooldown yet.
    /// assert!(action_cd.available(cd, charges));
    /// // Apply a cooldown to it now.
    /// action_cd.apply(cd, charges);
    /// // The ActionCd is now on cooldown.
    /// assert!(!action_cd.available(cd, charges));
    ///
    /// // Advance the ActionCd by 1s
    /// action_cd.advance(1000);
    /// // The ActionCd should be off cooldown again.
    /// assert!(action_cd.available(cd, charges));
    /// ```
    pub fn available(&self, cd: u32, charges: u8) -> bool {
        self.cd_until(cd, charges) == 0
    }
    /// Advances the cooldown forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        self.cd = self.cd.saturating_sub(time)
    }
    /// Returns the time until the cooldown group can be used.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ActionCd;
    /// // Create a new ActionCd.
    /// let mut action_cd = ActionCd::new();
    ///
    /// // The cd and charges for the cooldown group.
    /// let cd = 30000;
    /// let charges = 2;
    ///
    /// // Use up both charges
    /// action_cd.apply(cd, charges);
    /// action_cd.apply(cd, charges);
    ///
    /// // Advance the cooldown by 15s,
    /// action_cd.advance(15000);
    ///
    /// // There should now be 15s left until the cd can be used again.
    /// assert_eq!(action_cd.cd_until(cd, charges), 15000);
    /// ```
    pub fn cd_until(&self, cd: u32, charges: u8) -> u32 {
        self.cd.saturating_sub(cd * (charges as u32 - 1))
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
/// each target being hit some multiple of `45ms` after the last . This struct
/// provides a simple interface to start a cascade from a specified delay.
///
/// # Examples
///
/// ```
/// # use xivc_core::timing::EventCascade;
/// # use xivc_core::world::{World,
/// #     Actor,
/// #     EventProxy,
/// #     ActorId,
/// #     Faction,
/// #     DamageEventExt,
/// #     ActionTargetting,
/// #     DamageEvent
/// };
/// # use xivc_core::enums::DamageInstance;
/// # fn example(world: &impl World, event_sink: &mut impl EventProxy) {
/// # let src = world.actor(ActorId(0)).unwrap();
/// # let targets = std::iter::empty(); // doc moment
/// // Create a cascade starting at a delay of 600ms.
/// let mut cascade = EventCascade::new(600, 1);
/// // Iterate over the targets of an action.
/// for target in targets {
///     // Apply damage with the cascading delay to each target.
///     event_sink.damage(src, DamageInstance::new(300).magical(), target, cascade.next());
/// }
/// # }
/// ```
pub struct EventCascade {
    amount: u32,
    time: u32,
}

impl EventCascade {
    /// The number of milliseconds in a base cascade tick.
    ///
    /// Damage is usually `1` tick, while friendly buff
    /// application is usually `0` or `3` ticks.
    pub const TICK: u32 = 45;
    /// Creates a new cascade with the specified starting time and
    /// a cascade amount as a multiple of [`EventCascade::TICK`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use xivc_core::timing::EventCascade;
    /// let mut cascade = EventCascade::new(250, 3);
    ///
    /// assert_eq!(cascade.next(), 250);
    /// assert_eq!(cascade.next(), 385);
    /// ```
    pub const fn new(start: u32, ticks: u32) -> Self {
        Self {
            time: start,
            amount: Self::TICK * ticks,
        }
    }
    /// Creates a new cascade with the specified starting time and
    /// a custom cascaade amount in milliseconds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use xivc_core::timing::EventCascade;
    /// let mut cascade = EventCascade::with_amount(0, 100);
    ///
    /// assert_eq!(cascade.next(), 0);
    /// assert_eq!(cascade.next(), 100);
    /// assert_eq!(cascade.next(), 200);
    /// ```
    pub const fn with_amount(start: u32, amount: u32) -> Self {
        Self {
            time: start,
            amount,
        }
    }

    #[allow(clippy::should_implement_trait)]
    /// Returns the next time the cascade will activate at.
    ///
    /// # Examples
    ///
    /// ```
    /// # use xivc_core::timing::EventCascade;
    /// let mut cascade = EventCascade::new(500, 1);
    ///
    /// let mut last = cascade.next();
    /// for x in 0..20 {
    ///     let next = cascade.next();
    ///     // every call to .next() is always increasing
    ///     assert!(next > last);
    ///     last = next;
    /// }
    /// ```
    pub fn next(&mut self) -> u32 {
        let out = self.time;
        self.time += self.amount;
        out
    }
}

#[derive(Clone, Copy, Debug, Default)]
/// A duration that can be scaled by haste buffs and stats.
pub struct ScaleTime {
    duration: u32,
    stat: Option<SpeedStat>,
    haste: bool,
}

impl ScaleTime {
    /// Returns a [`ScaleTime`] with the specified parameters.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// let scale_time = ScaleTime::new(15000, None, true);
    ///
    /// assert_eq!(scale_time.duration(), 15000);
    /// assert_eq!(scale_time.stat(), None);
    /// assert_eq!(scale_time.haste(), true);
    /// ```
    pub const fn new(duration: u32, stat: Option<SpeedStat>, haste: bool) -> Self {
        Self {
            duration,
            stat,
            haste,
        }
    }
    /// Returns a [`ScaleTime`] with a duration of `0`.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// let scale_time = ScaleTime::zero();
    ///
    /// assert!(scale_time.is_zero());
    /// ```
    pub const fn zero() -> Self {
        Self::new(0, None, false)
    }
    /// Returns a [`ScaleTime`] with the specified duration.
    ///
    /// This scale time will scale off of [`SkillSpeed`] and
    /// will be affected by haste buffs
    ///
    /// # Examples
    ///
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// # use xivc_core::math::SpeedStat;
    /// let scale_time = ScaleTime::skill(130);
    ///
    /// assert_eq!(scale_time.stat(), Some(SpeedStat::SkillSpeed));
    /// assert_eq!(scale_time.haste(), true);
    /// ```
    ///
    /// [`SkillSpeed`]: SpeedStat::SkillSpeed
    pub const fn skill(duration: u32) -> Self {
        Self::new(duration, Some(SpeedStat::SkillSpeed), true)
    }
    /// Returns a [`ScaleTime`] with the specified duration.
    ///
    /// This scale time will scale off of [`SpellSpeed`] and
    /// will be affected by haste buffs
    ///
    /// # Examples
    ///
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// # use xivc_core::math::SpeedStat;
    /// let scale_time = ScaleTime::spell(2500);
    ///
    /// assert_eq!(scale_time.stat(), Some(SpeedStat::SpellSpeed));
    /// assert_eq!(scale_time.haste(), true);
    /// ```
    ///
    /// [`SpellSpeed`]: SpeedStat::SpellSpeed
    pub const fn spell(duration: u32) -> Self {
        Self::new(duration, Some(SpeedStat::SpellSpeed), true)
    }
    /// Returns a [`ScaleTime`] with the specified duration.
    ///
    /// This scale time will not scale off of any stat and
    /// will not be affected by haste buffs.
    ///
    /// # Examples
    ///
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// let scale_time = ScaleTime::none(120000);
    ///
    /// assert_eq!(scale_time.stat(), None);
    /// assert_eq!(scale_time.haste(), false);
    /// ```
    pub const fn none(duration: u32) -> Self {
        Self::new(duration, None, false)
    }
    /// Returns the base duration of this scale time.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// let scale_time = ScaleTime::skill(15000);
    ///
    /// assert_eq!(scale_time.duration(), 15000);
    /// ```
    pub const fn duration(&self) -> u32 {
        self.duration
    }
    /// Returns the [`SpeedStat`] this scale time scales off of.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// # use xivc_core::math::SpeedStat;
    /// let scale_time = ScaleTime::spell(1000);
    ///
    /// assert_eq!(scale_time.stat(), Some(SpeedStat::SpellSpeed));
    /// ```
    pub const fn stat(&self) -> Option<SpeedStat> {
        self.stat
    }
    /// Returns `true` if this scale time can be affected by
    /// haste status effects.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// let scale_time = ScaleTime::skill(5000);
    ///
    /// assert_eq!(scale_time.haste(), true);
    /// ```
    pub const fn haste(&self) -> bool {
        self.haste
    }
    /// Returns `true` if this scale time has a duration of `0`.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::timing::ScaleTime;
    /// let scale_time_zero = ScaleTime::zero();
    /// let scale_time_skill = ScaleTime::skill(0);
    /// let scale_time_none = ScaleTime::none(500);
    ///
    /// assert_eq!(scale_time_zero.is_zero(), true);
    /// assert_eq!(scale_time_skill.is_zero(), true);
    /// assert_eq!(scale_time_none.is_zero(), false);
    /// ```
    pub const fn is_zero(&self) -> bool {
        self.duration == 0
    }
}
