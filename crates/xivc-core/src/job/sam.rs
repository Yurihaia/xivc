use core::fmt::{self, Display};

use macros::var_consts;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    job::{Job, JobState},
    job_cd_struct, need_target, status_effect,
    timing::{DurationInfo, EventCascade, ScaleTime},
    util::{combo_pos_pot, combo_pot, ComboState, GaugeU8},
    world::{
        status::{consume_status, consume_status_stack, StatusEffect, StatusEventExt},
        ActionTargetting, Actor, DamageEventExt, EventError, EventProxy, Faction, Positional,
        World,
    },
};

use super::CastInitInfo;

// so i can be lazy with associated constant derives
#[derive(Copy, Clone, Debug, Default)]
/// The [`Job`] struct for Samurai.
pub struct SamJob;

/// The status effect Fugetsu.
pub const FUGETSU: StatusEffect = status_effect!(
    "Fugetsu" 40000 { damage { out = 113 / 100 } }
);
/// The status effect Fuka.
pub const FUKA: StatusEffect = status_effect!(
    "Fuka" 40000 { haste { 100 - 13 } }
);
/// The status effect Ogi Namikiri Ready.
pub const OGI_READY: StatusEffect = status_effect!("Ogi Namikiri Ready" 30000);
/// The status effect Meikyo Shisui.
pub const MEIKYO: StatusEffect = status_effect!("Meikyo Shisui" 15000);
/// The status effect Enhanced Enpi.
pub const ENENPI: StatusEffect = status_effect!("Enhanced Enpi" 15000);
/// The DoT effect Higanbana.
pub const HIGANBANA: StatusEffect = status_effect!("Higanbana" 60000);

impl Job for SamJob {
    type Action = SamAction;
    type State = SamState;
    type CastError = SamError;
    type Event = ();
    type CdGroup = SamCdGroup;

    fn check_cast<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &Self::State,
        _: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    ) -> CastInitInfo<Self::CdGroup> {
        let di = src.duration_info();

        let gcd = action.gcd().map(|v| di.get_duration(v)).unwrap_or_default() as u16;
        let (lock, snap) = di.get_cast(action.cast(), 600);

        let cd = action
            .cd_group()
            .map(|v| (v, action.cooldown(), action.cd_charges()));

        // check errors
        if let Some((cdg, cd, charges)) = cd {
            if !state.cds.available(cdg, cd, charges) {
                event_sink.error(EventError::Cooldown(action.into()));
            }
        }

        use SamAction::*;
        if state.kenki < action.kenki_cost() {
            SamError::Kenki(action.kenki_cost()).submit(event_sink);
        }
        if let Some(sen_cost) = action.sen_cost() {
            if sen_cost != state.sen.count() {
                SamError::IaiSen(sen_cost).submit(event_sink);
            }
        }
        match action {
            Ikishoten if !src.in_combat() => event_sink.error(EventError::InCombat),
            Namikiri if !src.has_own_status(OGI_READY) => {
                SamError::OgiRdy.submit(event_sink);
            }
            Shoha | Shoha2 if state.meditation != 3 => SamError::Meditation.submit(event_sink),
            Hagakure if state.sen.count() == 0 => SamError::HagaSen.submit(event_sink),
            // really ugly if the if guard is in the match branch lol
            KaeshiHiganbana | KaeshiGoken | KaeshiSetsugekka | KaeshiNamikiri => {
                if !state.combos.check_kaeshi_for(action) {
                    SamError::Kaeshi(match action {
                        KaeshiHiganbana => Higanbana,
                        KaeshiGoken => TenkaGoken,
                        KaeshiSetsugekka => Midare,
                        KaeshiNamikiri => Namikiri,
                        _ => unreachable!(),
                    })
                    .submit(event_sink)
                }
            }
            _ => (),
        }

        CastInitInfo {
            gcd,
            lock,
            snap,
            cd,
        }
    }

    fn set_cd(state: &mut Self::State, group: Self::CdGroup, cooldown: u32, charges: u8) {
        state.cds.apply(group, cooldown, charges);
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
            this.actors_for_action(Some(Faction::Enemy), t)
                .map(|a| a.id())
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
                    s.sen.grant_ka();
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                    s.sen.grant_ka();
                    p.apply_status(FUKA, 1, this_id, dl);
                    true
                } else {
                    false
                };
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
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
                let mut c = EventCascade::new(dl, 1);
                p.damage_ch(860, first, c.next());
                for t in other {
                    p.damage_ch(215, t, c.next());
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
/// A custom error for Samurai actions.
pub enum SamError {
    /// Not executed following the specified iaijutsu.
    Kaeshi(SamAction),
    /// Not enough Kenki gauge.
    Kenki(u8),
    /// Incorrect number of Sen for the iaijutsu.
    IaiSen(u8),
    /// No Sen for Hagakure.
    HagaSen,
    /// Not enough stacks of Meditation.
    Meditation,
    /// Not under the effect Ogi Namikiri Ready.
    OgiRdy,
}
impl SamError {
    /// Submits the cast error into the [`EventProxy`].
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
#[var_consts {
    /// Returns `true` if the action uses tsubame gaeshi.
    pub const tsubame
    /// Returns the base GCD recast time, or `None` if the action is not a gcd.
    pub const gcd: ScaleTime?
    pub const skill for gcd = ScaleTime::skill(2500)
    /// Returns the base milliseconds the action takes to cast.
    pub const cast: ScaleTime = ScaleTime::zero()
    /// Returns the human friendly name of the action.
    pub const name: &'static str
    /// Returns the cooldown of the skill in milliseconds.
    pub const cooldown: u32 = 0
    /// Returns the number of charges a skill has, or `1` if it is a single charge skill.
    pub const cd_charges: u8 = 1
    /// Returns the delay in milliseconds for the damage/statuses to be applied.
    pub const effect_delay: u32 = 0
    /// Returns the kenki cost of the specified action.
    pub const kenki_cost: u8 = 0
    /// Returns the number of sen needed to perform the iaijutsu.
    pub const sen_cost: u8?
}]
#[allow(missing_docs)] // no reason to document the variants.
/// An action specific to the Reaper job.
pub enum SamAction {
    #[skill]
    #[name = "Hakaze"]
    Hakaze,
    #[skill]
    #[name = "Jinpu"]
    Jinpu,
    #[skill]
    #[name = "Shifu"]
    Shifu,
    #[skill]
    #[name = "Yukikaze"]
    Yukikaze,
    #[skill]
    #[name = "Gekko"]
    Gekko,
    #[skill]
    #[name = "Kasha"]
    Kasha,
    #[skill]
    #[name = "Fuga"]
    Fuga,
    #[skill]
    #[name = "Mangetsu"]
    Mangetsu,
    #[skill]
    #[name = "Oka"]
    Oka,
    #[skill]
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
    // #[skill]
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
    // #[skill]
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
    #[skill]
    #[cast = ScaleTime::skill(1300)]
    #[name = "Ogi Namikiri"]
    Namikiri,
    #[skill]
    #[cast = ScaleTime::skill(1300)]
    #[name = "Higanbana"]
    #[sen_cost = 1]
    Higanbana,
    #[skill]
    #[cast = ScaleTime::skill(1300)]
    #[name = "Tenka Goken"]
    #[sen_cost = 2]
    TenkaGoken,
    #[skill]
    #[cast = ScaleTime::skill(1300)]
    #[name = "Midare Setsugekka"]
    #[sen_cost = 3]
    Midare,
    #[skill]
    #[name = "Kaeshi: Higanbana"]
    #[cooldown = 60000]
    #[cd_charges = 2]
    #[tsubame]
    KaeshiHiganbana,
    #[skill]
    #[name = "Kaeshi: Goken"]
    #[cooldown = 60000]
    #[cd_charges = 2]
    #[tsubame]
    KaeshiGoken,
    #[skill]
    #[name = "Kaeshi: Setsugekka"]
    #[cooldown = 60000]
    #[cd_charges = 2]
    #[tsubame]
    KaeshiSetsugekka,
    #[skill]
    #[name = "Kaeshi: Namikiri"]
    KaeshiNamikiri,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The state of the Samurai job gauges, cooldowns, and combos.
pub struct SamState {
    /// The cooldowns for Samurai actions.
    pub cds: SamCds,
    /// The combos for Samurai.
    pub combos: SamCombos,
    /// The Sen gauge.
    pub sen: Sen,
    /// The stacks of Meditation.
    pub meditation: GaugeU8<3>,
    /// The Kenki gauge.
    pub kenki: GaugeU8<100>,
}

impl JobState for SamState {
    fn advance(&mut self, time: u32) {
        self.combos.advance(time);
        self.cds.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The combos for Samurai.
pub struct SamCombos {
    /// The main combo.
    ///
    /// Includes the Kasha, Gekko, Yukikaze, Mangetsu, and Oka combos.
    pub main: ComboState<MainCombo>,
    /// The combo for Tsubame-gaeshi.
    pub kaeshi: ComboState<KaeshiCombo>,
}

impl SamCombos {
    /// Checks that the main combo prerequisite is met for a certain action.
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

    /// Checks that the kaeshi combo prerequisite is met for a certain action.
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

    /// Advances the combos forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        self.main.advance(time);
        self.kaeshi.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The possible states the main combo can be in.
pub enum MainCombo {
    /// Combo Action: Hakaze is met.
    Hakaze,
    /// Combo Action: Shifu is met.
    Shifu,
    /// Combo Action: Jinpu is met.
    Jinpu,
    /// Combo Action: Fuko is met.
    Fuko,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The possible states the Tsubame-gaeshi combo can be in.
pub enum KaeshiCombo {
    /// Able to cast Kaeshi: Higanbana.
    Higanbana,
    /// Able to cast Kaeshi: Goken.
    Goken,
    /// Able to cast Kaeshi: Setsugekka.
    Setsugekka,
    /// Able to cast Kaeshi: Namikiri.
    Namikiri,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The Sen gauge.
pub struct Sen {
    bits: u8,
}
impl Sen {
    const SETSU: u8 = 1 << 0;
    const GETSU: u8 = 1 << 1;
    const KA: u8 = 1 << 2;

    /// Grants Setsu.
    pub fn grant_setsu(&mut self) {
        self.bits |= Self::SETSU
    }
    /// Grants Getsu.
    pub fn grant_getsu(&mut self) {
        self.bits |= Self::GETSU
    }
    /// Grants Ka.
    pub fn grant_ka(&mut self) {
        self.bits |= Self::KA
    }
    /// Returns the number of Sen present.
    pub fn count(&self) -> u8 {
        self.bits.count_ones() as u8
    }
    /// Clears the Sen gauge.
    pub fn clear(&mut self) {
        self.bits = 0;
    }
}

job_cd_struct! {
    SamAction =>

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Clone, Debug, Default)]
    /// The active cooldowns for Samurai actions.
    pub SamCds

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    /// The various cooldown groups a Samurai action can be part of.
    pub SamCdGroup

    "Shinten"
    shinten Shinten: Shinten;
    "Kyuten"
    kyuten Kyuten: Kyuten;
    "Gyoten"
    gyoten Gyoten: Gyoten;
    "Yaten"
    yaten Yaten: Yaten;
    "Hagakure"
    hagakure Hagakure: Hagakure;
    "Guren and Senei"
    senei Senei: Senei Guren;
    "Ikishoten"
    ikishoten Ikishoten: Ikishoten;
    "Meikyo Shisui"
    meikyo Meikyo: Meikyo;
    "Shoha and Shoha II"
    shoha Shoha: Shoha Shoha2;
    "Tsubame-gaeshi"
    tsubame Tsubame: KaeshiHiganbana KaeshiGoken KaeshiSetsugekka;
}
