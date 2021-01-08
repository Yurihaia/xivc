use std::fmt;

use crate::{
    math,
    sim::{
        cooldown::{ActionCooldown, CooldownManager},
        ActionError, EffectInstance, StatusEffect
    },
    status_effect,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
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

    // Always keep this last as a sort of length marker
    // this is a very shitty solution but at least it works :)
    #[doc(hidden)]
    LengthMarker,
}

impl GnbAction {
    pub const fn gcd(&self) -> bool {
        // Very cursed but as long as all the GCDs are in a line it will be fine
        (Self::Keen as u8) <= *self as u8 && *self as u8 <= (Self::Lightning as u8)
    }

    // The time when one more charge exists
    const fn charge_limit(&self) -> u32 {
        if let Self::Divide = self {
            3000
        } else {
            0
        }
    }
}

// Having these inside of a module helps solidify the name in terms of autocompletion
// Also the pattern of combo::Keen instead of KeenCombo is nicer imo
pub mod combo {
    crate::action_combo! {
        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub enum Keen {
            Brutal,
            Solid,
        }

        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub enum Slice {
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
    pub kc: combo::Keen,
    pub sc: combo::Slice,
    pub gc: combo::Gnashing,
    pub cont: combo::Cont,
    pub carts: u8,
    pub cooldown: CooldownManager<GnbActionCooldown>,
}

#[derive(Copy, Clone)]
pub struct GnbActionCooldown {
    // Make sure to update this :)
    arr: [u32; GnbAction::LengthMarker as u8 as usize],
}
impl ActionCooldown for GnbActionCooldown {
    // Lazy and not space efficient but I don't want to manually code a better solution :)
    // I should really make a macro for this.
    // Turns out a macro would be a pain to make. Who would have thought :))))))))))))))))
    type Action = GnbAction;
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            arr: [0; GnbAction::LengthMarker as u8 as usize],
        }
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
impl fmt::Debug for GnbActionCooldown {
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
            kc: combo::Keen::None,
            sc: combo::Slice::None,
            gc: combo::Gnashing::None,
            cooldown: CooldownManager::new(GnbActionCooldown::new()),
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
        match self.cooldown.action(&ac) {
            0 => (),
            v if v <= ac.charge_limit() => (),
            v => return Err(ActionError::ActionCooldown(v)),
        }
        let mut ani_lock = 60;
        let mut cont = combo::Cont::None;
        match ac {
            // GCDs
            Keen => {
                event.damage(200);
                self.kc = combo::Keen::Brutal(1500);
                self.sc = combo::Slice::None;
                self.gc = combo::Gnashing::None;
            }
            Brutal => {
                if let combo::Keen::Brutal(_) = self.kc {
                    event.damage(300);
                    self.kc = combo::Keen::Solid(1500);
                } else {
                    event.damage(100);
                    self.kc = combo::Keen::None;
                }
                self.sc = combo::Slice::None;
                self.gc = combo::Gnashing::None;
            }
            Solid => {
                if let combo::Keen::Solid(_) = self.kc {
                    event.damage(400);
                    self.carts = (self.carts + 1).min(2);
                } else {
                    event.damage(100);
                }
                self.kc = combo::Keen::None;
                self.sc = combo::Slice::None;
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
                self.kc = combo::Keen::None;
                self.sc = combo::Slice::Slaughter(1500);
                self.gc = combo::Gnashing::None;
            }
            Slaughter => {
                if let combo::Slice::Slaughter(_) = self.sc {
                    event.damage(250);
                    self.carts = (self.carts + 1).min(2);
                } else {
                    event.damage(100);
                }
                self.kc = combo::Keen::None;
                self.sc = combo::Slice::None;
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
                    self.cooldown
                        .apply_action(&ac, math::speed_calc(speed, 3000) as u32);
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
                self.cooldown
                    .apply_action(&ac, math::speed_calc(speed, 6000) as u32);
            }
            Lightning => {
                event.damage(150);
                self.kc = combo::Keen::None;
                self.sc = combo::Slice::None;
                self.gc = combo::Gnashing::None;
            }
            // Offensive oGCDs
            Divide => {
                event.damage(200);
                self.cooldown.apply_action(&ac, 3000);
            }
            Blasting => {
                event.damage(800);
                self.cooldown.apply_action(&ac, 3000);
            }
            Shock => {
                event.damage(200);
                event.dot_apply(EffectInstance::new(SHOCK_EFFECT, 1500, 1), 90);
                self.cooldown.apply_action(&ac, 6000);
            }
            NoMercy => {
                event.effect_apply(EffectInstance::new(NO_MERCY_EFFECT, 2000, 1));
                self.cooldown.apply_action(&ac, 6000);
            }
            Bloodfest => {
                self.carts = 2;
                self.cooldown.apply_action(&ac, 9000);
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
            LengthMarker => {
                ani_lock = 0;
                eprint!("LengthMarker used as an action :)");
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
        self.kc.advance(time);
        self.sc.advance(time);
        self.gc.advance(time);
        self.cont.advance(time);
    }
}

impl Default for GnbJobState {
    fn default() -> Self {
        Self::new()
    }
}
