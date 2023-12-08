use core::{cmp, ops};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A utility function that returns the potency of an action
/// depending on if it was comboed into and if it hit it's positional.
pub const fn combo_pos_pot(
    base: u16,
    if_pos: u16,
    if_combo: u16,
    if_both: u16,
    combo: bool,
    pos: bool,
) -> u16 {
    match (combo, pos) {
        (false, false) => base,
        (false, true) => if_pos,
        (true, false) => if_combo,
        (true, true) => if_both,
    }
}

/// A utility function that returns the potency of an action
/// depending on if it was comboed into.
pub const fn combo_pot(base: u16, if_combo: u16, combo: bool) -> u16 {
    if combo {
        if_combo
    } else {
        base
    }
}

/// A utility function that returns the potency of an action
/// depending on if it hit it's positional.
pub const fn pos_pot(base: u16, if_pos: u16, pos: bool) -> u16 {
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
pub struct GaugeU8<const MAX: u8>(u8);

impl<const MAX: u8> GaugeU8<MAX> {
    /// Sets the gauge to a value.
    ///
    /// # Panics
    /// This function panics if the value the gauge is set to
    /// exceeds the maximum value the gauge can be.
    pub fn set(&mut self, value: u8) {
        if value > MAX {
            panic!()
        }
        self.0 = value;
    }
    /// Sets the gauge to a value, regardless of
    /// if it is larger than the maximum value or not.
    pub fn set_unchecked(&mut self, value: u8) {
        self.0 = value;
    }
    /// Sets the gauge to a value, regardless of
    /// if it is larger than the maximum value or not.
    pub fn set_saturating(&mut self, value: u8) {
        self.0 = value.min(MAX);
    }
    /// Sets the gauge to the maximum value it can hold.
    pub fn set_max(&mut self) {
        self.0 = MAX;
    }
    /// Returns the maximum value the gauge can hold.
    pub fn max(&self) -> u8 {
        MAX
    }
    /// Resets the gauge down to zero.
    pub fn clear(&mut self) {
        self.0 = 0;
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
/// # fn example(world: &impl World, event_sink: &mut impl EventProxy) {
/// # let src = world.actor(ActorId(0)).unwrap();
/// const TARGET_CIRCLE: ActionTargetting = ActionTargetting::target_circle(5, 25);
/// const MELEE: ActionTargetting = ActionTargetting::single(3);
/// // ...
/// let target_enemy = |t: ActionTargetting| {
///     src.actors_for_action(Some(Faction::Enemy), t).map(|a| a.id())
/// };
/// // ...
/// let (first, other) = need_target!(target_enemy(TARGET_CIRCLE), event_sink, aoe);
/// let mut cascade = EventCascade::new(600);
/// event_sink.damage(1000, first, cascade.next());
/// for target in other {
///     event_sink.damage(500, target, cascade.next());
/// }
/// // ...
/// let target = need_target!(target_enemy(MELEE).next(), event_sink);
/// event_sink.damage(350, target, 400);
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
pub struct ComboState<C> {
    pub combo: Option<(C, u32)>,
}

impl<C> ComboState<C> {
    /// Resets the combo.
    pub fn reset(&mut self) {
        self.combo = None;
    }
    /// Sets the combo to the specified combo stage.
    pub fn set(&mut self, combo: C) {
        self.combo = Some((combo, 30000));
    }
    /// Checks if the combo is at the specified combo stage.
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
