use macros::var_consts;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    enums::ActionCategory,
    status_effect,
    world::{
        status::{StatusEffect, StatusEventExt},
        Actor, EventSink, World,
    },
};

#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
#[var_consts {
    /// Returns the human friendly name of the action.
    pub const name: &'static str;
    /// Returns the cooldown of the skill in milliseconds.
    pub const cooldown: u32 = 0;
    /// Returns the number of charges a skill has, or `1` if it is a single charge skill.
    pub const cd_charges: u8 = 1;
    /// Returns the delay in milliseconds for the damage/statuses to be applied.
    pub const effect_delay: u32 = 0;
    /// Returns the [`ActionCategory`] this action is part of.
    pub const category: ActionCategory = ActionCategory::Ability;
}]
#[allow(missing_docs)]
/// A melee DPS role action.
pub enum MeleeRoleAction {
    #[cooldown = 45000]
    #[cd_charges = 2]
    #[name = "True North"]
    TrueNorth,
}

/// The status effect "True North".
pub const TRUE_NORTH: StatusEffect = status_effect!("True North" 10000);

impl MeleeRoleAction {
    /// Casts the role action, submitting all events to the supplied event sink.
    pub fn cast<'w, W: World + 'w>(&self, event_sink: &mut impl EventSink<'w, W>) {
        let this_id = event_sink.source().id();
        let dl = self.effect_delay();
        match self {
            Self::TrueNorth => {
                event_sink.apply_status(TRUE_NORTH, 1, this_id, dl);
            }
        }
    }
}
