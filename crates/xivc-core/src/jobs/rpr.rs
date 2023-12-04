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
        status::{consume_status, StatusEffect, StatusEventExt},
        ActionTargetting, Actor, CastSnapEvent, DamageEventExt, EventError, EventProxy, Faction,
        Positional, World,
    },
};

#[derive(Clone, Copy, Debug, Default)]
pub struct RprJob;

pub const DEATHS_DESIGN: StatusEffect = status_effect!(
    "Death's Design" 30000 { damage { in = 110 / 100 } }
);
pub const ARCANE_CIRCLE: StatusEffect = status_effect!(
    "Arcane Circle" 20000 { damage { out = 103 / 100 } }
);
pub const CIRCLE_SACRIFICE: StatusEffect = status_effect!("Circle of Sacrifice" 5000);
pub const BLOODSOWN_SACRIFICE: StatusEffect = status_effect!("Bloodsown Sacrifice" 6000);
pub const IMMORTAL_SACRIFICE: StatusEffect = status_effect!("Immortal Sacrifice" 30000);
pub const SOUL_REAVER: StatusEffect = status_effect!("Soul Reaver" 30000);
pub const SOULSOW: StatusEffect = status_effect!("Soulsow" permanent);
pub const ENSHROUD: StatusEffect = status_effect!("Enshroud" 30000);
pub const ENHARPE: StatusEffect = status_effect!("Enhanced Harpe" 20000);
pub const ENGIBBET: StatusEffect = status_effect!("Enhanced Gibbet" 60000);
pub const ENGALLOWS: StatusEffect = status_effect!("Enhanced Gallows" 60000);
pub const ENVOID: StatusEffect = status_effect!("Enhanced Void Reaping" 30000);
pub const ENCROSS: StatusEffect = status_effect!("Enhanced Cross Reaping" 30000);

impl Job for RprJob {
    type Action = RprAction;
    type State = RprState;
    type Error = RprError;
    type Event = ();

    fn init_cast<'w, P: EventProxy, W: World>(
        ac: Self::Action,
        s: &mut Self::State,
        w: &'w W,
        this: &'w W::Actor<'w>,
        p: &mut P,
    ) {
        let stat = ac.speed_stat();

        let cast = match ac {
            Harpe if this.has_own_status(ENHARPE) => 0,
            Soulsow if !this.in_combat() => 0,
            _ => ac.cast(),
        };

        let snap = s.cds.set_cast_lock(p, w.duration_info(), cast, 600, stat);

        if ac.gcd() {
            s.cds.set_gcd(p, w.duration_info(), ac.gcd_base(), stat);
        }

        // don't need to worry about gcd scaling cooldowns
        if !s.cds.job.available(ac, ac.cooldown(), ac.cd_charges()) {
            p.error(EventError::Cooldown(ac.into()));
        }
        if ac.cooldown() > 0 {
            s.cds.job.apply(ac, ac.cooldown(), ac.cd_charges());
        }

        // check gauge errors
        use RprAction::*;
        if s.lemure_shroud > 0 && ac.enshroud_invalid() {
            RprError::Enshroud(ac).submit(p)
        }
        match ac {
            BloodStalk | GrimSwathe => {
                if s.soul < 50 {
                    RprError::Soul(50).submit(p);
                }
            }
            UnveiledGibbet => {
                if s.soul < 50 {
                    RprError::Soul(50).submit(p);
                }
                if !this.has_own_status(ENGIBBET) {
                    RprError::UnvGibbet.submit(p);
                }
            }
            UnveiledGallows => {
                if s.soul < 50 {
                    RprError::Soul(50).submit(p);
                }
                if !this.has_own_status(ENGALLOWS) {
                    RprError::UnvGallows.submit(p);
                }
            }
            Gibbet | Gallows | Guillotine => {
                if !this.has_own_status(SOUL_REAVER) {
                    RprError::SoulReaver.submit(p);
                }
            }
            Enshroud if s.shroud < 50 => {
                RprError::Shroud(50).submit(p);
            }
            HarvestMoon if !this.has_own_status(SOULSOW) => {
                RprError::Soulsow.submit(p);
            }
            VoidReaping | CrossReaping | GrimReaping | Communio => {
                if s.lemure_shroud == 0 {
                    RprError::Lemure(1).submit(p);
                }
            }
            LemuresSlice | LemuresScythe => {
                if s.void_shroud < 2 {
                    RprError::Void(2).submit(p);
                }
            }
            PlentifulHarvest => {
                if !this.has_own_status(IMMORTAL_SACRIFICE) {
                    RprError::Sacrifice.submit(p);
                }
                if this.has_own_status(BLOODSOWN_SACRIFICE) {
                    RprError::Bloodsown.submit(p);
                }
            }
            _ => (),
        }

        p.event(CastSnapEvent::new(ac).into(), snap);
    }

    fn cast_snap<'w, P: EventProxy, W: World>(
        ac: Self::Action,
        s: &mut Self::State,
        _: &'w W,
        this: &'w W::Actor<'w>,
        p: &mut P,
    ) {
        use RprAction::*;

        let target_enemy = |t: ActionTargetting| {
            this.actors_for_action(t)
                .filter(|t| t.faction() == Faction::Enemy)
                .map(actor_id)
        };

        let dl = ac.effect_delay();

        let this_id = this.id();

        if ac.gcd() {
            match ac {
                Gibbet | Gallows | Guillotine => {
                    p.remove_stacks(SOUL_REAVER, 1, this_id, 0);
                }
                _ => {
                    p.remove_status(SOUL_REAVER, this_id, 0);
                }
            }
        }

        #[allow(clippy::match_single_binding)]
        match ac {
            Slice => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.combos.main.set(MainCombo::Slice);
                s.soul += 10;
                p.damage(320, t, dl);
            }
            WaxingSlice => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                let combo = if s.combos.main.check(MainCombo::Slice) {
                    s.combos.main.set(MainCombo::Waxing);
                    s.soul += 10;
                    true
                } else {
                    s.combos.main.reset();
                    false
                };
                p.damage(combo_pot(160, 400, combo), t, dl);
            }
            ShadowOfDeath => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                p.apply_or_extend_status(DEATHS_DESIGN, 1, 2, t, dl);
                p.damage(300, t, dl);
            }
            Harpe => {
                let t = need_target!(target_enemy(RANGED).next(), p);
                p.damage(300, t, dl);
            }
            // it doesn't really matter
            HellsIngress | HellsEgress => {
                p.apply_status(ENHARPE, 1, this_id, dl);
            }
            SpinningScythe => {
                let mut c = EventCascade::new(dl);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    p.damage(140, t, c.next());
                }
                if hit {
                    s.soul += 10;
                    s.combos.main.set(MainCombo::Spinning);
                } else {
                    s.combos.main.reset();
                }
            }
            InfernalSlice => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                let combo = if s.combos.main.check(MainCombo::Waxing) {
                    s.soul += 10;
                    true
                } else {
                    false
                };
                s.combos.main.reset();
                p.damage(combo_pot(180, 500, combo), t, dl);
            }
            WhorlOfDeath => {
                let mut c = EventCascade::new(dl);
                for t in target_enemy(CIRCLE) {
                    let dl = c.next();
                    p.apply_or_extend_status(DEATHS_DESIGN, 1, 2, t, dl);
                    p.damage(100, t, dl);
                }
            }
            NightmareScythe => {
                let combo = s.combos.main.check(MainCombo::Spinning);
                s.combos.main.reset();
                let mut c = EventCascade::new(dl);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    p.damage(combo_pot(120, 180, combo), t, c.next());
                }
                if hit {
                    s.soul += 10;
                }
            }
            BloodStalk => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.soul -= 50;
                p.damage(340, t, dl);
                // almost certain it is no delay of the soul reaver stack
                p.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            GrimSwathe => {
                let mut c = EventCascade::new(dl);
                for t in need_target!(target_enemy(CONE), p, uaoe) {
                    p.damage(140, t, c.next());
                }
                s.soul -= 50;
                p.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            SoulSlice => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.soul -= 50;
                p.damage(460, t, dl);
            }
            SoulScythe => {
                let mut c = EventCascade::new(dl);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    p.damage(180, t, c.next());
                }
                if hit {
                    s.soul += 50;
                }
            }
            Gibbet => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                let en = consume_status(this, p, ENGIBBET, 0);
                s.shroud += 10;
                let pos = this.check_positional(Positional::Flank, t);
                p.damage(combo_pos_pot(400, 460, 460, 520, en, pos), t, dl);
                p.apply_status(ENGALLOWS, 1, this_id, 0);
            }
            Gallows => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                let en = consume_status(this, p, ENGALLOWS, 0);
                s.shroud += 10;
                let pos = this.check_positional(Positional::Rear, t);
                p.damage(combo_pos_pot(400, 460, 460, 520, en, pos), t, dl);
                p.apply_status(ENGIBBET, 1, this_id, 0);
            }
            Guillotine => {
                let mut c = EventCascade::new(dl);
                for t in need_target!(target_enemy(CONE), p, uaoe) {
                    p.damage(200, t, c.next());
                }
                s.shroud += 10;
            }
            ArcaneCircle => {
                let mut c = EventCascade::new(dl);
                for t in this
                    .actors_for_action(ActionTargetting::circle(30))
                    .filter(|v| v.faction() == Faction::Friendly)
                    .map(actor_id)
                {
                    let dl = c.next();
                    p.apply_status(ARCANE_CIRCLE, 1, t, dl);
                    p.apply_status(CIRCLE_SACRIFICE, 1, t, dl);
                }
                p.apply_status(BLOODSOWN_SACRIFICE, 1, this_id, dl);
                // TODO: i'm lazy and i'll fix this eventually
                // but this won't ever really be observable
                // unless you specifically want less stacks lol
                p.apply_status(IMMORTAL_SACRIFICE, 8, this_id, dl);
            }
            Gluttony => {
                let (f, o) = need_target!(target_enemy(TARGET_CIRCLE), p, aoe);
                s.soul -= 50;
                let mut c = EventCascade::new(dl);
                p.damage(520, f, c.next());
                for t in o {
                    p.damage(390, t, c.next());
                }
                p.apply_status(SOUL_REAVER, 2, this_id, 0);
            }
            Enshroud => {
                s.shroud -= 50;
                p.apply_status(ENSHROUD, 1, this_id, 0);
                s.lemure_shroud.set_max();
            }
            Soulsow => {
                p.apply_status(SOULSOW, 1, this_id, 0);
            }
            PlentifulHarvest => {
                let (first, other) = need_target!(target_enemy(ActionTargetting::line(15)), p, aoe);
                let stacks = this
                    .get_own_status(IMMORTAL_SACRIFICE)
                    .map(|v| v.stack)
                    .unwrap_or_default();
                s.shroud += 50;
                let mut c = EventCascade::new(dl);
                p.damage(680 + 40 * stacks as u16, first, c.next());
                for t in other {
                    // 60% less
                    p.damage(272 + 16 * stacks as u16, t, c.next());
                }
                p.remove_status(IMMORTAL_SACRIFICE, this_id, 0);
            }
            Communio => {
                let (f, o) = need_target!(target_enemy(TARGET_CIRCLE), p, aoe);
                let mut c = EventCascade::new(dl);
                p.damage(1100, f, c.next());
                for t in o {
                    p.damage(440, t, c.next());
                }
                s.lemure_shroud.clear();
                s.void_shroud.clear();
                p.remove_status(ENSHROUD, this_id, 0);
            }
            UnveiledGibbet => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.soul -= 50;
                p.damage(400, t, dl);
                p.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            UnveiledGallows => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.soul -= 50;
                p.damage(400, t, dl);
                p.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            VoidReaping => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                let en = consume_status(this, p, ENVOID, 0);
                s.lemure_shroud -= 1;
                if s.lemure_shroud == 0 {
                    s.void_shroud.clear();
                    p.remove_status(ENSHROUD, this_id, 0);
                } else {
                    p.apply_status(ENCROSS, 1, this_id, 0);
                    s.void_shroud += 1;
                }
                p.damage(combo_pot(460, 520, en), t, dl);
            }
            CrossReaping => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                let en = consume_status(this, p, ENCROSS, 0);
                s.lemure_shroud -= 1;
                if s.lemure_shroud == 0 {
                    s.void_shroud.clear();
                    p.remove_status(ENSHROUD, this_id, 0);
                } else {
                    p.apply_status(ENVOID, 1, this_id, 0);
                    s.void_shroud += 1;
                }
                p.damage(combo_pot(460, 520, en), t, dl);
            }
            GrimReaping => {
                s.lemure_shroud -= 1;
                if s.lemure_shroud == 0 {
                    s.void_shroud.clear();
                    p.remove_status(ENSHROUD, this_id, 0);
                } else {
                    s.void_shroud += 1;
                }
                let mut c = EventCascade::new(dl);
                for t in need_target!(target_enemy(CONE), p, uaoe) {
                    p.damage(200, t, c.next());
                }
            }
            HarvestMoon => {
                let (f, o) = need_target!(target_enemy(TARGET_CIRCLE), p, aoe);
                consume_status(this, p, SOULSOW, 0);
                let mut c = EventCascade::new(dl);
                p.damage(600, f, c.next());
                for t in o {
                    p.damage(300, t, c.next());
                }
            }
            LemuresSlice => {
                let t = need_target!(target_enemy(MELEE).next(), p);
                s.void_shroud -= 2;
                p.damage(240, t, dl);
            }
            LemuresScythe => {
                let mut c = EventCascade::new(dl);
                for t in need_target!(target_enemy(CONE), p, uaoe) {
                    p.damage(100, t, c.next());
                }
                s.void_shroud -= 2;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RprError {
    Soul(u8),
    Shroud(u8),
    Soulsow,
    Lemure(u8),
    Void(u8),
    Sacrifice,
    Bloodsown,
    SoulReaver,
    UnvGibbet,
    UnvGallows,
    Enshroud(RprAction),
}
impl RprError {
    pub fn submit(self, p: &mut impl EventProxy) {
        p.error(self.into())
    }
}

impl From<RprError> for EventError {
    fn from(value: RprError) -> Self {
        Self::Job(value.into())
    }
}

impl Display for RprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Soul(v) => write!(f, "Not enough Soul Gauge, needed at least {}.", v),
            Self::Shroud(v) => write!(f, "Not enough Shroud Gauge, needed at least {}.", v),
            Self::Soulsow => write!(f, "Not under the effect of '{}'.", SOULSOW.name),
            Self::Lemure(v) => write!(
                f,
                "Not enough stacks of Lemure Shroud, needed at least {}.",
                v
            ),
            Self::Void(v) => write!(
                f,
                "Not enough stacks of Void Shroud, needed at least {}.",
                v
            ),
            Self::Sacrifice => write!(f, "Not under the effect of '{}'.", IMMORTAL_SACRIFICE.name),
            Self::Bloodsown => write!(
                f,
                "Cannot use action '{}' under the effect of '{}'.",
                RprAction::PlentifulHarvest.name(),
                BLOODSOWN_SACRIFICE.name,
            ),
            Self::SoulReaver => write!(f, "Not under the effect of '{}'.", SOUL_REAVER.name),
            Self::UnvGibbet => write!(f, "Not under the effect of '{}'.", ENGIBBET.name),
            Self::UnvGallows => write!(f, "Not under the effect of '{}'.", ENGALLOWS.name),
            Self::Enshroud(ac) => write!(
                f,
                "Cannot use action '{}' under the effect of '{}'.",
                ac.name(),
                ENSHROUD.name,
            ),
        }
    }
}

const MELEE: ActionTargetting = ActionTargetting::single(3);
const RANGED: ActionTargetting = ActionTargetting::single(25);
const CIRCLE: ActionTargetting = ActionTargetting::circle(5);
const CONE: ActionTargetting = ActionTargetting::cone(8, 180);
const TARGET_CIRCLE: ActionTargetting = ActionTargetting::target_circle(5, 25);

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
    /// Returns `true` if the action is a spell.
    pub spell
)]
#[flag(
    /// Returns `true` if the action cannot be used during Enshroud.
    pub enshroud_invalid
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
    /// Returns the base GCD length of the skill in milliseconds.
    pub const gcd_base: u16 = 2500
)]
#[property(
    /// Returns the number of charges a skill has, or 1 if it is a single charge skill.
    pub const cd_charges: u8 = 1
)]
#[property(
    /// Returns the delay in milliseconds for the damage/statuses to be applied.
    pub const effect_delay: u32 = 0
)]
pub enum RprAction {
    #[gcd]
    #[enshroud_invalid]
    #[name = "Slice"]
    Slice,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Waxing Slice"]
    WaxingSlice,
    #[gcd]
    #[name = "Shadow of Death"]
    ShadowOfDeath,
    #[gcd]
    #[spell]
    #[cast = 1300]
    #[name = "Harpe"]
    Harpe,
    #[name = "Hell's Ingress"]
    HellsIngress,
    #[name = "Hell's Egress"]
    HellsEgress,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Spinning Scythe"]
    SpinningScythe,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Infernal Slice"]
    InfernalSlice,
    #[gcd]
    #[name = "Whorl of Death"]
    WhorlOfDeath,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Nightmare Scythe"]
    NightmareScythe,
    #[enshroud_invalid]
    #[name = "Blood Stalk"]
    BloodStalk,
    #[enshroud_invalid]
    #[name = "Grim Swathe"]
    GrimSwathe,
    #[gcd]
    #[enshroud_invalid]
    #[cooldown = 30000]
    #[cd_charges = 2]
    #[name = "Soul Slice"]
    SoulSlice,
    #[gcd]
    #[enshroud_invalid]
    #[cooldown = 30000]
    #[cd_charges = 2]
    #[name = "Soul Scythe"]
    SoulScythe,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Gibbet"]
    Gibbet,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Gallows"]
    Gallows,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Guillotine"]
    Guillotine,
    #[cooldown = 120000]
    #[name = "Arcane Circle"]
    ArcaneCircle,
    #[enshroud_invalid]
    #[name = "Gluttony"]
    Gluttony,
    #[enshroud_invalid]
    #[cooldown = 15000]
    #[name = "Enshroud"]
    Enshroud,
    #[gcd]
    #[spell]
    #[cast = 5000]
    #[name = "Soulsow"]
    Soulsow,
    #[gcd]
    #[enshroud_invalid]
    #[name = "Plentiful Harvest"]
    PlentifulHarvest,
    #[gcd]
    #[spell]
    #[cast = 1300]
    #[name = "Communio"]
    Communio,
    #[enshroud_invalid]
    #[name = "Unveiled Gibbet"]
    UnveiledGibbet,
    #[enshroud_invalid]
    #[name = "Unveiled Gallows"]
    UnveiledGallows,
    // Regress,
    #[gcd]
    #[gcd_base = 1500]
    #[name = "Void Reaping"]
    VoidReaping,
    #[gcd]
    #[gcd_base = 1500]
    #[name = "Cross Reaping"]
    CrossReaping,
    #[gcd]
    #[gcd_base = 1500]
    #[name = "Grim Reaping"]
    GrimReaping,
    #[gcd]
    #[name = "Harvest Moon"]
    HarvestMoon,
    #[cooldown = 1000]
    #[name = "Lemure's Slice"]
    LemuresSlice,
    #[cooldown = 1000]
    #[name = "Lemure's Scythe"]
    LemuresScythe,
}

impl RprAction {
    pub const fn cd_speed_stat(&self) -> Option<SpeedStat> {
        if !self.gcd() || self.cd_charges() > 1 {
            return None;
        }
        Some(if self.spell() {
            SpeedStat::SpellSpeed
        } else {
            SpeedStat::SkillSpeed
        })
    }

    pub const fn speed_stat(&self) -> Option<SpeedStat> {
        if self.gcd_base() == 1500 || !self.gcd() {
            return None;
        }
        Some(if self.spell() {
            SpeedStat::SpellSpeed
        } else {
            SpeedStat::SkillSpeed
        })
    }
}

impl JobAction for RprAction {
    fn action_type(&self) -> ActionType {
        if self.gcd() {
            if self.spell() {
                ActionType::Spell
            } else {
                ActionType::Weaponskill
            }
        } else {
            ActionType::Ability
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct RprState {
    pub cds: JobCds<RprCds>,
    pub combos: RprCombos,
    pub soul: GaugeU8<100>,
    pub shroud: GaugeU8<100>,
    pub lemure_shroud: GaugeU8<5>,
    pub void_shroud: GaugeU8<5>,
}

impl JobState for RprState {
    fn advance(&mut self, time: u32) {
        self.cds.advance(time);
        self.cds.job.advance(time);
        self.combos.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct RprCombos {
    pub main: ComboState<MainCombo>,
}

impl RprCombos {
    pub fn check_main_for(&self, action: RprAction) -> bool {
        let c = match action {
            RprAction::WaxingSlice => MainCombo::Slice,
            RprAction::InfernalSlice => MainCombo::Waxing,
            RprAction::NightmareScythe => MainCombo::Spinning,
            _ => return true,
        };
        self.main.check(c)
    }

    pub fn advance(&mut self, time: u32) {
        self.main.advance(time);
    }
}

// lmfao
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MainCombo {
    Slice,
    Waxing,
    Spinning,
}

job_cd_struct! {
    RprAction =>
    
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Clone, Debug, Default)]
    pub RprCds
    
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    pub RprCdGroup
    
    hells Hells: HellsIngress HellsEgress;
    reaver Reaver: BloodStalk GrimSwathe UnveiledGibbet UnveiledGallows;
    soul Soul: SoulSlice SoulScythe;
    circle Circle: ArcaneCircle;
    gluttony Gluttony: Gluttony;
    enshroud Enshroud: Enshroud;
    lemures Lemures: LemuresSlice LemuresScythe;
}

// this job is wild
// job_cd_struct! {
//     #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
//     #[derive(Clone, Debug, Default)]
//     pub struct RprCds for RprAction {
//         hells: HellsIngress | HellsEgress,
//         reaver: BloodStalk | GrimSwathe | UnveiledGibbet | UnveiledGallows,
//         soul: SoulSlice | SoulScythe,
//         circle: ArcaneCircle,
//         gluttony: Gluttony,
//         enshroud: Enshroud,
//         lemure: LemuresSlice | LemuresScythe
//     }
// }
