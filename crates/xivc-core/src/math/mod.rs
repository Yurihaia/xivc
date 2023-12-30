//! The math that FFXIV uses.
//!
//! This module contains the [`XivMath`] struct, which is what you should
//! use for any calculations you need to do. This struct contains
//! a [player's stats], the [misc info] about them, their [weapon information],
//! and other things like the extra animation lock.
//!
//! On [`XivMath`], the functions [`action_damage`], [`dot_damage_snapshot`],
//! [`aa_damage`], and [`action_cast_length`] are the most important.
//! These functions are the way to convert from potency/base action recast time
//! into the value that has been modified by player stats and buffs. See their
//! documentation for more information.
//!
//! An important aspect of how FFXIV does its math is that all calculations are
//! done using integers. For this reason, many of the functions on [`XivMath`] will
//! return a scaled integer.
//! The scale will always be documented in the relevant function.
//!
//! [player's stats]: PlayerStats
//! [misc info]: PlayerInfo
//! [weapon information]: WeaponInfo
//! [`action_damage`]: XivMath::action_damage
//! [`dot_damage_snapshot`]: XivMath::dot_damage_snapshot
//! [`aa_damage`]: XivMath::aa_damage
//! [`action_cast_length`]: XivMath::action_cast_length

pub mod data;
use data::{JobField, LevelField};

use crate::enums::{Clan, DamageElement, DamageType, Job};

#[derive(Copy, Clone, Debug)]
/// Player main & substats.
/// These values are all the same as the ones you would find in-game.
pub struct PlayerStats {
    // Main stats
    /// The Strength main stat.
    pub str: u16,
    /// The Vitality main stat.
    pub vit: u16,
    /// The Dexterity main stat.
    pub dex: u16,
    /// The Intelligence main stat.
    pub int: u16,
    /// The Mind main stat.
    pub mnd: u16,
    // Substats
    /// The Determination substat.
    pub det: u16,
    /// The Critical Hit substat.
    pub crt: u16,
    /// The Direct Hit substat.
    pub dh: u16,
    /// The Skill Speed substat.
    pub sks: u16,
    /// The Spell Speed substat.
    pub sps: u16,
    /// The Tenacity substat.
    pub ten: u16,
    /// The Piety substat.
    pub pie: u16,
}

impl PlayerStats {
    /// Returns a set of player stats with all of the values
    /// set to the defaults for some specific level.
    pub const fn default(lvl: u8) -> Self {
        let main = data::level(lvl, LevelField::MAIN) as u16;
        let sub = data::level(lvl, LevelField::SUB) as u16;
        Self {
            // mainstats
            str: main,
            vit: main,
            dex: main,
            int: main,
            mnd: main,
            // substats that use mainstat scaling
            det: main,
            pie: main,
            // substats
            crt: sub,
            dh: sub,
            sks: sub,
            sps: sub,
            ten: sub,
        }
    }
}

#[derive(Copy, Clone, Debug)]
/// Information about the player that is not tied to gear
pub struct PlayerInfo {
    /// The race and clan of the player
    pub clan: Clan,
    /// The current job or class equipped
    pub job: Job,
    /// The level of the player
    pub lvl: u8,
}

#[derive(Copy, Clone, Debug)]
/// Information about the weaponn the player has equipped
pub struct WeaponInfo {
    /// "Physical Damage" or "Magic Damage" field, they are always the same
    pub wd: u16,
    /// "Auto Attack" field, multiplied by 100
    pub auto: u16,
    /// "Weapon Delay" field, multiplied by 100
    pub delay: u16,
}

/// Main interface with all of the game's calculations.  
///
/// # Usage
///
/// Most of the high level interface with this struct comes through one of the following
/// * [`action_damage`] for direct damage calculation
/// * [`dot_damage_snapshot`] for damage over time (DoT) damage calculation
/// * [`aa_damage`] for auto attack damage calculation
/// * [`action_cast_length`] for GCD and cast time calculation
///
/// Many of the helper functions will return their values as a scaled integer. Because of the way
/// FFXIV does its math, greater accuracy has been found when not interacting with floating point
/// numbers on the backend.
///
/// [`action_damage`]: XivMath::action_damage
/// [`dot_damage_snapshot`]: XivMath::dot_damage_snapshot
/// [`aa_damage`]: XivMath::aa_damage
/// [`action_cast_length`]: XivMath::action_cast_length
#[derive(Copy, Clone, Debug)]
pub struct XivMath {
    /// The stats of the player.
    pub stats: PlayerStats,
    /// The weapon being used by the player.
    pub weapon: WeaponInfo,
    /// The information of the player.
    pub info: PlayerInfo,
    /// Extra animation lock.
    ///
    /// This is often going to be the ping to the servers plus
    /// some value accounting for FPS.
    pub ex_lock: u16,
}

/// The stat to use for main stat calculations.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ActionStat {
    /// Used for damage and healing from tanks, melee, and phys ranged
    AttackPower,
    /// Used for damage from healers and casters.
    /// Also used for heals from casters besides ACN/SMN's `Physick`
    //  (lmao they still haven't fixed Physick)
    AttackMagic,
    /// Used for heals from healers. Also used for ACN/SMN's `Physick`
    HealingMagic,
}

/// The stat to use for relevant speed calculations.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SpeedStat {
    /// Used for the cast and recast times for spells and DoT scalars originating from spells
    SpellSpeed,
    /// Used for the cast and recast times for weaponskills,
    /// DoT scalars originating from weaponskills,
    /// and the scalar used for auto attacks.
    SkillSpeed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Handling of critical and direct hits.
pub enum HitTypeHandle {
    /// Signifies that the critical/direct hit should be averaged out in damage.
    /// Calculated as `1 + damage% * chance`
    Avg,
    /// Signifies that the critical/direct hit occured.
    Yes,
    /// Signifies that the critical/direct hit did not occur.
    No,
    /// Signifies that the action is an auto critical/direct hit action
    Force,
}

impl HitTypeHandle {
    /// Returns `true` if the handle is [`Force`]
    ///
    /// [`Force`]: Self::Force
    pub const fn is_force(&self) -> bool {
        matches!(self, Self::Force)
    }
}

impl XivMath {
    const DET_MOD: u64 = 140;
    const TEN_MOD: u64 = 100;
    const SPD_MOD: u64 = 130;
    const CHR_MOD: u64 = 200;
    const DHR_MOD: u64 = 550;
    const PIE_MOD: u64 = 150;

    /// Creates a new `XivMath` instance based on the player's stats.
    pub const fn new(stats: PlayerStats, weapon: WeaponInfo, player: PlayerInfo) -> Self {
        XivMath {
            stats,
            weapon,
            info: player,
            // 10 ms ping
            ex_lock: 10,
        }
    }

    /// The relevant attack power stat.  
    /// This is dexterity for ROG/NIN and all phys ranged and strength otherwise.
    pub const fn attack_power(&self) -> u64 {
        match data::attack_power(self.info.job) {
            JobField::DEX => self.stats.dex as u64,
            _ => self.stats.str as u64,
        }
    }

    /// The relevant attack magic stat.  
    /// This is mind for healers and intelligence otherwise.
    pub const fn attack_magic(&self) -> u64 {
        if self.info.job.healer() {
            self.stats.mnd as u64
        } else {
            self.stats.int as u64
        }
    }

    /// The relevant healing magic stat.  
    /// This is always mind.
    pub const fn healing_magic(&self) -> u64 {
        self.stats.mnd as u64
    }

    /// The relevant speed stat based off of the parameter.
    const fn speed_mod(&self, stat: SpeedStat) -> u64 {
        match stat {
            SpeedStat::SkillSpeed => self.sks_mod(),
            SpeedStat::SpellSpeed => self.sps_mod(),
        }
    }

    /// Returns the trait modifier for the player's job.
    ///
    /// This is the modifier for traits like "Main and Mend".
    pub fn job_trait_mod(&self) -> u64 {
        let job = self.info.job;
        if job.healer() || job.caster() {
            130
        } else if job.ranged() {
            120
        } else {
            100
        }
    }

    /// Returns the stat used to attack for the player's job.
    pub fn job_attack_stat(&self) -> ActionStat {
        self.info.job.attack_stat()
    }

    /// The crit multiplied based on the handling.  
    /// Output is scaled by `10000000` to allow for greater accuracy for [`HitTypeHandle::Avg`].
    pub fn crit_mod(&self, handle: HitTypeHandle, buffs: &impl Buffs) -> u64 {
        match handle {
            // damn these look similar
            HitTypeHandle::Force => {
                // scaled by 1000
                let chance = buffs.crit_chance(0);
                // scaled by 1000, this is a multiplier
                let dmg_mod = self.crit_damage();
                // the extra buff that happens because of the force crit
                let force_buff = 1000 + (dmg_mod - 1000) * chance / 1000;

                dmg_mod * force_buff
            }
            HitTypeHandle::Avg => {
                let chance = buffs.crit_chance(self.crit_chance());

                let dmg_mod = self.crit_damage();

                1000000 + (dmg_mod - 1000) * chance
            }
            // these two don't take buffs into account because
            // whether or not they crit was already determined
            HitTypeHandle::Yes => {
                let dmg_mod = self.crit_damage();
                dmg_mod * 1000
            }
            HitTypeHandle::No => 1000000,
        }
    }

    /// The direct hit multiplier based on the handling.  
    /// Output is scaled by `1000000` to allow for greater accuracy for [`HitTypeHandle::Avg`].
    pub fn dhit_mod(&self, handle: HitTypeHandle, buffs: &impl Buffs) -> u64 {
        match handle {
            HitTypeHandle::Force => 1250 * (1000 + 250 * buffs.dhit_chance(0) / 1000),
            HitTypeHandle::Avg => 1000000 + 250 * buffs.dhit_chance(self.dhit_chance()),
            HitTypeHandle::Yes => 1250000,
            HitTypeHandle::No => 1000000,
        }
    }

    /// The main stat used for a specific action handling.
    // basically boilerplate lmao
    pub const fn main_stat(&self, stat: ActionStat) -> u64 {
        match stat {
            ActionStat::AttackPower => self.attack_power(),
            ActionStat::AttackMagic => self.attack_magic(),
            ActionStat::HealingMagic => self.healing_magic(),
        }
    }

    /// The Weapon Damage modifier based on the action stat used.
    /// * When using attack power, the weapon's Physical Damage will be used
    /// * When using attack magic or healing magic, the weapon's Magic Damage will be used.
    ///   Additionally, the job stat modifier will be chosen depending on the main stat being used.
    /// The output of this function is a multiplier scaled by `100`.
    pub const fn wd_mod(&self, stat: ActionStat) -> u64 {
        let stat_field = match stat {
            ActionStat::AttackPower => data::attack_power(self.info.job),
            ActionStat::AttackMagic if self.info.job.healer() => JobField::MND,
            ActionStat::AttackMagic => JobField::INT,
            ActionStat::HealingMagic => JobField::MND,
        };
        data::level(self.info.lvl, LevelField::MAIN) * data::job(self.info.job, stat_field) / 1000
            + self.weapon.wd as u64
    }

    /// The Attack Damage modifier based on the action stat used.  
    /// The output of this function is a multiplier scaled by `100`.
    pub const fn atk_damage(&self, stat: ActionStat) -> u64 {
        let lvl_main = data::level(self.info.lvl, LevelField::MAIN);
        // self.main_stat(stat) can be under lvl_main bc of job modifiers
        // so we handle that here
        let main = self.main_stat(stat);
        let atk_mod = data::atk_mod(self.info.job, self.info.lvl);
        if main < lvl_main {
            // seems to work. pretty sure div_ceil is correct here.
            100 - (atk_mod * (lvl_main - main)).div_ceil(lvl_main)
        } else {
            atk_mod * (main - lvl_main) / lvl_main + 100
        }
    }

    /// The Determination modifier.  
    /// The output of this function is a multiplier scaled by `1000`.
    pub const fn det_damage(&self, force_dh: bool) -> u64 {
        let ddet = self.stats.det as u64 - data::level(self.info.lvl, LevelField::MAIN);
        let ddh = self.stats.dh as u64 - data::level(self.info.lvl, LevelField::SUB);

        Self::DET_MOD * ddet / data::level(self.info.lvl, LevelField::DIV)
            + Self::DET_MOD * ddh * (force_dh as u64) / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }

    /// The Tenacity modifier. Only used if the player is a tank.  
    /// The output of this function is a multiplier scaled by `1000`.
    pub const fn ten_damage(&self) -> u64 {
        if self.info.job.tank() {
            Self::TEN_MOD * (self.stats.ten as u64 - data::level(self.info.lvl, LevelField::SUB))
                / data::level(self.info.lvl, LevelField::DIV)
                + 1000
        } else {
            1000
        }
    }

    /// The Critical Hit modifier. Has a base x1.4 modifier.  
    /// The output of this function is a multiplier scaled by `1000`.
    pub const fn crit_damage(&self) -> u64 {
        200 * (self.stats.crt as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            // base critical hit damage
            + 1400
    }

    /// The Critical Hit chance. Has a base 5% rate.  
    /// The output of this function is a probability scaled by `1000`.
    pub const fn crit_chance(&self) -> u64 {
        Self::CHR_MOD * (self.stats.crt as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            // base critical hit rate
            + 50
    }

    /// The Direct Hit chance. Unlike crit, the base rate is 0%.  
    /// The output of this function is a probability scaled by `1000`.
    pub const fn dhit_chance(&self) -> u64 {
        Self::DHR_MOD * (self.stats.dh as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
    }

    /// The Skill Speed modifier.  
    /// The output of this function is a multiplier scaled by `1000`
    pub const fn sks_mod(&self) -> u64 {
        Self::SPD_MOD * (self.stats.sks as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }

    /// The Spell Speed modifier.  
    /// The output of this function is a multiplier scaled by `1000`
    pub const fn sps_mod(&self) -> u64 {
        Self::SPD_MOD * (self.stats.sps as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }

    /// The MP regen per tick. Value is 200 unless the player is a healer, in which case
    /// Piety is taken into account
    pub const fn mp_regen(&self) -> u64 {
        if self.info.job.healer() {
            Self::PIE_MOD * (self.stats.pie as u64 - data::level(self.info.lvl, LevelField::MAIN))
                / data::level(self.info.lvl, LevelField::DIV)
                + 200
        } else {
            200
        }
    }

    /// The Auto attack modifier. Similar to [`wd_mod`](Self::wd_mod) but includes weapon delay.
    /// The output of this function is a multiplier scaled by `100`
    #[rustfmt::skip]
    pub const fn aa_mod(&self) -> u64 {
        (data::level(self.info.lvl, LevelField::MAIN)
            * data::job(self.info.job, data::attack_power(self.info.job)) / 1000
            + self.weapon.wd as u64)
            * self.weapon.delay as u64 / 300
    }

    /// Returns a copy of this struct but with stats updated by the
    /// specified [`Buffs`].
    pub fn with_stats(&self, buffs: &impl Buffs) -> Self {
        Self {
            stats: buffs.stats(self.stats),
            ..*self
        }
    }

    /// Calculates the damage a direct damage action with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`, whether or not the
    /// action `crit` or `dhit`, and a random modifier `rand` between `9500` and `10500` inclusive.
    // TODO: write examples
    #[allow(clippy::too_many_arguments)]
    pub fn action_damage(
        &self,
        potency: u64,
        dmg_ty: DamageType,
        dmg_el: DamageElement,
        stat: ActionStat,
        crit: HitTypeHandle,
        dhit: HitTypeHandle,
        // between 9500 and 10500?????
        // Scaled by 10000
        rand: u64,
        buffs: &impl Buffs,
    ) -> u64 {
        let this = self.with_stats(buffs);
        // The exact order is unknown, and should only lead to ~1-2 damage variation.
        // This order is used by Ari in their tank calc sheet.
        #[rustfmt::skip]
        let prerand = potency
            * this.atk_damage(stat) / 100
            * this.det_damage(dhit.is_force()) / 1000
            * this.ten_damage() / 1000
            * this.wd_mod(stat) / 100
            * this.job_trait_mod() / 100
            + (potency < 100) as u64;
        #[rustfmt::skip]
        let prebuff = prerand
            * this.crit_mod(crit, buffs) / 1000000
            * this.dhit_mod(dhit, buffs) / 1000000
            * rand / 10000;
        buffs.damage(prebuff, dmg_ty, dmg_el)
    }

    /// Calculates the damage a damage over time tick with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`,
    /// the type of `speed_stat` that the action was modified by,
    /// and the chance the dot has to `crit` or `dhit`,
    #[allow(clippy::too_many_arguments)]
    pub fn dot_damage_snapshot(
        &self,
        potency: u64,
        dmg_ty: DamageType,
        dmg_el: DamageElement,
        stat: ActionStat,
        speed_stat: SpeedStat,
        buffs: &impl Buffs,
    ) -> EotSnapshot {
        let this = self.with_stats(buffs);
        // why
        #[rustfmt::skip]
        let prerand = match stat {
            ActionStat::AttackMagic => potency
                * this.wd_mod(stat) / 100
                * this.atk_damage(stat) / 100
                * this.speed_mod(speed_stat) / 1000
                * this.det_damage(false) / 1000
                * this.ten_damage() / 1000
                * this.job_trait_mod() / 100
                + (potency < 100) as u64,
            ActionStat::AttackPower => potency
                * this.atk_damage(stat) / 100
                * this.det_damage(false) / 1000
                * this.ten_damage() / 1000
                * this.speed_mod(speed_stat) / 1000
                * this.wd_mod(stat) / 100
                * this.job_trait_mod() / 100
                + (potency < 100) as u64,
            _ => panic!("ActionStat::HealingMagic cannot be used in XivMath::dot_damage_snapshot"),
        };
        EotSnapshot {
            base: buffs.damage(prerand, dmg_ty, dmg_el),
            crit_chance: buffs.crit_chance(this.crit_chance()) as u16,
            dhit_chance: buffs.dhit_chance(this.dhit_chance()) as u16,
            crit_damage: this.crit_damage() as u16,
        }
    }

    /// Calculates the damage of an auto attack with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`, whether or not the
    /// action `crit` or `dhit`, and a random modifier `rand` between `9500` and `10500` inclusive.
    /// The potency is 100 for ARC/BRD/MCH, and 110 for all other classes/jobs.
    pub fn aa_damage(
        &self,
        potency: u64,
        dmg_ty: DamageType,
        dmg_el: DamageElement,
        crit: HitTypeHandle,
        dhit: HitTypeHandle,
        rand: u64,
        buffs: &impl Buffs,
    ) -> u64 {
        let this = self.with_stats(buffs);
        #[rustfmt::skip]
        let prerng = potency
            * this.atk_damage(ActionStat::AttackPower) / 100
            * this.det_damage(dhit.is_force()) / 1000
            * this.ten_damage() / 1000
            * this.sks_mod() / 1000
            * this.aa_mod() / 100
            * this.job_trait_mod() / 100
            + (potency < 100) as u64;
        #[rustfmt::skip]
        let prebuff = prerng
            * this.crit_mod(crit, buffs) / 1000000
            * this.dhit_mod(dhit, buffs) / 1000000
            * rand / 10000;
        buffs.damage(prebuff, dmg_ty, dmg_el)
    }

    /// Calculates the cast or recast time of an action that uses `speed_stat`.
    /// `base` is the time in milliseconds for the base scaled duration length.
    /// The output of this function is the time in milliseconds.
    pub fn action_cast_length(&self, base: u64, speed_stat: SpeedStat, buffs: &impl Buffs) -> u64 {
        buffs.haste(base * (2000 - self.speed_mod(speed_stat)) / 1000)
    }
}

/// The collection of [status effects] that interact with the game math.
///
/// [status effects]: crate::world::status::StatusEffect
pub trait Buffs {
    /// The combined damage multiplier.
    fn damage(&self, base: u64, dmg_ty: DamageType, dmg_el: DamageElement) -> u64;

    // these should always be additive
    // some handling depends on it, and there is no way to test
    // the correct way they should be handled if they aren't multiplicative
    /// The combined additional Critical Hit chance.
    fn crit_chance(&self, base: u64) -> u64;
    /// The combined addition Direct Hit chance.
    fn dhit_chance(&self, base: u64) -> u64;

    /// The combined effect on the player's stats.
    fn stats(&self, base: PlayerStats) -> PlayerStats;
    /// The combined haste effects.
    fn haste(&self, base: u64) -> u64;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Hash)]
/// A snapshot for some Effect-over-Time status.
pub struct EotSnapshot {
    /// The base damage before randomization for the status.
    pub base: u64,
    /// The critical hit damage for the status.
    pub crit_damage: u16,
    /// The critical hit chance for the status.
    pub crit_chance: u16,
    /// The direct hit chance for the status.
    pub dhit_chance: u16,
}

impl EotSnapshot {
    /// Returns the resulting damage/healing for this EoT.
    ///
    /// the params `crit` and `dhit` are the handling for the respective hit types. Note that
    /// healing can never direct hit, and no dots are auto-crit/dhits.
    ///
    /// If this is a DoT effect, rand should be between `95000` and `105000`.<br>
    /// If this is a HoT effect, rand should be between `97000` and `103000`.
    pub fn eot_result(&self, crit: HitTypeHandle, dhit: HitTypeHandle, rand: u64) -> u64 {
        self.base * rand / 10000 * self.crt_mod(crit) / 1000000 * self.dh_mod(dhit) / 1000000
    }

    /// The crit multiplier based on the handling.  
    /// Output is scaled by `1000000`  to allow for greater accuracy for [`HitTypeHandle::Avg`].
    const fn crt_mod(&self, handle: HitTypeHandle) -> u64 {
        let damage = self.crit_damage as u64;
        let chance = self.crit_chance as u64;

        match handle {
            // dots can never force crit/dhit but i'll keep this here
            HitTypeHandle::Force => damage * (1000 + (damage - 1000) * chance / 1000),
            HitTypeHandle::Avg => 1000000 + (damage - 1000) * chance,
            HitTypeHandle::Yes => damage * 1000,
            HitTypeHandle::No => 1000000,
        }
    }

    /// The direct hit multiplier based on the handling.  
    /// Output is scaled by `1000000` to allow for greater accuracy for [`HitTypeHandle::Avg`].
    const fn dh_mod(&self, handle: HitTypeHandle) -> u64 {
        let damage = 1250;
        let chance = self.dhit_chance as u64;

        match handle {
            HitTypeHandle::Force => damage * (1000 + (damage - 1000) * chance / 1000),
            HitTypeHandle::Avg => 1000000 + (damage - 1000) * chance,
            HitTypeHandle::Yes => damage * 1000,
            HitTypeHandle::No => 1000000,
        }
    }
}
