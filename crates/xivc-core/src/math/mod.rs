pub mod data;
use data::{JobField, LevelField};

use crate::enums::{Clan, DamageInstance, Job};

#[derive(Copy, Clone, Debug)]
/// Player main & substats.
/// These values are all the same as the ones you would find in-game.
pub struct PlayerStats {
    // Main stats
    pub str: u16,
    pub vit: u16,
    pub dex: u16,
    pub int: u16,
    pub mnd: u16,
    // Substats
    pub det: u16,
    pub crt: u16,
    pub dh: u16,
    pub sks: u16,
    pub sps: u16,
    pub ten: u16,
    pub pie: u16,
}

impl PlayerStats {
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
    /// "Physical Damage" field, default to 0 on if not present
    pub phys_dmg: u16,
    /// "Magic Damage" field, default to 0 on if not present
    pub magic_dmg: u16,
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
/// * [`prebuff_action_damage`](Self::prebuff_action_damage) for direct damage calculation
/// * [`prebuff_dot_damage`](Self::prebuff_dot_damage) for damage over time (DoT) damage calculation
/// * [`prebuff_aa_damage`](Self::prebuff_aa_damage) for auto attack damage calculation
/// * [`action_cast_length`](Self::action_cast_length) for GCD and cast time calculation
///
/// Many of the helper functions will return their values as a scaled integer. Because of the way
/// FFXIV does its math, greater accuracy has been found when not interacting with floating point
/// numbers on the backend.
#[derive(Copy, Clone, Debug)]
pub struct XivMath {
    pub stats: PlayerStats,
    pub weapon: WeaponInfo,
    pub info: PlayerInfo,
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

    /// The crit multiplied based on the handling.  
    /// Output is scaled by `10000000` to allow for greater accuracy for [`CDHHandle::Avg`].
    fn crit_mod(&self, handle: HitTypeHandle, buffs: &impl Buffs) -> u64 {
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
    /// Output is scaled by `1000000` to allow for greater accuracy for [`CDHHandle::Avg`].
    fn dhit_mod(&self, handle: HitTypeHandle, buffs: &impl Buffs) -> u64 {
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
            + if let ActionStat::AttackPower = stat {
                self.weapon.phys_dmg as u64
            } else {
                self.weapon.magic_dmg as u64
            }
    }

    /// The Attack Damage modifier based on the action stat used.  
    /// The output of this function is a multiplier scaled by `100`.
    pub const fn atk_damage(&self, stat: ActionStat) -> u64 {
        let lvl_main = data::level(self.info.lvl, LevelField::MAIN);
        data::atk_mod(self.info.job, self.info.lvl) * (self.main_stat(stat) - lvl_main) / lvl_main
            + 100
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
            + self.weapon.phys_dmg as u64)
            * self.weapon.delay as u64 / 300
    }

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
    #[rustfmt::skip]
    #[allow(clippy::too_many_arguments)]
    pub fn action_damage(
        &self,
        potency: u64,
        stat: ActionStat,
        traits: u64,
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
        let prebuff = potency
            * this.wd_mod(stat) / 100
            * this.atk_damage(stat) / 100
            * this.det_damage(dhit.is_force()) / 1000
            * this.ten_damage() / 1000
            * traits / 100
            * this.crit_mod(crit, buffs) / 1000000
            * this.dhit_mod(dhit, buffs) / 1000000
            * rand / 10000;
        buffs.basic_damage(prebuff, stat)
    }

    /// Calculates the damage a damage over time tick with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`,
    /// the type of `speed_stat` that the action was modified by,
    /// and the chance the dot has to `crit` or `dhit`,
    #[rustfmt::skip]
    #[allow(clippy::too_many_arguments)]
    pub fn dot_damage_snapshot(
        &self,
        potency: u64,
        stat: ActionStat,
        traits: u64,
        speed_stat: SpeedStat,
        buffs: &impl Buffs,
    ) -> EotSnapshot {
        let this = self.with_stats(buffs);
        let prebuff = potency
            * this.atk_damage(stat) / 100
            * this.det_damage(false) / 1000
            * this.ten_damage() / 1000
            * this.speed_mod(speed_stat) / 1000
            * this.wd_mod(stat) / 100
            * traits / 100
            + 1;
        EotSnapshot {
            base: buffs.basic_damage(prebuff, stat),
            crit_chance: buffs.crit_chance(this.crit_chance()) as u16,
            dhit_chance: buffs.dhit_chance(this.dhit_chance()) as u16,
            crit_damage: this.crit_damage() as u16
        }
    }

    /// Calculates the damage of an auto attack with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`, whether or not the
    /// action `crit` or `dhit`, and a random modifier `rand` between `9500` and `10500` inclusive.
    /// The potency is 100 for ARC/BRD/MCH, and 110 for all other classes/jobs.
    #[rustfmt::skip]
    pub fn aa_damage(
        &self,
        potency: u64,
        traits: u64,
        crit: HitTypeHandle,
        dhit: HitTypeHandle,
        rand: u64,
        buffs: &impl Buffs,
    ) -> u64 {
        let this = self.with_stats(buffs);
        let prebuff = potency
            * this.aa_mod() / 100
            * this.atk_damage(ActionStat::AttackPower) / 100
            * this.det_damage(dhit.is_force()) / 1000
            * this.ten_damage() / 1000
            * this.sks_mod() / 1000
            * traits / 100
            * this.crit_mod(crit, buffs) / 1000000
            * this.dhit_mod(dhit, buffs) / 1000000
            * rand / 10000;
        buffs.basic_damage(prebuff, ActionStat::AttackPower)
    }

    /// Calculates the cast or recast time of an action that uses `speed_stat`.
    /// `base` is the time in milliseconds for the base scaled duration length.
    /// The output of this function is the time in milliseconds.
    pub fn action_cast_length(&self, base: u64, speed_stat: SpeedStat, buffs: &impl Buffs) -> u64 {
        buffs.haste(base * (2000 - self.speed_mod(speed_stat)) / 1000)
    }
}

pub trait Buffs {
    fn damage(&self, base: DamageInstance) -> DamageInstance;

    // these should always be additive
    // some handling depends on it, and there is no way to test
    // the correct way they should be handled if they aren't multiplicative
    fn crit_chance(&self, base: u64) -> u64;
    fn dhit_chance(&self, base: u64) -> u64;

    fn stats(&self, base: PlayerStats) -> PlayerStats;
    fn haste(&self, base: u64) -> u64;

    fn basic_damage(&self, base: u64, stat: ActionStat) -> u64 {
        self.damage(DamageInstance::basic(base, stat)).dmg
    }
}

#[derive(Copy, Clone, Debug)]
pub struct EotSnapshot {
    pub base: u64,
    pub crit_damage: u16,
    pub crit_chance: u16,
    pub dhit_chance: u16,
}

impl EotSnapshot {
    pub fn dot_damage(&self, crit: HitTypeHandle, dhit: HitTypeHandle, rand: u64) -> u64 {
        self.base * rand / 10000 * self.crt_mod(crit) / 1000000 * self.dh_mod(dhit) / 1000000
    }

    /// The crit multiplier based on the handling.  
    /// Output is scaled by `1000000`  to allow for greater accuracy for [`CDHHandle::Avg`].
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
    /// Output is scaled by `1000000` to allow for greater accuracy for [`CDHHandle::Avg`].
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