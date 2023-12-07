use core::{cmp, ops};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::world::{Actor, ActorId};

pub const fn time_until_end_cd(cd: u16, ac_cooldown: u16, charges: u16) -> u16 {
    match charges {
        0 => cd,
        c => cd.saturating_sub(ac_cooldown * (c - 1)),
    }
}

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

pub const fn combo_pot(base: u16, if_combo: u16, combo: bool) -> u16 {
    if combo {
        if_combo
    } else {
        base
    }
}

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
pub struct GaugeU8<const MAX: u8>(u8);

impl<const MAX: u8> GaugeU8<MAX> {
    pub fn set(&mut self, value: u8) {
        if value > MAX {
            panic!()
        }
        self.0 = value;
    }
    pub fn set_unchecked(&mut self, value: u8) {
        self.0 = value;
    }

    pub fn set_max(&mut self) {
        self.0 = MAX;
    }

    pub fn max(&self) -> u8 {
        MAX
    }

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
pub struct ComboState<C> {
    pub combo: Option<(C, u32)>,
}

impl<C> ComboState<C> {
    pub fn reset(&mut self) {
        self.combo = None;
    }

    pub fn set(&mut self, combo: C) {
        self.combo = Some((combo, 30000));
    }

    pub fn check(&self, combo: C) -> bool
    where
        C: Eq,
    {
        self.combo
            .as_ref()
            .map(|(c, _)| c == &combo)
            .unwrap_or_default()
    }

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

pub fn actor_id<'w>(actor: &impl Actor<'w>) -> ActorId {
    actor.id()
}
