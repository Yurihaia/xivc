use core::fmt::{self, Display};

use macros::var_consts;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    job::{ActionType, Job, JobAction, JobState},
    job_cd_struct,
    math::SpeedStat,
    need_target, status_effect,
    timing::{EventCascade, JobCds},
    util::{actor_id, combo_pos_pot, combo_pot, ComboState, GaugeU8},
    world::{
        status::{consume_status, consume_status_stack, StatusEffect, StatusEventExt},
        ActionTargetting, Actor, CastSnapEvent, DamageEventExt, EventError, EventProxy, Faction,
        Positional, World,
    },
};

// so i can be lazy with associated constant derives
#[derive(Copy, Clone, Debug, Default)]
pub struct SamJob;

pub const FUGETSU: StatusEffect = status_effect!(
    "Fugetsu" 40000 { damage { out = 113 / 100 } }
);
pub const FUKA: StatusEffect = status_effect!(
    "Fuka" 40000 { haste { 100 - 13 } }
);
pub const OGI_READY: StatusEffect = status_effect!("Ogi Namikiri Ready" 30000);
pub const MEIKYO: StatusEffect = status_effect!("Meikyo Shisui" 15000);
pub const ENENPI: StatusEffect = status_effect!("Enhanced Enpi" 15000);
pub const HIGANBANA: StatusEffect = status_effect!("Higanbana" 60000);

impl Job for SamJob {
    type Action = SamAction;
    type State = SamState;
    type Error = SamError;
    type Event = ();

    // technically this can't handle dropped casts (cooldowns get reset if a cast drops)
    // but i doubt that option will EVER be useful
    fn init_cast<'w, P: EventProxy, W: World>(
        ac: Self::Action,
        s: &mut Self::State,
        w: &W,
        this: &'w W::Actor<'w>,
        p: &mut P,
    ) {
        // all of these don't return to give better information to the user
        #[rustfmt::skip] // rust fmt mangles these two things in a really bad way
        let snap = s.cds.set_cast_lock(
            p,
            w.duration_info(),
            ac.cast(),
            600,
            None,
        );

        if ac.gcd() {
            #[rustfmt::skip]
            s.cds.set_gcd(
                p,
                w.duration_info(),
                2500,
                Some(SpeedStat::SkillSpeed)
            );
        }

        // don't need to worry about gcd length cooldowns
        if !s.cds.job.available(ac, ac.cooldown(), ac.cd_charges()) {
            p.error(EventError::Cooldown(ac.into()));
        }
        if ac.cooldown() > 0 {
            s.cds.job.apply(ac, ac.cooldown(), ac.cd_charges());
        }

        use SamAction::*;
        if s.kenki < ac.kenki_cost() {
            SamError::Kenki(ac.kenki_cost()).submit(p);
        }
        if ac.iaijutsu() && ac.sen_cost() != s.sen.count() {
            SamError::IaiSen(ac.sen_cost()).submit(p);
        }
        match ac {
            Ikishoten if !this.in_combat() => p.error(EventError::InCombat),
            Namikiri if !this.has_own_status(OGI_READY) => {
                SamError::OgiRdy.submit(p);
            }
            Shoha | Shoha2 if s.meditation != 3 => SamError::Meditation.submit(p),
            Hagakure if s.sen.count() == 0 => SamError::HagaSen.submit(p),
            // really ugly if the if guard is in the match branch lol
            KaeshiHiganbana | KaeshiGoken | KaeshiSetsugekka | KaeshiNamikiri => {
                if !s.combos.check_kaeshi_for(ac) {
                    SamError::Kaeshi(ac).submit(p)
                }
            }
            _ => (),
        }
        // create a cast snapshot event at the specified time
        // will be 0 if ac.cast() is 0
        p.event(CastSnapEvent::new(ac).into(), snap);
    }

    fn cast_snap<'w, P: EventProxy, W: World>(
        ac: Self::Action,
        s: &mut Self::State,
        _: &W,
        this: &'w W::Actor<'w>,
        p: &mut P,
    ) {
        use SamAction::*;
        let target_enemy = |t: ActionTargetting| {
            this.actors_for_action(t)
                .filter(|t| t.faction() == Faction::Enemy)
                .map(actor_id)
        };

        let consume_meikyo = |p: &mut P| consume_status_stack(this, p, MEIKYO, 0);

        let dl = ac.effect_delay();

        let this_id = this.id();

        match ac {
            Hakaze => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.kaeshi.reset();
                consume_meikyo(p);
                s.combos.main.set(MainCombo::Hakaze);
                s.kenki += 5;
                p.damage(200, t, dl);
            }
            Jinpu => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Hakaze) {
                    s.combos.main.set(MainCombo::Jinpu);
                    s.kenki += 5;
                    p.apply_status(FUGETSU, 1, this_id, dl);
                    true
                } else {
                    s.combos.main.reset();
                    false
                };
                p.damage(combo_pot(120, 280, combo), t, dl);
            }
            Shifu => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Hakaze) {
                    s.combos.main.set(MainCombo::Shifu);
                    s.kenki += 5;
                    p.apply_status(FUKA, 1, this_id, dl);
                    true
                } else {
                    s.combos.main.reset();
                    false
                };
                p.damage(combo_pot(120, 280, combo), t, dl);
            }
            Yukikaze => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Hakaze) {
                    s.kenki += 15;
                    s.sen.grant_setsu();
                    true
                } else {
                    false
                };
                p.damage(combo_pot(120, 300, combo), t, dl);
                s.combos.main.reset();
            }
            Gekko => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Jinpu) {
                    s.kenki += 10;
                    s.sen.grant_getsu();
                    true
                } else {
                    false
                };
                if meikyo {
                    p.apply_status(FUGETSU, 1, this_id, dl);
                }
                let pos = this.check_positional(Positional::Rear, t);
                p.damage(combo_pos_pot(120, 170, 330, 380, combo, pos), t, dl);
                s.combos.main.reset();
            }
            Kasha => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Shifu) {
                    s.kenki += 10;
                    s.sen.grant_kaa();
                    true
                } else {
                    false
                };
                if meikyo {
                    p.apply_status(FUKA, 1, this_id, dl);
                }
                let pos = this.check_positional(Positional::Rear, t);
                p.damage(combo_pos_pot(120, 170, 330, 380, combo, pos), t, dl);
                s.combos.main.reset();
            }
            Fuga | Fuko => {
                s.combos.kaeshi.reset();
                consume_meikyo(p);
                let mut c = EventCascade::new(dl);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    p.damage(100, t, c.next());
                }
                if hit {
                    s.combos.main.set(MainCombo::Fuko);
                    s.kenki += 10;
                } else {
                    s.combos.main.reset();
                }
            }
            Mangetsu => {
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Fuko) {
                    s.kenki += 10;
                    s.sen.grant_getsu();
                    p.apply_status(FUGETSU, 1, this_id, dl);
                    true
                } else {
                    false
                };
                let mut c = EventCascade::new(dl);
                for t in target_enemy(CIRCLE) {
                    p.damage(combo_pot(100, 120, combo), t, c.next());
                }
                s.combos.main.reset();
            }
            Oka => {
                s.combos.kaeshi.reset();
                let meikyo = consume_meikyo(p);
                let combo = if meikyo || s.combos.main.check(MainCombo::Fuko) {
                    s.kenki += 10;
                    s.sen.grant_kaa();
                    p.apply_status(FUKA, 1, this_id, dl);
                    true
                } else {
                    false
                };
                let mut c = EventCascade::new(dl);
                for t in target_enemy(CIRCLE) {
                    p.damage(combo_pot(100, 120, combo), t, c.next());
                }
                s.combos.main.reset();
            }
            Enpi => {
                let t = need_target!(target_enemy(RANGED).next(), p);
                s.kenki += 10;
                let en_enpi = consume_status(this, p, ENENPI, 0);
                p.damage(if en_enpi { 260 } else { 100 }, t, dl);
            }
            Shinten => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.kenki -= 25;
                p.damage(250, t, dl);
            }
            Kyuten => {
                s.kenki -= 25;
                let mut c = EventCascade::new(dl);
                for t in target_enemy(CIRCLE) {
                    p.damage(120, t, c.next());
                }
            }
            Gyoten => {
                let t = need_target!(target_enemy(RANGED).next(), p);
                s.kenki -= 10;
                p.damage(100, t, dl);
            }
            Yaten => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.kenki -= 10;
                p.damage(100, t, dl);
                p.apply_status(ENENPI, 1, this_id, dl);
            }
            Hagakure => {
                let sen_count = s.sen.count();
                s.sen.clear();
                s.kenki += sen_count * 10;
            }
            Guren => {
                let (first, other) = need_target!(target_enemy(ActionTargetting::line(10)), p, aoe);
                s.kenki -= 25;
                let mut c = EventCascade::new(dl);
                p.damage(500, first, c.next());
                for t in other {
                    p.damage(375, t, c.next());
                }
            }
            Meikyo => {
                // meikyo might not even have a delay i'm not sure
                // it seems REALLY fast
                p.apply_status(MEIKYO, 3, this_id, dl);
            }
            Senei => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.kenki -= 25;
                p.damage(860, t, dl);
            }
            Ikishoten => {
                s.kenki += 50;
                p.apply_status(OGI_READY, 1, this_id, dl);
            }
            Shoha => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.meditation.clear();
                p.damage(560, t, dl);
            }
            Shoha2 => {
                s.meditation.clear();
                let mut c = EventCascade::new(dl);
                for t in target_enemy(CIRCLE) {
                    p.damage(200, t, c.next());
                }
            }
            Namikiri => {
                let (first, other) =
                    need_target!(target_enemy(ActionTargetting::cone(8, 135)), p, aoe);
                s.meditation += 1;
                s.combos.kaeshi.set(KaeshiCombo::Namikiri);
                consume_status(this, p, OGI_READY, 0);
                let mut c = EventCascade::new(dl);
                p.damage_ch(860, first, c.next());
                for t in other {
                    p.damage_ch(215, t, c.next());
                }
            }
            Higanbana => {
                let t = need_target!(target_enemy(IAIJUTSU).next(), p);
                s.meditation += 1;
                s.combos.kaeshi.set(KaeshiCombo::Higanbana);
                s.sen.clear();
                p.damage(200, t, dl);
                p.apply_dot(HIGANBANA, 45, 1, t, dl);
            }
            TenkaGoken => {
                s.meditation += 1;
                s.combos.kaeshi.set(KaeshiCombo::Goken);
                s.sen.clear();
                let mut c = EventCascade::new(dl);
                for t in target_enemy(ActionTargetting::circle(8)) {
                    p.damage(300, t, c.next());
                }
            }
            Midare => {
                let t = need_target!(target_enemy(IAIJUTSU).next(), p);
                s.meditation += 1;
                s.combos.kaeshi.set(KaeshiCombo::Setsugekka);
                s.sen.clear();
                p.damage_ch(640, t, dl);
            }
            KaeshiHiganbana => {
                let t = need_target!(target_enemy(IAIJUTSU).next(), p);
                s.meditation += 1;
                s.combos.kaeshi.reset();
                p.damage(200, t, dl);
                p.apply_dot(HIGANBANA, 45, 1, t, dl);
            }
            KaeshiGoken => {
                s.meditation += 1;
                s.combos.kaeshi.reset();
                let mut c = EventCascade::new(dl);
                for t in target_enemy(ActionTargetting::circle(8)) {
                    p.damage(300, t, c.next());
                }
            }
            KaeshiSetsugekka => {
                let t = need_target!(target_enemy(IAIJUTSU).next(), p);
                s.meditation += 1;
                s.combos.kaeshi.reset();
                p.damage_ch(640, t, dl);
            }
            KaeshiNamikiri => {
                let (first, other) =
                    need_target!(target_enemy(ActionTargetting::cone(8, 135)), p, aoe);
                s.meditation += 1;
                s.combos.kaeshi.reset();
                let mut c = EventCascade::new(dl);
                p.damage_ch(860, first, c.next());
                for t in other {
                    p.damage_ch(215, t, c.next());
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SamError {
    Kaeshi(SamAction),
    Kenki(u8),
    IaiSen(u8),
    HagaSen,
    Meditation,
    OgiRdy,
}
impl SamError {
    pub fn submit(self, p: &mut impl EventProxy) {
        p.error(self.into())
    }
}

impl From<SamError> for EventError {
    fn from(value: SamError) -> Self {
        Self::Job(value.into())
    }
}

impl Display for SamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kaeshi(ac) => write!(f, "Not executed following {}", ac.name()),
            Self::Kenki(k) => write!(f, "Not enough Kenki, needed at least {}", k),
            Self::IaiSen(s) => write!(f, "Invalid Sen count, expected {}", s),
            Self::HagaSen => write!(f, "Invalid Sen count, expected at least 1"),
            Self::Meditation => write!(f, "Invalid Meditation count, expected 3"),
            Self::OgiRdy => write!(f, "Not under the effect of 'Ogi Namikiri Ready'"),
        }
    }
}

const MELEE: ActionTargetting = ActionTargetting::single(3);
const RANGED: ActionTargetting = ActionTargetting::single(20);
const IAIJUTSU: ActionTargetting = ActionTargetting::single(6);
const CIRCLE: ActionTargetting = ActionTargetting::circle(5);

#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
#[var_consts]
#[flag(
    /// Returns `true` if the action is a GCD.
    pub gcd
)]
#[flag(
    /// Returns `tru` if the action is an iaijutsu.
    pub iaijutsu
)]
#[flag(
    /// Returns `true` if the action uses tsubame gaeshi.
    pub tsubame
)]
#[property(
    /// Returns the base milliseconds the action takes to cast.
    pub const cast: u16 = 0
)]
#[property(
    /// Returns the human friendly name of the action.
    pub const name: &'static str
)]
#[property(
    /// Returns the cooldown of the skill in milliseconds.
    pub const cooldown: u32 = 0
)]
#[property(
    /// Returns the number of charges a skill has, or 1 if it is a single charge skill.
    pub const cd_charges: u8 = 1
)]
#[property(
    /// Returns the delay in milliseconds for the damage/statuses to be applied.
    pub const effect_delay: u32 = 0
)]
#[property(
    /// Returns the kenki cost of the specified action.
    pub const kenki_cost: u8 = 0
)]
#[property(
    /// Returns the number of sen needed to perform the iaijutsu.
    pub const sen_cost: u8 = 0
)]
pub enum SamAction {
    #[gcd]
    #[name = "Hakaze"]
    Hakaze,
    #[gcd]
    #[name = "Jinpu"]
    Jinpu,
    #[gcd]
    #[name = "Shifu"]
    Shifu,
    #[gcd]
    #[name = "Yukikaze"]
    Yukikaze,
    #[gcd]
    #[name = "Gekko"]
    Gekko,
    #[gcd]
    #[name = "Kasha"]
    Kasha,
    #[gcd]
    #[name = "Fuga"]
    Fuga,
    #[gcd]
    #[name = "Mangetsu"]
    Mangetsu,
    #[gcd]
    #[name = "Oka"]
    Oka,
    #[gcd]
    #[name = "Enpi"]
    Enpi,
    #[name = "Hissatsu: Shinten"]
    #[cooldown = 1000]
    #[kenki_cost = 25]
    Shinten,
    #[name = "Hissatsu: Kyuten"]
    #[cooldown = 1000]
    #[kenki_cost = 25]
    Kyuten,
    #[name = "Hissatsu: Gyoten"]
    #[cooldown = 10000]
    #[kenki_cost = 10]
    Gyoten,
    #[name = "Hissatsu: Yaten"]
    #[cooldown = 10000]
    #[kenki_cost = 10]
    Yaten,
    #[name = "Hagakure"]
    #[cooldown = 5000]
    Hagakure,
    #[name = "Hissatsu: Guren"]
    #[cooldown = 120000]
    #[kenki_cost = 25]
    Guren,
    // Meditate,
    // ThirdEye,
    #[name = "Meikyo Shisui"]
    #[cooldown = 55000]
    #[cd_charges = 2]
    Meikyo,
    // commenting this out for now because its a fake skill
    // #[gcd]
    // #[cast = 130]
    // #[name = "Iaijutsu"]
    // Iaijutsu,
    #[name = "Hissatsu: Senei"]
    #[cooldown = 120000]
    #[kenki_cost = 25]
    Senei,
    #[name = "Ikishoten"]
    #[cooldown = 120000]
    Ikishoten,
    // commenting this out for now because its a fake skill
    // #[gcd]
    // #[name = "Tsubame-gaeshi"]
    // Tsubame,
    #[name = "Shoha"]
    #[cooldown = 15000]
    Shoha,
    #[name = "Shoha II"]
    #[cooldown = 15000]
    // looks better than shoha2
    #[cfg_attr(feature = "serde", serde(rename = "shoha_2"))]
    Shoha2,
    #[name = "Fuko"]
    Fuko,
    #[gcd]
    #[cast = 1300]
    #[name = "Ogi Namikiri"]
    Namikiri,
    #[gcd]
    #[cast = 1300]
    #[name = "Higanbana"]
    #[sen_cost = 1]
    #[iaijutsu]
    Higanbana,
    #[gcd]
    #[cast = 1300]
    #[name = "Tenka Goken"]
    #[sen_cost = 2]
    #[iaijutsu]
    TenkaGoken,
    #[gcd]
    #[cast = 1300]
    #[name = "Midare Setsugekka"]
    #[sen_cost = 3]
    #[iaijutsu]
    Midare,
    #[gcd]
    #[name = "Kaeshi: Higanbana"]
    #[cooldown = 60000]
    #[cd_charges = 2]
    #[tsubame]
    KaeshiHiganbana,
    #[gcd]
    #[name = "Kaeshi: Goken"]
    #[cooldown = 60000]
    #[cd_charges = 2]
    #[tsubame]
    KaeshiGoken,
    #[gcd]
    #[name = "Kaeshi: Setsugekka"]
    #[cooldown = 60000]
    #[cd_charges = 2]
    #[tsubame]
    KaeshiSetsugekka,
    #[gcd]
    #[name = "Kaeshi: Namikiri"]
    KaeshiNamikiri,
}

impl SamAction {
    pub const fn cd_speed_stat(&self) -> Option<SpeedStat> {
        None
    }
}

impl JobAction for SamAction {
    fn action_type(&self) -> ActionType {
        if self.gcd() {
            ActionType::Weaponskill
        } else {
            ActionType::Ability
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SamState {
    pub cds: JobCds<SamCds>,
    pub combos: SamCombos,
    pub sen: Sen,
    pub meditation: GaugeU8<3>,
    pub kenki: GaugeU8<100>,
}

impl JobState for SamState {
    fn advance(&mut self, time: u32) {
        self.combos.advance(time);
        self.cds.advance(time);
        self.cds.job.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SamCombos {
    pub main: ComboState<MainCombo>,
    pub kaeshi: ComboState<KaeshiCombo>,
}

impl SamCombos {
    pub fn check_main_for(&self, action: SamAction) -> bool {
        let c = match action {
            SamAction::Jinpu | SamAction::Shifu | SamAction::Yukikaze => MainCombo::Hakaze,
            SamAction::Kasha => MainCombo::Shifu,
            SamAction::Gekko => MainCombo::Jinpu,
            SamAction::Mangetsu | SamAction::Oka => MainCombo::Fuko,
            _ => return true,
        };
        self.main.check(c)
    }

    pub fn check_kaeshi_for(&self, action: SamAction) -> bool {
        let c = match action {
            SamAction::KaeshiHiganbana => KaeshiCombo::Higanbana,
            SamAction::KaeshiGoken => KaeshiCombo::Goken,
            SamAction::KaeshiSetsugekka => KaeshiCombo::Setsugekka,
            SamAction::KaeshiNamikiri => KaeshiCombo::Namikiri,
            _ => return true,
        };
        self.kaeshi.check(c)
    }

    pub fn advance(&mut self, time: u32) {
        self.main.advance(time);
        self.kaeshi.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainCombo {
    Hakaze,
    Shifu,
    Jinpu,
    Fuko,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KaeshiCombo {
    Higanbana,
    Goken,
    Setsugekka,
    Namikiri,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct Sen {
    bits: u8,
}
impl Sen {
    const SETSU: u8 = 1 << 0;
    const GETSU: u8 = 1 << 1;
    const KA: u8 = 1 << 2;

    pub fn grant_setsu(&mut self) {
        self.bits |= Self::SETSU
    }
    pub fn grant_getsu(&mut self) {
        self.bits |= Self::GETSU
    }
    pub fn grant_kaa(&mut self) {
        self.bits |= Self::KA
    }
    pub fn count(&self) -> u8 {
        self.bits.count_ones() as u8
    }
    pub fn clear(&mut self) {
        self.bits = 0;
    }
}

job_cd_struct! {
    SamAction =>
    
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Clone, Debug, Default)]
    pub SamCds
    
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    pub SamCdGroup
    
    shinten Shinten: Shinten;
    kyuten Kyuten: Kyuten;
    gyoten Gyoten: Gyoten;
    yaten Yaten: Yaten;
    hagakure Hagakure: Hagakure;
    senei Senei: Senei Guren;
    ikishoten Ikishoten: Ikishoten;
    meikyo Meikyo: Meikyo;
    shoha Shoha: Shoha Shoha2;
    tsubame Tsubame: KaeshiHiganbana KaeshiGoken KaeshiSetsugekka;
}
