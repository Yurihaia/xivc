pub mod data;
use data::{LevelField, JobField};

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

#[derive(Copy, Clone, Debug)]
/// Information about the player that is not tied to gear
pub struct PlayerInfo {
    /// The race and clan of the player
    pub clan: data::Clan,
    /// The current job or class equipped
    pub job: data::Job,
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
pub enum CDHHandle {
    /// Signifies that the critical/direct hit should be averaged out in damage.
    /// Calculated as `1 + damage% * chance`
    Avg,
    /// Signifies that the critical/direct hit occured.
    Yes,
    /// Signifies that the critical/direct hit did not occur.
    No,
}

impl XivMath {
    /// Creates a new `XivMath` instance based on the player's stats.
    pub const fn new(stats: PlayerStats, weapon: WeaponInfo, player: PlayerInfo) -> Self {
        XivMath {
            stats,
            weapon,
            info: player
        }
    }

    /// The relevant attack power stat.  
    /// This is dexterity for ROG/NIN and all phys ranged and strength otherwise.
    pub const fn attack_power(&self) -> u64 {
        match self.info.job.attack_power() {
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
    
    /// The crit multiplied based on the handling. Output is scaled by `1000`
    const fn crt_mod(&self, handle: CDHHandle) -> u64 {
        match handle {
            CDHHandle::Yes => self.crt_damage(),
            CDHHandle::No => 1000,
            CDHHandle::Avg => 1000 + (self.crt_damage() - 1000) * self.crt_chance() / 1000,
        }
    }
    
    /// The direct hit multiplier based on the handling. Output is scaled by `1000`
    const fn dh_mod(&self, handle: CDHHandle) -> u64 {
        match handle {
            CDHHandle::Yes => 1250,
            CDHHandle::No => 1000,
            CDHHandle::Avg => 1000 + 250 * self.dh_chance() / 1000,
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
            ActionStat::AttackPower => self.info.job.attack_power(),
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
    pub const fn det_damage(&self) -> u64 {
        130 * (self.stats.det as u64 - data::level(self.info.lvl, LevelField::MAIN))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }

    /// The Tenacity modifier. Only used if the player is a tank.  
    /// The output of this function is a multiplier scaled by `1000`.
    pub const fn ten_damage(&self) -> u64 {
        if self.info.job.tank() {
            100 * (self.stats.ten as u64 - data::level(self.info.lvl, LevelField::SUB))
                / data::level(self.info.lvl, LevelField::DIV)
                + 1000
        } else {
            1000
        }
    }

    /// The Critical Hit modifier. Has a base x1.4 modifier.  
    /// The output of this function is a multiplier scaled by `1000`.
    pub const fn crt_damage(&self) -> u64 {
        200 * (self.stats.crt as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1400
    }

    /// The Critical Hit chance. Has a base 5% rate.  
    /// The output of this function is a probability scaled by `1000`.
    pub const fn crt_chance(&self) -> u64 {
        200 * (self.stats.crt as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 50
    }

    /// The Direct Hit chance. Unlike crit, the base rate is 0%.  
    /// The output of this function is a probability scaled by `1000`.
    pub const fn dh_chance(&self) -> u64 {
        550 * (self.stats.dh as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
    }

    /// The Skill Speed modifier.  
    /// The output of this function is a multiplier scaled by `1000`
    pub const fn sks_mod(&self) -> u64 {
        130 * (self.stats.sks as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }

    /// The Spell Speed modifier.  
    /// The output of this function is a multiplier scaled by `1000`
    pub const fn sps_mod(&self) -> u64 {
        130 * (self.stats.sps as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }

    /// The MP regen per tick. Value is 200 unless the player is a healer, in which case
    /// Piety is taken into account
    pub const fn mp_regen(&self) -> u64 {
        if self.info.job.healer() {
            150 * (self.stats.pie as u64 - data::level(self.info.lvl, LevelField::MAIN))
                / data::level(self.info.lvl, LevelField::DIV)
                + 200
        } else {
            200
        }
    }

    /// The Auto attack modifier. Similar to [`wd_mod`](Self::wd_mod) but includes weapon delay.  
    /// The output of this function is a multiplier scaled by `100`
    pub const fn aa_mod(&self) -> u64 {
        (data::level(self.info.lvl, LevelField::MAIN)
            * data::job(self.info.job, self.info.job.attack_power())
            / 1000
            + self.weapon.phys_dmg as u64)
            * self.weapon.delay as u64
            / 300
    }

    /// Calculates the damage a direct damage action with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`, whether or not the
    /// action `crit` or `dhit`, and a random modifier `rand` between `9500` and `10500` inclusive.
    // TODO: write examples
    pub const fn prebuff_action_damage(
        &self,
        potency: u64,
        stat: ActionStat,
        traits: u64,
        crit: CDHHandle,
        dhit: CDHHandle,
        // between 9500 and 10500?????
        // Scaled by 10000
        rand: u64,
    ) -> u64 {
        let d1 = potency * self.atk_damage(stat) * self.det_damage() / 100 / 1000;
        let d2 = d1 * self.ten_damage() / 1000 * self.wd_mod(stat) / 100 * traits / 100;
        let d3 = d2 * self.crt_mod(crit) / 1000 * self.dh_mod(dhit) / 1000;
        d3 * rand / 10000
    }
    
    
    /// Calculates the damage a damage over tick tick with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`,
    /// the type of `speed_stat` that the action was modified by,
    /// whether or not the action `crit` or `dhit`,
    /// and a random modifier `rand` between `9500` and `10500` inclusive.
    #[allow(clippy::too_many_arguments)]
    pub const fn prebuff_dot_damage(
        &self,
        potency: u64,
        stat: ActionStat,
        traits: u64,
        speed_stat: SpeedStat,
        crit: CDHHandle,
        dhit: CDHHandle,
        // Scaled by 10000
        rand: u64,
    ) -> u64 {
        let d1 = potency * self.atk_damage(stat) * self.det_damage() / 100 / 1000;
        let d2 = d1 * self.ten_damage() / 1000 * self.speed_mod(speed_stat) / 1000
            * self.wd_mod(stat)
            / 100
            * traits
            / 100
            + 1;
        let d3 = d2 * rand / 10000;
        d3 * self.crt_mod(crit) / 1000 * self.dh_mod(dhit) / 1000
    }
    
    /// Calculates the damage of an auto attack with a certain `potency` will do.
    /// The damage depends on the type of `stat` used, the job `traits`, whether or not the
    /// action `crit` or `dhit`, and a random modifier `rand` between `9500` and `10500` inclusive.  
    /// The potency is 100 for ARC/BRD/MCH, and 110 for all other classes/jobs.
    pub const fn prebuff_aa_damage(
        &self,
        potency: u64,
        traits: u64,
        crit: CDHHandle,
        dhit: CDHHandle,
        rand: u64,
    ) -> u64 {
        let d1 =
            potency * self.atk_damage(ActionStat::AttackPower) * self.det_damage() / 100 / 1000;
        let d2 = d1 * self.ten_damage() / 1000 * self.sks_mod() / 1000 * self.aa_mod() / 100
            * traits
            / 100;
        let d3 = d2 * self.crt_mod(crit) / 1000 * self.dh_mod(dhit) / 1000;
        d3 * rand / 10000
    }

    /// Calculates the cast or recast time of an action that uses `speed_stat`.  
    /// `buffs` is the sum of all of the speed buffs, and `haste` is the multiplier of the Hatse effect.
    /// Both are scaled by `100`.  
    /// The output of this function is the time in centiseconds.
    pub const fn action_cast_length(
        &self,
        base: u64,
        speed_stat: SpeedStat,
        buffs: u64,
        haste: u64,
    ) -> u64 {
        let g1 = (2000 - self.speed_mod(speed_stat)) * base / 100;
        let g2 = (100 - buffs) * (100 - haste) / 100;
        g1 * g2 / 1000
    }
}