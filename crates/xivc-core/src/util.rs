//! Various utility types and functions.

use core::{
    any::TypeId,
    cmp, fmt,
    iter::{FusedIterator, Map},
    mem, ops,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    err,
    timing::EventCascade,
    world::{status::StatusEffect, ActionTargetting, ActorId, ActorRef, EventError, Faction},
};

/// A utility function that returns the potency of an action
/// depending on if it was comboed into and if it hit it's positional.
pub const fn combo_pos_pot(
    base: u64,
    if_pos: u64,
    if_combo: u64,
    if_both: u64,
    combo: bool,
    pos: bool,
) -> u64 {
    match (combo, pos) {
        (false, false) => base,
        (false, true) => if_pos,
        (true, false) => if_combo,
        (true, true) => if_both,
    }
}

/// A utility function that returns the potency of an action
/// depending on if it was comboed into.
pub const fn combo_pot(base: u64, if_combo: u64, combo: bool) -> u64 {
    if combo {
        if_combo
    } else {
        base
    }
}

/// A utility function that returns the potency of an action
/// depending on if it hit it's positional.
pub const fn pos_pot(base: u64, if_pos: u64, pos: bool) -> u64 {
    if pos {
        if_pos
    } else {
        base
    }
}

// i don't think there will ever be any gauge that needs a u16+
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// A struct that that is used to keep track off
/// various job gauges in the game.
///
/// This struct implements assignment operations like
/// [`AddAssign`] and [`SubAssign`], which will saturate
/// at the bounds this gauge can hold.
///
/// It also implements various trait like [`PartialEq<u8>`],
/// [`PartialOrd<u8>`], and [`Deref<Target = u8>`][deref],
/// allowing transparent access to the value inside.
///
/// # Examples
/// ```
/// # use xivc_core::util::GaugeU8;
/// // Create a new gauge with a maximum value of 25.
/// let mut gauge = GaugeU8::<25>::new();
///
/// // Increase the gauge by 10.
/// gauge += 10;
/// assert_eq!(gauge, 10);
///
/// // Increase the gauge by 10 again.
/// gauge += 10;
/// assert_eq!(gauge, 20);
///
/// // Increase the gauge by 10 a third time.
/// gauge += 10;
/// // However this one will saturate at the upper bound
/// assert_eq!(gauge, 25);
///
/// // Decrease the gauge by 100. This will saturate to 0.
/// gauge -= 100;
/// assert_eq!(gauge, 0);
///
/// // Increase the gauge by 5, then double it.
/// gauge += 5;
/// gauge *= 2;
/// assert_eq!(gauge, 10);
/// ```
///
/// [deref]: ops::Deref
/// [`AddAssign`]: ops::AddAssign
/// [`SubAssign`]: ops::SubAssign
pub struct GaugeU8<const MAX: u8>(u8);

impl<const MAX: u8> GaugeU8<MAX> {
    /// Creates a new gauge, with its value set to `0`.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 100.
    /// let gauge = GaugeU8::<100>::new();
    /// // The value of the newly created gauge should be 0.
    /// assert_eq!(gauge, 0);
    /// ```
    pub const fn new() -> Self {
        Self(0)
    }
    /// Sets the gauge to a value.
    ///
    /// # Panics
    /// This function panics if the value the gauge is set to
    /// exceeds the maximum value the gauge can be.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 50.
    /// let mut gauge = GaugeU8::<50>::new();
    /// // Set the gauge to 25.
    /// gauge.set(25);
    ///
    /// assert_eq!(gauge, 25);
    /// ```
    ///
    /// ```should_panic
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 50.
    /// let mut gauge = GaugeU8::<50>::new();
    /// // Set the gauge to 75. This is greater than
    /// // the maximum so this function will panic.
    /// gauge.set(75);
    /// ```
    pub fn set(&mut self, value: u8) {
        if value > MAX {
            panic!()
        }
        self.0 = value;
    }
    /// Sets the gauge to a value, regardless of
    /// if it is larger than the maximum value or not.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 50.
    /// let mut gauge = GaugeU8::<50>::new();
    /// // Set the gauge to 75. This is greater than
    /// // the maximum but this function will never panic.
    /// gauge.set_unchecked(75);
    ///
    /// assert_eq!(gauge, 75);
    /// ```
    pub fn set_unchecked(&mut self, value: u8) {
        self.0 = value;
    }
    /// Sets the gauge to a value, regardless of
    /// if it is larger than the maximum value or not.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 50.
    /// let mut gauge = GaugeU8::<50>::new();
    /// // Set the gauge to 75. This is greater than
    /// // the maximum but this function
    /// // will saturate to the maximum value.
    /// gauge.set_saturating(75);
    ///
    /// assert_eq!(gauge, 50);
    /// ```
    pub fn set_saturating(&mut self, value: u8) {
        self.0 = value.min(MAX);
    }
    /// Sets the gauge to the maximum value it can hold.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 5.
    /// let mut gauge = GaugeU8::<5>::new();
    /// // Set the gauge to the maximum value it can hold.
    /// gauge.set_max();
    ///
    /// assert_eq!(gauge, 5);
    /// ```
    pub fn set_max(&mut self) {
        self.0 = MAX;
    }
    /// Returns the maximum value the gauge can hold.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 20.
    /// let mut gauge = GaugeU8::<20>::new();
    ///
    /// assert_eq!(gauge.max(), 20);
    /// ```
    pub fn max(&self) -> u8 {
        MAX
    }
    /// Resets the gauge down to zero.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 100.
    /// let mut gauge = GaugeU8::<100>::new();
    /// // Increase the gauge by 75.
    /// gauge += 75;
    /// // Clear the gauge.
    /// gauge.clear();
    ///
    /// assert_eq!(gauge, 0);
    /// ```
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// Returns the value held in this gauge.
    ///
    /// Note that the gauge also implements [`Deref`],
    /// and that should be preferred unless
    /// required in a const context.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 100.
    /// let mut gauge = GaugeU8::<100>::new();
    /// // Set the gauge to 30.
    /// gauge.set(30);
    /// // Increase the gauge by 20.
    /// gauge += 20;
    ///
    /// // It should now hold 50.
    /// assert_eq!(gauge.value(), 50);
    /// // gauge.value() returns the same thing
    /// // as the Deref implementation.
    /// assert_eq!(gauge.value(), *gauge);
    /// ```
    ///
    /// [`Deref`]: ops::Deref
    pub const fn value(&self) -> u8 {
        self.0
    }
    
    /// Attempts to consume the `amount` specified and returns `true` if there was
    /// enough gauge.
    /// 
    /// # Examples
    /// ```
    /// # use xivc_core::util::GaugeU8;
    /// // Create a new gauge with a maximum value of 100.
    /// let mut gauge = GaugeU8::<100>::new();
    /// // Increase the gauge by 25.
    /// gauge += 25;
    /// 
    /// // Consume will not decrease the gauge if there isn't enough.
    /// assert!(!gauge.consume(50));
    /// assert_eq!(gauge, 25);
    /// 
    /// gauge += 25;
    /// 
    /// // But it will once there is enough.
    /// assert!(gauge.consume(50));
    /// assert_eq!(gauge, 0);
    /// ```
    pub fn consume(&mut self, amount: u8) -> bool {
        if *self >= amount {
            *self -= amount;
            true
        } else {
            false
        }
    }
}

impl<const MAX: u8> ops::AddAssign<u8> for GaugeU8<MAX> {
    fn add_assign(&mut self, rhs: u8) {
        self.0 = (self.0 + rhs).min(MAX);
    }
}
impl<const MAX: u8> ops::SubAssign<u8> for GaugeU8<MAX> {
    fn sub_assign(&mut self, rhs: u8) {
        self.0 = self.0.saturating_sub(rhs);
    }
}
impl<const MAX: u8> ops::MulAssign<u8> for GaugeU8<MAX> {
    fn mul_assign(&mut self, rhs: u8) {
        self.0 = self.0.saturating_mul(rhs);
    }
}
impl<const MAX: u8> ops::DivAssign<u8> for GaugeU8<MAX> {
    fn div_assign(&mut self, rhs: u8) {
        self.0 = self.0.saturating_div(rhs);
    }
}
impl<const MAX: u8> cmp::PartialEq<u8> for GaugeU8<MAX> {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}
impl<const MAX: u8> cmp::PartialEq<GaugeU8<MAX>> for u8 {
    fn eq(&self, other: &GaugeU8<MAX>) -> bool {
        *self == other.0
    }
}
impl<const MAX: u8> cmp::PartialOrd<u8> for GaugeU8<MAX> {
    fn partial_cmp(&self, other: &u8) -> Option<cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<const MAX: u8> cmp::PartialOrd<GaugeU8<MAX>> for u8 {
    fn partial_cmp(&self, other: &GaugeU8<MAX>) -> Option<cmp::Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<const MAX: u8> ops::Deref for GaugeU8<MAX> {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// #[macro_export]
// /// A utility macro to submit an error and then short circuit
// /// from a [`cast_snap`] function if the player does not have a target.
// ///
// /// The first parameter is the expression to evaluate the target from.
// /// This is an `Option<T>` by default, or an iterator in the case of `aoe` or `uaoe`.
// ///
// /// The second parameter should be a mutable reference to the event sink
// /// that the error should be submitted to.
// ///
// /// Finally, an optional keyword of `aoe` or `uaoe` can be specified,
// /// which changes the behavior of the macro.
// /// * None: Used for single target.
// /// * `aoe`: Used for an aoe that needs to separate
// ///     the primary target from the secondary targets.
// /// * `uaoe`: Used for an aoe that does not need to separate
// ///     the primary target from the secondary targets,
// ///     but still requires a target to be cast.
// ///
// /// # Examples
// ///
// /// ```
// /// # use xivc_core::world::{
// /// #     WorldRef,
// /// #     ActorId,
// /// #     EventSink,
// /// #     ActionTargetting,
// /// #     Faction,
// /// #     DamageEventExt,
// /// #     ActorRef,
// /// # };
// /// # use xivc_core::job::brd::BrdAction;
// /// # use xivc_core::timing::{EventCascade};
// /// # use xivc_core::need_target;
// /// # use xivc_core::enums::DamageInstance;
// /// # fn example<'w, W: WorldRef<'w>>(world: &'w W, event_sink: &mut impl EventSink<'w, W>, action: BrdAction) {
// /// # let src = world.actor(ActorId(0)).unwrap();
// /// // Constants like these are recommended to reduce boilerplate.
// /// const TARGET_CIRCLE: ActionTargetting = ActionTargetting::target_circle(5, 25);
// /// const MELEE: ActionTargetting = ActionTargetting::single(3);
// ///
// /// // A closure like this is also recommended to reduce boilerplate.
// /// let target_enemy = |t: ActionTargetting| {
// ///     src.actors_for_action(Some(Faction::Enemy), t).map(|a| a.id())
// /// };
// ///
// /// // Deal damage to targets in a circle with a radius of 5y and a range of 25y.
// /// // This aoe will have damage falloff.
// /// let (first, other) = need_target!(target_enemy(TARGET_CIRCLE), event_sink, aoe);
// /// let mut cascade = EventCascade::new(600, 1);
// /// event_sink.damage(action, DamageInstance::new(1000).slashing(), first, cascade.next());
// /// for target in other {
// ///     event_sink.damage(action, DamageInstance::new(500).slashing(), target, cascade.next());
// /// }
// ///
// /// // Deal damage to a single target within a range of 3y.
// /// let target = need_target!(target_enemy(MELEE).next(), event_sink);
// /// event_sink.damage(action, DamageInstance::new(350).slashing(), target, 400);
// /// # }
// /// ```
// ///
// /// [`cast_snap`]: crate::job::Job::cast_snap
// macro_rules! need_target {
//     ($t:expr, $p:expr) => {{
//         let Some(v) = $t else {
//             $p.error($crate::world::EventError::NoTarget);
//             return;
//         };
//         v
//     }};
//     ($t:expr, $p:expr, aoe) => {{
//         let mut i = $t;
//         let Some(first) = i.next() else {
//             $p.error($crate::world::EventError::NoTarget);
//             return;
//         };
//         (first, i)
//     }};
//     ($t:expr, $p:expr, uaoe) => {{
//         let mut i = ($t).peekable();
//         if i.peek().is_none() {
//             $p.error($crate::world::EventError::NoTarget);
//             return;
//         }
//         i
//     }};
// }

/// Utility methods for [`ActorRef`] to help targetting actors.
pub trait ActionTargettingExt<'w>: ActorRef<'w> {
    /// Targets a single enemy.
    /// 
    /// If there is no target, or the target is out of range, returns [`EventError::NoTarget`].
    fn target_enemy(&self, targetting: ActionTargetting) -> Result<Self, EventError> {
        self.actors_for_action(Some(Faction::Enemy), targetting)
            .next()
            .ok_or(EventError::NoTarget)
    }

    /// Targets multiple enemies and links in a delay cascade.
    /// 
    /// If the specified `targetting` doesn't require a target (for example, [`Circle`]), this
    /// will never be [`Err`].
    /// Otherwise, if there is no target, or the target is out of range, returns [`EventError::NoTarget`].
    /// 
    /// [`Circle`]: ActionTargetting::Circle
    fn target_enemy_aoe(
        &self,
        targetting: ActionTargetting,
        cascade: EventCascade,
    ) -> Result<AoeIter<impl Iterator<Item = Self> + 'w>, EventError> {
        if let Some(range) = targetting.requires_target() {
            let target = self.target().ok_or(EventError::NoTarget)?;
            if !self.within_range(target.id(), ActionTargetting::Single { range }) {
                err!(EventError::NoTarget);
            }
            if target.faction() != Faction::Enemy {
                err!(EventError::NoTarget);
            }
        }
        let inner = self.actors_for_action(Some(Faction::Enemy), targetting);
        Ok(AoeIter { inner, cascade })
    }

    /// Targets a single party member.
    /// 
    /// If there is no target, or the target is out of range, returns [`EventError::NoTarget`].
    fn target_party(&self, targetting: ActionTargetting) -> Result<Self, EventError> {
        self.actors_for_action(Some(Faction::Party), targetting)
            .next()
            .ok_or(EventError::NoTarget)
    }

    /// Targets multiple party members and links in a delay cascade.
    /// 
    /// If the specified `targetting` doesn't require a target (for example, [`Circle`]), this
    /// will never be [`Err`].
    /// Otherwise, if there is no target, or the target is out of range, returns [`EventError::NoTarget`].
    /// 
    /// [`Circle`]: ActionTargetting::Circle
    fn target_party_aoe(
        &self,
        targetting: ActionTargetting,
        cascade: EventCascade,
    ) -> Result<AoeIter<impl Iterator<Item = Self> + 'w>, EventError> {
        if let Some(range) = targetting.requires_target() {
            let target = self.target().ok_or(EventError::NoTarget)?;
            if !self.within_range(target.id(), ActionTargetting::Single { range }) {
                return Err(EventError::NoTarget);
            }
            if target.faction() != Faction::Party {
                return Err(EventError::NoTarget);
            }
        }
        let inner = self.actors_for_action(Some(Faction::Party), targetting);
        Ok(AoeIter { inner, cascade })
    }
}

impl<'w, A: ActorRef<'w>> ActionTargettingExt<'w> for A {}

/// An iterator that returns a list of actors along with a delay cascade.
/// 
/// See [`target_enemy_aoe`] or [`target_party_aoe`] for more information.
/// 
/// [`target_enemy_aoe`]: ActionTargettingExt::target_enemy_aoe
/// [`target_party_aoe`]: ActionTargettingExt::target_party_aoe
#[derive(Clone, Debug)]
pub struct AoeIter<I> {
    inner: I,
    cascade: EventCascade,
}

impl<I> AoeIter<I> {
    /// Adds damage falloff to the iterator.
    /// 
    /// The first value returned will have a falloff of `100`, and every one after that will have a falloff
    /// matching the specified value.
    pub fn falloff(self, falloff: u8) -> AoeFalloffIter<I> {
        AoeFalloffIter {
            inner: self,
            first: true,
            falloff,
        }
    }
}

impl<'w, A: ActorRef<'w>, I: Iterator<Item = A>> AoeIter<I> {
    /// A convenience method that maps an [`ActorRef`] to its [`ActorId`].
    pub fn id(self) -> AoeIter<Map<I, impl FnMut(A) -> ActorId>> {
        AoeIter {
            inner: self.inner.map(|v| v.id()),
            cascade: self.cascade,
        }
    }
}

impl<I: Iterator> Iterator for AoeIter<I> {
    type Item = (I::Item, u32);

    fn next(&mut self) -> Option<Self::Item> {
        Some((self.inner.next()?, self.cascade.next()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for AoeIter<I> {}
impl<I: FusedIterator> FusedIterator for AoeIter<I> {}

/// An iterator that returns a list of actors along with a delay cascade.
/// 
/// See [`falloff`] for more information.
/// 
/// [`falloff`]: AoeIter::falloff
#[derive(Clone, Debug)]
pub struct AoeFalloffIter<I> {
    inner: AoeIter<I>,
    first: bool,
    falloff: u8,
}

impl<'w, A: ActorRef<'w>, I: Iterator<Item = A>> AoeFalloffIter<I> {
    /// A convenience method that maps an [`ActorRef`] to its [`ActorId`].
    pub fn id(self) -> AoeFalloffIter<Map<I, impl FnMut(A) -> ActorId>> {
        AoeFalloffIter {
            inner: self.inner.id(),
            first: self.first,
            falloff: self.falloff,
        }
    }
}

impl<I: Iterator> Iterator for AoeFalloffIter<I> {
    type Item = (I::Item, u32, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let (item, delay) = self.inner.next()?;
        let falloff = if mem::replace(&mut self.first, false) {
            100
        } else {
            self.falloff
        };
        Some((item, delay, falloff))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for AoeFalloffIter<I> {}
impl<I: FusedIterator> FusedIterator for AoeFalloffIter<I> {}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
/// The state of an action combo.
///
/// # Examples
/// ```
/// # use xivc_core::util::ComboState;
/// // An enum that represents the state a combo can be in.
/// // Each variant is the action that satisfies
/// // The tooltip section "Combo Action: <Action>".
/// // the combo enum must implement Eq for ComboState::check to work.
/// #[derive(PartialEq, Eq)]
/// pub enum MainCombo {
///     Action1,
///     Action2,
///     Action3,
/// }
/// // Create a new combo state for the main combo.
/// let mut combo_state = ComboState::<MainCombo>::new();
///
/// // Sets the combo state to Action1.
/// combo_state.set(MainCombo::Action1);
///
/// assert!(combo_state.check(MainCombo::Action1));
///
/// // Resets the combo state.
/// combo_state.reset();
///
/// assert!(!combo_state.check(MainCombo::Action1));
///
/// // Sets the combo state to Action2, then advances
/// // the combo by 30s. This will cause the combo to break.
/// combo_state.set(MainCombo::Action2);
/// combo_state.advance(30000);
///
/// assert!(!combo_state.check(MainCombo::Action2));
/// ```
pub struct ComboState<C> {
    pub combo: Option<(C, u32)>,
}

impl<C> ComboState<C> {
    /// Creates a new combo state.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::ComboState;
    /// # #[derive(PartialEq, Eq)]
    /// # pub enum MainCombo {
    /// #     Action1,
    /// #     Action2,
    /// #     Action3,
    /// # }
    /// // Create the combo state for the main combo.
    /// let combo_state = ComboState::<MainCombo>::new();
    ///
    /// assert!(!combo_state.check(MainCombo::Action1));
    /// ```
    pub const fn new() -> Self {
        Self { combo: None }
    }
    /// Resets the combo.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::ComboState;
    /// # #[derive(PartialEq, Eq)]
    /// # pub enum MainCombo {
    /// #     Action1,
    /// #     Action2,
    /// #     Action3,
    /// # }
    /// // Create the combo state for the main combo.
    /// let mut combo_state = ComboState::<MainCombo>::new();
    ///
    /// // Set the combo state to Action2, then reset it.
    /// combo_state.set(MainCombo::Action2);
    /// combo_state.reset();
    ///
    /// assert!(!combo_state.check(MainCombo::Action2));
    /// ```
    pub fn reset(&mut self) {
        self.combo = None;
    }
    /// Sets the combo to the specified combo stage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::ComboState;
    /// # #[derive(PartialEq, Eq)]
    /// # pub enum MainCombo {
    /// #     Action1,
    /// #     Action2,
    /// #     Action3,
    /// # }
    /// // Create the combo state for the main combo.
    /// let mut combo_state = ComboState::<MainCombo>::new();
    ///
    /// // Set the combo state to Action3.
    /// combo_state.set(MainCombo::Action3);
    ///
    /// assert!(combo_state.check(MainCombo::Action3));
    /// ```
    pub fn set(&mut self, combo: C) {
        self.combo = Some((combo, 30000));
    }
    /// Checks if the combo is at the specified combo stage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::util::ComboState;
    /// # #[derive(PartialEq, Eq)]
    /// # pub enum MainCombo {
    /// #     Action1,
    /// #     Action2,
    /// #     Action3,
    /// # }
    /// // Create the combo state for the main combo.
    /// let mut combo_state = ComboState::<MainCombo>::new();
    ///
    /// // Set the combo state to Action1.
    /// combo_state.set(MainCombo::Action1);
    ///
    /// assert!(combo_state.check(MainCombo::Action1));
    /// assert!(!combo_state.check(MainCombo::Action2));
    /// assert!(!combo_state.check(MainCombo::Action3));
    /// ```
    pub fn check(&self, combo: C) -> bool
    where
        C: Eq,
    {
        self.combo
            .as_ref()
            .map(|(c, _)| c == &combo)
            .unwrap_or_default()
    }

    /// Advances the combos forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        if let Some((_, t)) = &mut self.combo {
            let time = t.saturating_sub(time);
            if time == 0 {
                self.combo = None;
            } else {
                *t = time;
            }
        }
    }
}

impl<C> Default for ComboState<C> {
    fn default() -> Self {
        Self { combo: None }
    }
}

#[macro_export]
/// Defines a new bool RNG distribution wrapper.
///
/// This is generally used to let simulation implementations have control
/// over specific instances of rng. For example, a simulation may want to
/// force a specific Thunder 3 tick to get Thundercloud, or make a Reverse Cascade
/// gcd guaranteed to not give a feather.
///
/// The granularity is up to interpretation, but ideally should at least be split
/// between different effects. For example, don't combine the Silken Symmetry
/// proc from Cascade and the Fourfold Feather from Reverse Cascade, despite them
/// sharing the same 50% chance.
///
/// # Examples
/// ```
/// # use xivc_core::bool_job_dist;
/// bool_job_dist! {
///     /// The 35% chance for a Straight Shot Ready proc.
///     pub StraightShotReady = 35 / 100;
///     /// The 10% chance for a Thundercloud proc.
///     pub Thundercloud = 1 / 10;
///     /// The 50% chance to get a Fourfold Feather.
///     pub FourfoldFeather = 1 / 2;
/// }
/// ```
macro_rules! bool_job_dist {
    (
        $(
            $(#[$m:meta])*
            $v:vis $id:ident = $n:literal / $d:literal;
        )*
    ) => {
        $(
            $(#[$m])*
            $v struct $id;

            impl rand::distributions::Distribution<bool> for $id {
                fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> bool {
                    rng.gen_ratio($n, $d)
                }
            }
        )*
    };
}

/// "Converts" a value into another type, as long as both types are the same.
/// If the types do not match, returns an [`Err`] containing the original value.
///
/// This is essentially a by value version of [`Any::downcast_ref`]. It mainly
/// is useful for [`EventSink::random`] implementations to return specific
/// values for the different calls to it.
///
/// [`Any::downcast_ref`]: core::any::Any#method.downcast_ref
/// [`EventSink::random`]: crate::world::EventSink::random
pub fn convert<S, D>(s: S) -> Result<D, S>
where
    S: 'static,
    D: 'static,
{
    if TypeId::of::<S>() == TypeId::of::<D>() {
        // Safety
        // S and D are the same type because their TypeIds match.
        // ManuallyDrop has the same layout as its contents,
        // So ManuallyDrop<S> can be transmuted to S.
        // `s` will not get double dropped because it is inside a ManuallyDrop.
        let out = mem::ManuallyDrop::new(s);
        Ok(unsafe { mem::transmute_copy(&out) })
    } else {
        Err(s)
    }
}

/// Converts a reference into another type, as long as both types are the same.
/// If the types do not match, returns [`None`].
///
/// This is essentially a version of [`Any::downcast_ref`] that works without
/// trait objects or unsizing. It mainly is useful for [`EventSink::random`]
/// implementations to return specific values for the different calls to it.
///
/// [`Any::downcast_ref`]: core::any::Any#method.downcast_ref
/// [`EventSink::random`]: crate::world::EventSink::random
pub fn convert_ref<S, D>(s: &S) -> Option<&D>
where
    S: 'static,
    D: 'static,
{
    if TypeId::of::<S>() == TypeId::of::<D>() {
        // Safety
        // S and D are the same type because their TypeIds match.
        Some(unsafe { &*(s as *const S as *const D) })
    } else {
        None
    }
}

/// Converts a mutable reference into another type, as long as both types are the same.
/// If the types do not match, returns [`None`].
///
/// This is essentially a version of [`Any::downcast_ref`] that works without
/// trait objects or unsizing. It mainly is useful for [`EventSink::random`]
/// implementations to return specific values for the different calls to it.
///
/// [`Any::downcast_ref`]: core::any::Any#method.downcast_ref
/// [`EventSink::random`]: crate::world::EventSink::random
pub fn convert_mut<S, D>(s: &mut S) -> Option<&mut D>
where
    S: 'static,
    D: 'static,
{
    if TypeId::of::<S>() == TypeId::of::<D>() {
        // Safety
        // S and D are the same type because their TypeIds match.
        Some(unsafe { &mut *(s as *mut S as *mut D) })
    } else {
        None
    }
}

/// A generic error message for lacking a certain status effect.
///
/// It is output as `Not under the effect of <status name>`.
pub fn status_proc_error(f: &mut impl fmt::Write, status: StatusEffect) -> fmt::Result {
    write!(f, "Not under the effect of '{}'.", status.name)
}

/// Exits the function early with an error.
#[macro_export]
macro_rules! err {
    ($e:expr) => {
        { return Err($e.into()); }
    };
}