use std::{convert::TryInto, fmt};

use crate::{
    action_cooldown, math,
    sim::{
        cooldown::{ActionCooldown, CooldownManager},
        ActionError, EffectInstance, StatusEffect,
    },
    status_effect,
};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GnbAction {
    // GCDs
    Keen,
    Brutal,
    Solid,
    Burst,
    Slice,
    Slaughter,
    Fated,
    Gnashing,
    Savage,
    Wicked,
    Sonic,
    Lightning,
    // Offensive oGCDs
    Divide,
    Blasting,
    Shock,
    NoMercy,
    Bloodfest,
    Jugular,
    Abdomen,
    Eye,
    // Defensive oGCDs (not implemented yet)
}

action_cooldown! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    #[repr(u8)]
    pub enum GnbActionCooldown: GnbAction {
        Gnashing,
        Sonic,
        Divide,
        Blasting,
        Shock,
        NoMercy,
        Bloodfest,
    }
}

impl GnbActionCooldown {
    pub const LENGTH: usize = 7;
    // The time when one more charge exists
    const fn charge_limit(&self) -> u32 {
        if let Self::Divide = self {
            3000
        } else {
            0
        }
    }
}

impl GnbAction {
    pub const fn gcd(&self) -> bool {
        // Very cursed but as long as all the GCDs are in a line it will be fine
        (Self::Keen as u8) <= *self as u8 && *self as u8 <= (Self::Lightning as u8)
    }
}

// Having these inside of a module helps solidify the name in terms of autocompletion
// Also the pattern of combo::Keen instead of KeenCombo is nicer imo
pub mod combo {
    crate::action_combo! {
        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub enum Main {
            // Single
            Brutal,
            Solid,
            // AoE
            Slaughter,
        }

        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub enum Gnashing {
            Savage,
            Wicked,
        }

        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub enum Cont {
            Jugular,
            Abdomen,
            Eye,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GnbJobState {
    pub mc: combo::Main,
    pub gc: combo::Gnashing,
    pub cont: combo::Cont,
    pub carts: u8,
    pub cooldown: CooldownManager<GnbActionCooldowns>,
}

#[derive(Copy, Clone)]
pub struct GnbActionCooldowns {
    arr: [u32; GnbActionCooldown::LENGTH],
}
impl ActionCooldown for GnbActionCooldowns {
    // Lazy and not space efficient but I don't want to manually code a better solution :)
    // I should really make a macro for this.
    // Turns out a macro would be a pain to make. Who would have thought :))))))))))))))))
    type Action = GnbActionCooldown;
    fn new() -> Self
    where
        Self: Sized,
    {
        Self { arr: [0; 7] }
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
impl fmt::Debug for GnbActionCooldowns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GnbActionCooldown")
    }
}

pub trait GnbEventHandler {
    fn damage(&mut self, potency: u64);
    fn effect_apply(&mut self, effect: EffectInstance);
    fn dot_apply(&mut self, effect: EffectInstance, dot_potency: u64);
}

pub static SONIC_EFFECT: StatusEffect = status_effect!("Sonic Break");

pub static SHOCK_EFFECT: StatusEffect = status_effect!("Bow Shock");

pub static NO_MERCY_EFFECT: StatusEffect = status_effect!(
    // 120 / 100
    "No Mercy" { damage { out = 12 / 10 } }
);

#[derive(Copy, Clone, Debug)]
pub enum GnbActionError {
    NoCarts,
    Uncomboed,
}

impl GnbJobState {
    pub fn new() -> Self {
        Self {
            carts: 0,
            cont: combo::Cont::None,
            mc: combo::Main::None,
            gc: combo::Gnashing::None,
            cooldown: CooldownManager::new(GnbActionCooldowns::new()),
        }
    }

    pub fn action_cast(
        &mut self,
        ac: GnbAction,
        // the skillspeed mod
        // essentially something like 1.025
        // scaled by 1000
        speed: u64,
        event: &mut impl GnbEventHandler,
    ) -> Result<(), ActionError<GnbActionError>> {
        use GnbAction::*;
        self.cooldown.error(ac.gcd())?;
        match ac
            .try_into()
            .map(|v: GnbActionCooldown| (v.charge_limit(), self.cooldown.action(&v)))
        {
            Ok((_, 0)) => (),
            Ok((a, v)) if v <= a => (),
            Ok((_, v)) => return Err(ActionError::ActionCooldown(v)),
            Err(_) => (),
        }
        let mut ani_lock = 60;
        let mut cont = combo::Cont::None;
        match ac {
            // GCDs
            Keen => {
                event.damage(200);
                self.mc = combo::Main::Brutal(1500);
                self.gc = combo::Gnashing::None;
            }
            Brutal => {
                if let combo::Main::Brutal(_) = self.mc {
                    event.damage(300);
                    self.mc = combo::Main::Solid(1500);
                } else {
                    event.damage(100);
                    self.mc = combo::Main::None;
                }
                self.gc = combo::Gnashing::None;
            }
            Solid => {
                if let combo::Main::Solid(_) = self.mc {
                    event.damage(400);
                    self.carts = (self.carts + 1).min(2);
                } else {
                    event.damage(100);
                }
                self.mc = combo::Main::None;
                self.gc = combo::Gnashing::None;
            }
            Burst => {
                if self.carts > 0 {
                    event.damage(500);
                    self.carts -= 1;
                } else {
                    return Err(ActionError::Job(GnbActionError::NoCarts));
                }
            }
            Slice => {
                event.damage(150);
                self.mc = combo::Main::Slaughter(1500);
                self.gc = combo::Gnashing::None;
            }
            Slaughter => {
                if let combo::Main::Slaughter(_) = self.mc {
                    event.damage(250);
                    self.carts = (self.carts + 1).min(2);
                } else {
                    event.damage(100);
                }
                self.mc = combo::Main::None;
                self.gc = combo::Gnashing::None;
            }
            Fated => {
                if self.carts > 0 {
                    event.damage(320);
                    self.carts -= 1;
                } else {
                    return Err(ActionError::Job(GnbActionError::NoCarts));
                }
            }
            Gnashing => {
                if self.carts > 0 {
                    event.damage(450);
                    self.carts -= 1;
                    self.gc = combo::Gnashing::Savage(1500);
                    cont = combo::Cont::Jugular(1000);
                    self.cooldown.apply_action(
                        &GnbActionCooldown::Gnashing,
                        math::speed_calc(speed, 3000) as u32,
                    );
                    ani_lock = 70;
                } else {
                    return Err(ActionError::Job(GnbActionError::NoCarts));
                }
            }
            Savage => {
                if let combo::Gnashing::Savage(_) = self.gc {
                    event.damage(550);
                    self.gc = combo::Gnashing::Wicked(1500);
                    cont = combo::Cont::Abdomen(1000);
                    ani_lock = 50;
                } else {
                    return Err(ActionError::Job(GnbActionError::Uncomboed));
                }
            }
            Wicked => {
                if let combo::Gnashing::Wicked(_) = self.gc {
                    event.damage(650);
                    self.gc = combo::Gnashing::None;
                    cont = combo::Cont::Eye(1000);
                    ani_lock = 77;
                } else {
                    return Err(ActionError::Job(GnbActionError::Uncomboed));
                }
            }
            Sonic => {
                event.damage(300);
                event.dot_apply(EffectInstance::new(SONIC_EFFECT, 3000, 1), 90);
                self.cooldown.apply_action(
                    &GnbActionCooldown::Sonic,
                    math::speed_calc(speed, 6000) as u32,
                );
            }
            Lightning => {
                event.damage(150);
                self.mc = combo::Main::None;
                self.gc = combo::Gnashing::None;
            }
            // Offensive oGCDs
            Divide => {
                event.damage(200);
                self.cooldown.apply_action(&GnbActionCooldown::Divide, 3000);
            }
            Blasting => {
                event.damage(800);
                self.cooldown
                    .apply_action(&GnbActionCooldown::Blasting, 3000);
            }
            Shock => {
                event.damage(200);
                event.dot_apply(EffectInstance::new(SHOCK_EFFECT, 1500, 1), 90);
                self.cooldown.apply_action(&GnbActionCooldown::Shock, 6000);
            }
            NoMercy => {
                event.effect_apply(EffectInstance::new(NO_MERCY_EFFECT, 2000, 1));
                self.cooldown
                    .apply_action(&GnbActionCooldown::NoMercy, 6000);
            }
            Bloodfest => {
                self.carts = 2;
                self.cooldown
                    .apply_action(&GnbActionCooldown::Bloodfest, 9000);
            }
            Jugular => {
                if let combo::Cont::Jugular(_) = self.cont {
                    self.cont = combo::Cont::None;
                    event.damage(260);
                } else {
                    return Err(ActionError::Job(GnbActionError::Uncomboed));
                }
            }
            Abdomen => {
                if let combo::Cont::Abdomen(_) = self.cont {
                    self.cont = combo::Cont::None;
                    event.damage(280);
                } else {
                    return Err(ActionError::Job(GnbActionError::Uncomboed));
                }
            }
            Eye => {
                if let combo::Cont::Eye(_) = self.cont {
                    self.cont = combo::Cont::None;
                    event.damage(300);
                } else {
                    return Err(ActionError::Job(GnbActionError::Uncomboed));
                }
            }
        }
        if ac.gcd() {
            self.cont = cont;
            self.cooldown
                .apply_global(math::speed_calc(speed, 250) as u32);
        }
        self.cooldown.apply_ani_lock(ani_lock);
        Ok(())
    }

    pub fn advance(&mut self, time: u32) {
        self.cooldown.advance(time);
        self.mc.advance(time);
        self.gc.advance(time);
        self.cont.advance(time);
    }
}

impl Default for GnbJobState {
    fn default() -> Self {
        Self::new()
    }
}
