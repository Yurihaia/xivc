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
        status::{consume_status, StatusEffect, StatusEventExt},
        ActionTargetting, Actor, DamageEvent, DamageEventExt, EventError, EventProxy, Faction,
        Positional, World,
    },
};

use super::CastInitInfo;

/// The [`Job`] struct for Reaper.
#[derive(Clone, Copy, Debug, Default)]
pub struct RprJob;

/// The status effect "Death's Design".
pub const DEATHS_DESIGN: StatusEffect = status_effect!(
    "Death's Design" 30000 { damage { in = 110 / 100 } }
);
/// The status effect "Arcane Circle".
pub const ARCANE_CIRCLE: StatusEffect = status_effect!(
    "Arcane Circle" 20000 { damage { out = 103 / 100 } }
);
/// The status effect "Circle of Sacrifice".
pub const CIRCLE_SACRIFICE: StatusEffect = status_effect!("Circle of Sacrifice" 5000);
/// The status effect "Bloodsown Sacrifice".
pub const BLOODSOWN_SACRIFICE: StatusEffect = status_effect!("Bloodsown Sacrifice" 6000);
/// The status effect "Immortal Sacrifice".
pub const IMMORTAL_SACRIFICE: StatusEffect = status_effect!("Immortal Sacrifice" 30000);
/// The status effect "Soul Reaver".
pub const SOUL_REAVER: StatusEffect = status_effect!("Soul Reaver" 30000);
/// The status effect "Soulsow".
pub const SOULSOW: StatusEffect = status_effect!("Soulsow" permanent);
/// The status effect "Enshroud".
pub const ENSHROUD: StatusEffect = status_effect!("Enshroud" 30000);
/// The status effect "Enhanced Harpe".
pub const ENHARPE: StatusEffect = status_effect!("Enhanced Harpe" 20000);
/// The status effect "Enhanced Gibbet".
pub const ENGIBBET: StatusEffect = status_effect!("Enhanced Gibbet" 60000);
/// The status effect "Enhanced Gallows".
pub const ENGALLOWS: StatusEffect = status_effect!("Enhanced Gallows" 60000);
/// The status effect "Enhanced Void Reaping".
pub const ENVOID: StatusEffect = status_effect!("Enhanced Void Reaping" 30000);
/// The status effect "Enhanced Cross Reaping".
pub const ENCROSS: StatusEffect = status_effect!("Enhanced Cross Reaping" 30000);

impl Job for RprJob {
    type Action = RprAction;
    type State = RprState;
    type CastError = RprError;
    type Event = ();
    type CdGroup = RprCdGroup;

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

        use RprAction::*;
        if state.lemure_shroud > 0 && action.enshroud_invalid() {
            RprError::Enshroud(action).submit(event_sink);
        }
        match action {
            BloodStalk | GrimSwathe => {
                if state.soul < 50 {
                    RprError::Soul(50).submit(event_sink);
                }
            }
            UnveiledGibbet => {
                if state.soul < 50 {
                    RprError::Soul(50).submit(event_sink);
                }
                if !src.has_own_status(ENGIBBET) {
                    RprError::UnvGibbet.submit(event_sink);
                }
            }
            UnveiledGallows => {
                if state.soul < 50 {
                    RprError::Soul(50).submit(event_sink);
                }
                if !src.has_own_status(ENGALLOWS) {
                    RprError::UnvGallows.submit(event_sink);
                }
            }
            Gibbet | Gallows | Guillotine => {
                if !src.has_own_status(SOUL_REAVER) {
                    RprError::SoulReaver.submit(event_sink);
                }
            }
            Enshroud if state.shroud < 50 => {
                RprError::Shroud(50).submit(event_sink);
            }
            HarvestMoon if !src.has_own_status(SOULSOW) => {
                RprError::Soulsow.submit(event_sink);
            }
            VoidReaping | CrossReaping | GrimReaping | Communio => {
                if state.lemure_shroud == 0 {
                    RprError::Lemure(1).submit(event_sink);
                }
            }
            LemuresSlice | LemuresScythe => {
                if state.void_shroud < 2 {
                    RprError::Void(2).submit(event_sink);
                }
            }
            PlentifulHarvest => {
                if !src.has_own_status(IMMORTAL_SACRIFICE) {
                    RprError::Sacrifice.submit(event_sink);
                }
                if src.has_own_status(BLOODSOWN_SACRIFICE) {
                    RprError::Bloodsown.submit(event_sink);
                }
            }
            _ => (),
        };

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

    fn cast_snap<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &mut Self::State,
        _: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    ) {
        use RprAction::*;

        let target_enemy = |t: ActionTargetting| {
            src.actors_for_action(Some(Faction::Enemy), t)
                .map(|a| a.id())
        };

        let dl = action.effect_delay();

        let this_id = src.id();

        if action.gcd().is_some() {
            match action {
                Gibbet | Gallows | Guillotine => {
                    event_sink.remove_stacks(SOUL_REAVER, 1, this_id, 0);
                }
                _ => {
                    event_sink.remove_status(SOUL_REAVER, this_id, 0);
                }
            }
        }

        #[allow(clippy::match_single_binding)]
        match action {
            Slice => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                state.combos.main.set(MainCombo::Slice);
                state.soul += 10;
                event_sink.damage(DamageEvent::new(320, t).slashing(), dl);
            }
            WaxingSlice => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                let combo = if state.combos.main.check(MainCombo::Slice) {
                    state.combos.main.set(MainCombo::Waxing);
                    state.soul += 10;
                    true
                } else {
                    state.combos.main.reset();
                    false
                };
                event_sink.damage(
                    DamageEvent::new(combo_pot(160, 400, combo), t).slashing(),
                    dl,
                );
            }
            ShadowOfDeath => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                event_sink.apply_or_extend_status(DEATHS_DESIGN, 1, 2, t, dl);
                event_sink.damage(DamageEvent::new(300, t).slashing(), dl);
            }
            Harpe => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                event_sink.damage(DamageEvent::new(300, t).slashing(), dl);
            }
            // it doesn't really matter
            HellsIngress | HellsEgress => {
                event_sink.apply_status(ENHARPE, 1, this_id, dl);
            }
            SpinningScythe => {
                let mut c = EventCascade::new(dl, 1);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    event_sink.damage(DamageEvent::new(140, t).slashing(), c.next());
                }
                if hit {
                    state.soul += 10;
                    state.combos.main.set(MainCombo::Spinning);
                } else {
                    state.combos.main.reset();
                }
            }
            InfernalSlice => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                let combo = if state.combos.main.check(MainCombo::Waxing) {
                    state.soul += 10;
                    true
                } else {
                    false
                };
                state.combos.main.reset();
                event_sink.damage(
                    DamageEvent::new(combo_pot(180, 500, combo), t).slashing(),
                    dl,
                );
            }
            WhorlOfDeath => {
                let mut c = EventCascade::new(dl, 1);
                for t in target_enemy(CIRCLE) {
                    let dl = c.next();
                    event_sink.apply_or_extend_status(DEATHS_DESIGN, 1, 2, t, dl);
                    event_sink.damage(DamageEvent::new(100, t).slashing(), dl);
                }
            }
            NightmareScythe => {
                let combo = state.combos.main.check(MainCombo::Spinning);
                state.combos.main.reset();
                let mut c = EventCascade::new(dl, 1);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    event_sink.damage(
                        DamageEvent::new(combo_pot(120, 180, combo), t).slashing(),
                        c.next(),
                    );
                }
                if hit {
                    state.soul += 10;
                }
            }
            BloodStalk => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                state.soul -= 50;
                event_sink.damage(DamageEvent::new(340, t).slashing(), dl);
                // almost certain it is no delay of the soul reaver stack
                event_sink.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            GrimSwathe => {
                let mut c = EventCascade::new(dl, 1);
                for t in need_target!(target_enemy(CONE), event_sink, uaoe) {
                    event_sink.damage(DamageEvent::new(140, t).slashing(), c.next());
                }
                state.soul -= 50;
                event_sink.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            SoulSlice => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                state.soul -= 50;
                event_sink.damage(DamageEvent::new(460, t).slashing(), dl);
            }
            SoulScythe => {
                let mut c = EventCascade::new(dl, 1);
                let mut hit = false;
                for t in target_enemy(CIRCLE) {
                    hit = true;
                    event_sink.damage(DamageEvent::new(180, t).slashing(), c.next());
                }
                if hit {
                    state.soul += 50;
                }
            }
            Gibbet => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                let en = consume_status(src, event_sink, ENGIBBET, 0);
                state.shroud += 10;
                let pos = src.check_positional(Positional::Flank, t);
                event_sink.damage(
                    DamageEvent::new(combo_pos_pot(400, 460, 460, 520, en, pos), t).slashing(),
                    dl,
                );
                event_sink.apply_status(ENGALLOWS, 1, this_id, 0);
            }
            Gallows => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                let en = consume_status(src, event_sink, ENGALLOWS, 0);
                state.shroud += 10;
                let pos = src.check_positional(Positional::Rear, t);
                event_sink.damage(
                    DamageEvent::new(combo_pos_pot(400, 460, 460, 520, en, pos), t).slashing(),
                    dl,
                );
                event_sink.apply_status(ENGIBBET, 1, this_id, 0);
            }
            Guillotine => {
                let mut c = EventCascade::new(dl, 1);
                for t in need_target!(target_enemy(CONE), event_sink, uaoe) {
                    event_sink.damage(DamageEvent::new(200, t).slashing(), c.next());
                }
                state.shroud += 10;
            }
            ArcaneCircle => {
                let mut c = EventCascade::new(dl, 3);
                for t in src
                    .actors_for_action(Some(Faction::Party), ActionTargetting::circle(30))
                    .map(|a| a.id())
                {
                    let dl = c.next();
                    event_sink.apply_status(ARCANE_CIRCLE, 1, t, dl);
                    event_sink.apply_status(CIRCLE_SACRIFICE, 1, t, dl);
                }
                event_sink.apply_status(BLOODSOWN_SACRIFICE, 1, this_id, dl);
                // TODO: i'm lazy and i'll fix this eventually
                // but this won't ever really be observable
                // unless you specifically want less stacks lol
                event_sink.apply_status(IMMORTAL_SACRIFICE, 8, this_id, dl);
            }
            Gluttony => {
                let (f, o) = need_target!(target_enemy(TARGET_CIRCLE), event_sink, aoe);
                state.soul -= 50;
                let mut c = EventCascade::new(dl, 1);
                event_sink.damage(DamageEvent::new(520, f).magical(), c.next());
                for t in o {
                    event_sink.damage(DamageEvent::new(390, t).magical(), c.next());
                }
                event_sink.apply_status(SOUL_REAVER, 2, this_id, 0);
            }
            Enshroud => {
                state.shroud -= 50;
                event_sink.apply_status(ENSHROUD, 1, this_id, 0);
                state.lemure_shroud.set_max();
            }
            Soulsow => {
                event_sink.apply_status(SOULSOW, 1, this_id, 0);
            }
            PlentifulHarvest => {
                let (first, other) =
                    need_target!(target_enemy(ActionTargetting::line(15)), event_sink, aoe);
                let stacks = src
                    .get_own_status(IMMORTAL_SACRIFICE)
                    .map(|v| v.stack)
                    .unwrap_or_default();
                state.shroud += 50;
                let mut c = EventCascade::new(dl, 1);
                event_sink.damage(
                    DamageEvent::new(680 + 40 * stacks as u16, first).slashing(),
                    c.next(),
                );
                for t in other {
                    // 60% less
                    event_sink.damage(
                        DamageEvent::new(272 + 16 * stacks as u16, t).slashing(),
                        c.next(),
                    );
                }
                event_sink.remove_status(IMMORTAL_SACRIFICE, this_id, 0);
            }
            Communio => {
                let (f, o) = need_target!(target_enemy(TARGET_CIRCLE), event_sink, aoe);
                let mut c = EventCascade::new(dl, 1);
                event_sink.damage(DamageEvent::new(1100, f).magical(), c.next());
                for t in o {
                    event_sink.damage(DamageEvent::new(440, t).magical(), c.next());
                }
                state.lemure_shroud.clear();
                state.void_shroud.clear();
                event_sink.remove_status(ENSHROUD, this_id, 0);
            }
            UnveiledGibbet => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                state.soul -= 50;
                event_sink.damage(DamageEvent::new(400, t).slashing(), dl);
                event_sink.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            UnveiledGallows => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                state.soul -= 50;
                event_sink.damage(DamageEvent::new(400, t).slashing(), dl);
                event_sink.apply_status(SOUL_REAVER, 1, this_id, 0);
            }
            VoidReaping => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                let en = consume_status(src, event_sink, ENVOID, 0);
                state.lemure_shroud -= 1;
                if state.lemure_shroud == 0 {
                    state.void_shroud.clear();
                    event_sink.remove_status(ENSHROUD, this_id, 0);
                } else {
                    event_sink.apply_status(ENCROSS, 1, this_id, 0);
                    state.void_shroud += 1;
                }
                event_sink.damage(DamageEvent::new(combo_pot(460, 520, en), t).slashing(), dl);
            }
            CrossReaping => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                let en = consume_status(src, event_sink, ENCROSS, 0);
                state.lemure_shroud -= 1;
                if state.lemure_shroud == 0 {
                    state.void_shroud.clear();
                    event_sink.remove_status(ENSHROUD, this_id, 0);
                } else {
                    event_sink.apply_status(ENVOID, 1, this_id, 0);
                    state.void_shroud += 1;
                }
                event_sink.damage(DamageEvent::new(combo_pot(460, 520, en), t).slashing(), dl);
            }
            GrimReaping => {
                state.lemure_shroud -= 1;
                if state.lemure_shroud == 0 {
                    state.void_shroud.clear();
                    event_sink.remove_status(ENSHROUD, this_id, 0);
                } else {
                    state.void_shroud += 1;
                }
                let mut c = EventCascade::new(dl, 1);
                for t in need_target!(target_enemy(CONE), event_sink, uaoe) {
                    event_sink.damage(DamageEvent::new(200, t).slashing(), c.next());
                }
            }
            HarvestMoon => {
                let (f, o) = need_target!(target_enemy(TARGET_CIRCLE), event_sink, aoe);
                consume_status(src, event_sink, SOULSOW, 0);
                let mut c = EventCascade::new(dl, 1);
                event_sink.damage(DamageEvent::new(600, f).magical(), c.next());
                for t in o {
                    event_sink.damage(DamageEvent::new(300, t).magical(), c.next());
                }
            }
            LemuresSlice => {
                let t = need_target!(target_enemy(MELEE).next(), event_sink);
                state.void_shroud -= 2;
                event_sink.damage(DamageEvent::new(240, t).slashing(), dl);
            }
            LemuresScythe => {
                let mut c = EventCascade::new(dl, 1);
                for t in need_target!(target_enemy(CONE), event_sink, uaoe) {
                    event_sink.damage(DamageEvent::new(100, t).slashing(), c.next());
                }
                state.void_shroud -= 2;
            }
        }
    }
}

/// A custom cast error for Reaper actions.
#[derive(Clone, Copy, Debug)]
pub enum RprError {
    /// Not enough Soul gauge.
    Soul(u8),
    /// Not enough Shroud Gauge.
    Shroud(u8),
    /// Not under the effect of Soulsow.
    Soulsow,
    /// Not enough stacks of Lemure Shroud.
    Lemure(u8),
    /// Not enough stacks of Void Shroud.
    Void(u8),
    /// Not under the effect of Immortal Sacrifice.
    Sacrifice,
    /// Under the effect of Bloodsown Circle.
    Bloodsown,
    /// Not under the effect of Soul Reaver.
    SoulReaver,
    /// Not under the effect of Enhanced Gibbet.
    UnvGibbet,
    /// Not under the effect of Enhanced Gallows.
    UnvGallows,
    /// Under the effect of Enshroud.
    Enshroud(RprAction),
}
impl RprError {
    /// Submits the cast error into the [`EventProxy`].
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
#[var_consts {
    /// Returns `true` if the action cannot be used during Enshroud.
    pub const enshroud_invalid
    /// Returns the base GCD recast time, or `None` if the action is not a gcd.
    pub const gcd: ScaleTime?
    pub const spell for gcd = ScaleTime::spell(2500)
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
}]
#[allow(missing_docs)] // no reason to document the variants.
/// An action specific to the Reaper job.
pub enum RprAction {
    #[skill]
    #[enshroud_invalid]
    #[name = "Slice"]
    Slice,
    #[skill]
    #[enshroud_invalid]
    #[name = "Waxing Slice"]
    WaxingSlice,
    #[skill]
    #[name = "Shadow of Death"]
    ShadowOfDeath,
    #[spell]
    #[cast = ScaleTime::spell(1300)]
    #[name = "Harpe"]
    Harpe,
    #[name = "Hell's Ingress"]
    HellsIngress,
    #[name = "Hell's Egress"]
    HellsEgress,
    #[skill]
    #[enshroud_invalid]
    #[name = "Spinning Scythe"]
    SpinningScythe,
    #[skill]
    #[enshroud_invalid]
    #[name = "Infernal Slice"]
    InfernalSlice,
    #[skill]
    #[name = "Whorl of Death"]
    WhorlOfDeath,
    #[skill]
    #[enshroud_invalid]
    #[name = "Nightmare Scythe"]
    NightmareScythe,
    #[enshroud_invalid]
    #[name = "Blood Stalk"]
    BloodStalk,
    #[enshroud_invalid]
    #[name = "Grim Swathe"]
    GrimSwathe,
    #[skill]
    #[enshroud_invalid]
    #[cooldown = 30000]
    #[cd_charges = 2]
    #[name = "Soul Slice"]
    SoulSlice,
    #[skill]
    #[enshroud_invalid]
    #[cooldown = 30000]
    #[cd_charges = 2]
    #[name = "Soul Scythe"]
    SoulScythe,
    #[skill]
    #[enshroud_invalid]
    #[name = "Gibbet"]
    Gibbet,
    #[skill]
    #[enshroud_invalid]
    #[name = "Gallows"]
    Gallows,
    #[skill]
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
    #[spell]
    #[cast = ScaleTime::spell(5000)]
    #[name = "Soulsow"]
    Soulsow,
    #[skill]
    #[enshroud_invalid]
    #[name = "Plentiful Harvest"]
    PlentifulHarvest,
    #[spell]
    #[cast = ScaleTime::spell(1300)]
    #[name = "Communio"]
    Communio,
    #[enshroud_invalid]
    #[name = "Unveiled Gibbet"]
    UnveiledGibbet,
    #[enshroud_invalid]
    #[name = "Unveiled Gallows"]
    UnveiledGallows,
    // Regress,
    #[gcd = ScaleTime::none(1500)]
    #[name = "Void Reaping"]
    VoidReaping,
    #[gcd = ScaleTime::none(1500)]
    #[name = "Cross Reaping"]
    CrossReaping,
    #[gcd = ScaleTime::none(1500)]
    #[name = "Grim Reaping"]
    GrimReaping,
    #[spell]
    #[name = "Harvest Moon"]
    HarvestMoon,
    #[cooldown = 1000]
    #[name = "Lemure's Slice"]
    LemuresSlice,
    #[cooldown = 1000]
    #[name = "Lemure's Scythe"]
    LemuresScythe,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The state of the Reaper job gauges, cooldowns, and combos.
pub struct RprState {
    /// The cooldowns for Reaper actions.
    pub cds: RprCds,
    /// The combos for Reaper.
    pub combos: RprCombos,
    /// The Soul gauge.
    pub soul: GaugeU8<100>,
    /// The Shroud gauge.
    pub shroud: GaugeU8<100>,
    /// The stacks of Lemure Shroud.
    pub lemure_shroud: GaugeU8<5>,
    /// The stacks of Void Shroud.
    pub void_shroud: GaugeU8<5>,
}

impl JobState for RprState {
    fn advance(&mut self, time: u32) {
        self.cds.advance(time);
        self.combos.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The combos for Reaper.
pub struct RprCombos {
    /// The main combo.
    ///
    /// Includes the Infernal Slice combo as well as the Nightmare Scythe combo.
    pub main: ComboState<MainCombo>,
}

impl RprCombos {
    /// Checks that the main combo prerequisite is met for a certain action.
    pub fn check_main_for(&self, action: RprAction) -> bool {
        let c = match action {
            RprAction::WaxingSlice => MainCombo::Slice,
            RprAction::InfernalSlice => MainCombo::Waxing,
            RprAction::NightmareScythe => MainCombo::Spinning,
            _ => return true,
        };
        self.main.check(c)
    }

    /// Advances the combos forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        self.main.advance(time);
    }
}

// lmfao
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
/// The possible states the main combo can be in.
pub enum MainCombo {
    /// Combo Action: Slice is met.
    Slice,
    /// Combo Action: Waxing Slice is met.
    Waxing,
    /// Combo Action: Spinning Scythe is met.
    Spinning,
}

job_cd_struct! {
    RprAction =>

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Clone, Debug, Default)]
    /// The active cooldowns for Reaper actions.
    pub RprCds

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    /// The various cooldown groups a Reaper action can be part of.
    pub RprCdGroup

    "Hell's Ingress/Egress"
    hells Hells: HellsIngress HellsEgress;
    "Blood Stalk, Grim Swathe, and Unveiled Gibbet/Gallows"
    reaver Reaver: BloodStalk GrimSwathe UnveiledGibbet UnveiledGallows;
    "Soul Slice/Scythe"
    soul Soul: SoulSlice SoulScythe;
    "Arcane Circle"
    circle Circle: ArcaneCircle;
    "Gluttony"
    gluttony Gluttony: Gluttony;
    "Enshroud"
    enshroud Enshroud: Enshroud;
    "Lemure's Slice/Scythe"
    lemures Lemures: LemuresSlice LemuresScythe;
}
