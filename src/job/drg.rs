use std::{convert::TryInto, fmt};

use crate::{
    action_cooldown, math,
    sim::{
        cooldown::{ActionCooldown, CooldownManager},
        ActionError, EffectInstance, StatusEffect,
    },
    status_effect,
    action_combo
};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DrgAction {
    // GCDs
    True,
    Disembowel,
    Chaos,
    Vorpal,
    Full,
    FangClaw,
    Wheeling,
    Raiden,
    Doom,
    Sonic,
    Torment,
    Talon,
    // oGCDs
    // Buffs
    Surge,
    Charge,
    Sight,
    Litany,
    // Jumps
    Jump, // High Jump, once I get around to lower level stuff this might change
    Spineshatter,
    Dragonfire,
    Stardiver,
    // Misc
    Geirskogul,
    Nastrond,
    Mirage,
    Blood,
}

action_cooldown! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    #[repr(u8)]
    pub enum DrgActionCooldown: DrgAction {
        Surge,
        Charge,
        Sight,
        Litany,
        Jump,
        Spineshatter,
        Dragonfire,
        Stardiver,
        Geirskogul,
        Nastrond,
        Mirage,
        Blood,
    }
}
impl DrgActionCooldown {
    pub const LENGTH: usize = 12;
}

impl DrgAction {
    pub const fn gcd(&self) -> bool {
        // Very cursed but as long as all the GCDs are in a line it will be fine
        (Self::True as u8) <= *self as u8 && *self as u8 <= (Self::Talon as u8)
    }
}

action_combo! {
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub enum Combo {
        // Single
        Disembowel,
        Chaos,
        Vorpal,
        Full,
        FangClaw,
        Wheeling,
        Raiden,
        // AoE
        Sonic,
        Torment,
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DrgJobState {
    pub combo: Combo,
    pub eyes: u8,
    pub blood: u16,
    pub cooldown: CooldownManager<DrgActionCooldowns>,
}

#[derive(Copy, Clone)]
pub struct DrgActionCooldowns {
    arr: [u32; DrgActionCooldown::LENGTH],
}
impl ActionCooldown for DrgActionCooldowns {
    // Lazy and not space efficient but I don't want to manually code a better solution :)
    // I should really make a macro for this.
    // Turns out a macro would be a pain to make. Who would have thought :))))))))))))))))
    type Action = DrgActionCooldown;
    fn new() -> Self
    where
        Self: Sized,
    {
        Self { arr: [0; DrgActionCooldown::LENGTH] }
    }
    fn get(&self, ac: &Self::Action) -> u32 {
        self.arr[*ac as u8 as usize]
    }
    fn ref_mut(&mut self, ac: &Self::Action) -> &mut u32 {
        &mut self.arr[*ac as u8 as usize]
    }
    fn advance(&mut self, time: u32) {
        for x in self.arr.iter_mut() {
            *x = x.saturating_sub(time);
        }
    }
}
impl fmt::Debug for DrgActionCooldowns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DrgActionCooldown")
    }
}

pub trait DrgEventHandler {
    fn damage(&mut self, potency: u64, surge: bool);
    fn self_effect_apply(&mut self, effect: EffectInstance);
    fn effect_apply(&mut self, effect: EffectInstance);
    fn dot_apply(&mut self, effect: EffectInstance, dot_potency: u64);
}

pub static CHAOS_THRUST_EFFECT: StatusEffect = status_effect!(
    "Chaos Thrust"
);

pub static LIFE_SURGE_EFFECT: StatusEffect = status_effect!(
    "Life Surge" { crit { out = 1000 } }
);

pub static LANCE_CHARGE_EFFECT: StatusEffect = status_effect!(
    "Lance Charge" { damage { out = 115 / 100 } }
);

pub static RIGHT_EYE_EFFECT: StatusEffect = status_effect!(
    // 110 / 100
    "Right Eye" { damage { out = 11 / 10 } }
);

pub static LEFT_EYE_EFFECT: StatusEffect = status_effect!(
    "Left Eye" { damage { out = 105 / 100 } }
);

pub static LITANY_EFFECT: StatusEffect = status_effect!(
    "Battle Litany" { crit { out = 100 } }
);