use data::LevelField;

use self::data::JobField;

pub mod data;

#[derive(Copy, Clone, Debug)]
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
pub struct PlayerInfo {
    pub clan: data::Clan,
    pub job: data::Job,
    pub lvl: u8,
}

#[derive(Copy, Clone, Debug)]
pub struct WeaponInfo {
    pub phys_dmg: u16,
    pub magic_dmg: u16,
    pub auto: u16,
    pub delay: u16,
}

#[derive(Copy, Clone, Debug)]
pub struct XivMath {
    stats: PlayerStats,
    weapon: WeaponInfo,
    info: PlayerInfo,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ActionStat {
    AttackPower,
    AttackMagic,
    HealingMagic,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SpeedStat {
    SpellSpeed,
    SkillSpeed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CDHHandle {
    Avg,
    Yes,
    No,
}

impl XivMath {
    const fn attack_power(&self) -> u64 {
        match self.info.job.attack_power() {
            JobField::DEX => self.stats.dex as u64,
            _ => self.stats.str as u64,
        }
    }
    const fn attack_magic(&self) -> u64 {
        if self.info.job.healer() {
            self.stats.mnd as u64
        } else {
            self.stats.int as u64
        }
    }
    const fn healing_magic(&self) -> u64 {
        self.stats.mnd as u64
    }
    const fn speed_mod(&self, stat: SpeedStat) -> u64 {
        match stat {
            SpeedStat::SkillSpeed => self.sks_mod(),
            SpeedStat::SpellSpeed => self.sps_mod(),
        }
    }
    const fn crt_mod(&self, handle: CDHHandle) -> u64 {
        match handle {
            CDHHandle::Yes => self.crt_damage(),
            CDHHandle::No => 1000,
            CDHHandle::Avg => 1000 + (self.crt_damage() - 1000) * self.crt_chance() / 1000,
        }
    }
    const fn dh_mod(&self, handle: CDHHandle) -> u64 {
        match handle {
            CDHHandle::Yes => 1250,
            CDHHandle::No => 1000,
            CDHHandle::Avg => 1000 + 250 * self.dh_chance() / 1000,
        }
    }

    pub const fn main_stat(&self, stat: ActionStat) -> u64 {
        match stat {
            ActionStat::AttackPower => self.attack_power(),
            ActionStat::AttackMagic => self.attack_magic(),
            ActionStat::HealingMagic => self.healing_magic(),
        }
    }
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
    pub const fn atk_damage(&self, stat: ActionStat) -> u64 {
        let lvl_main = data::level(self.info.lvl, LevelField::MAIN);
        data::atk_mod(self.info.job, self.info.lvl) * (self.main_stat(stat) - lvl_main) / lvl_main
            + 100
    }
    pub const fn det_damage(&self) -> u64 {
        130 * (self.stats.det as u64 - data::level(self.info.lvl, LevelField::MAIN))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }
    pub const fn ten_damage(&self) -> u64 {
        if self.info.job.tank() {
            100 * (self.stats.ten as u64 - data::level(self.info.lvl, LevelField::SUB))
                / data::level(self.info.lvl, LevelField::DIV)
                + 1000
        } else {
            1000
        }
    }
    pub const fn crt_damage(&self) -> u64 {
        200 * (self.stats.crt as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1400
    }
    pub const fn crt_chance(&self) -> u64 {
        200 * (self.stats.crt as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 50
    }
    pub const fn dh_chance(&self) -> u64 {
        550 * (self.stats.dh as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
    }
    pub const fn sks_mod(&self) -> u64 {
        130 * (self.stats.sks as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }
    pub const fn sps_mod(&self) -> u64 {
        130 * (self.stats.sps as u64 - data::level(self.info.lvl, LevelField::SUB))
            / data::level(self.info.lvl, LevelField::DIV)
            + 1000
    }
    pub const fn mp_regen(&self) -> u64 {
        if self.info.job.healer() {
            150 * (self.stats.pie as u64 - data::level(self.info.lvl, LevelField::MAIN))
                / data::level(self.info.lvl, LevelField::DIV)
                + 200
        } else {
            200
        }
    }
    pub const fn aa_mod(&self) -> u64 {
        (data::level(self.info.lvl, LevelField::MAIN)
            * data::job(self.info.job, self.info.job.attack_power())
            / 1000
            + self.weapon.phys_dmg as u64)
            * self.weapon.delay as u64
            / 300
    }
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