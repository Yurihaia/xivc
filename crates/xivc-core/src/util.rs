//! Various utility types and functions.

use core::{cmp, ops};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Default)]
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
    /// // Set the gauge to some 30.
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

#[macro_export]
/// A utility macro to submit an error and then short circuit
/// from a [`cast_snap`] function if the player does not have a target.
///
/// The first parameter is the expression to evaluate the target from.
/// This is an `Option<T>` by default, or an iterator in the case of `aoe` or `uaoe`.
///
/// The second parameter should be a mutable reference to the event sink
/// that the error should be submitted to.
///
/// Finally, an optional keyword of `aoe` or `uaoe` can be specified,
/// which changes the behavior of the macro.
/// * None: Used for single target.
/// * `aoe`: Used for an aoe that needs to separate
///     the primary target from the secondary targets.
/// * `uaoe`: Used for an aoe that does not need to separate
///     the primary target from the secondary targets,
///     but still requires a target to be cast.
///
/// # Examples
///
/// ```
/// # use xivc_core::world::{
/// #     World,
/// #     ActorId,
/// #     EventProxy,
/// #     ActionTargetting,
/// #     Faction,
/// #     DamageEventExt,
/// #     Actor,
/// # };
/// # use xivc_core::timing::{EventCascade};
/// # use xivc_core::need_target;
/// # use xivc_core::enums::DamageInstance;
/// # fn example(world: &impl World, event_sink: &mut impl EventProxy) {
/// # let src = world.actor(ActorId(0)).unwrap();
/// // Constants like these are recommended to reduce boilerplate.
/// const TARGET_CIRCLE: ActionTargetting = ActionTargetting::target_circle(5, 25);
/// const MELEE: ActionTargetting = ActionTargetting::single(3);
///
/// // A closure like this is also recommended to reduce boilerplate.
/// let target_enemy = |t: ActionTargetting| {
///     src.actors_for_action(Some(Faction::Enemy), t).map(|a| a.id())
/// };
///
/// // Deal damage to targets in a circle with a radius of 5y and a range of 25y.
/// // This aoe will have damage falloff.
/// let (first, other) = need_target!(target_enemy(TARGET_CIRCLE), event_sink, aoe);
/// let mut cascade = EventCascade::new(600, 1);
/// event_sink.damage(src, DamageInstance::new(1000).slashing(), first, cascade.next());
/// for target in other {
///     event_sink.damage(src, DamageInstance::new(500).slashing(), target, cascade.next());
/// }
///
/// // Deal damage to a single target within a range of 3y.
/// let target = need_target!(target_enemy(MELEE).next(), event_sink);
/// event_sink.damage(src, DamageInstance::new(350).slashing(), target, 400);
/// # }
/// ```
///
/// [`cast_snap`]: crate::job::Job::cast_snap
macro_rules! need_target {
    ($t:expr, $p:expr) => {{
        let Some(v) = $t else {
            $p.error($crate::world::EventError::NoTarget);
            return;
        };
        v
    }};
    ($t:expr, $p:expr, aoe) => {{
        let mut i = $t;
        let Some(first) = i.next() else {
            $p.error($crate::world::EventError::NoTarget);
            return;
        };
        (first, i)
    }};
    ($t:expr, $p:expr, uaoe) => {{
        let mut i = ($t).peekable();
        if i.peek().is_none() {
            $p.error($crate::world::EventError::NoTarget);
            return;
        }
        i
    }};
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
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
