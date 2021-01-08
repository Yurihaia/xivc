use xivc_macros::embed_data;

use crate::{Clan, Job};


pub const fn attack_power(job: Job) -> JobField {
    use Job::*;
    match job {
        ROG | NIN | ARC | BRD | MCH | DNC => JobField::DEX,
        _ => JobField::STR,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum JobField {
    HP,
    MP,
    STR,
    VIT,
    DEX,
    INT,
    MND,
}

// Levels is just an int from 0..=80

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LevelField {
    MP,
    MAIN,
    SUB,
    DIV,
    HP,
    THREAT,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ClanField {
    STR,
    VIT,
    DEX,
    INT,
    MND,
}

// Omega cursed function
pub const fn atk_mod(job: Job, level: u8) -> u64 {
    let out = if level == 80 {
        165
    } else if level > 50 {
        ((level - 50) as u64 * 5) / 2 + 75
    } else {
        75
    };
    // This is currently speculation
    // Will provide correct values for level 80 however
    if job.tank() {
        (23 * out) / 33
    } else {
        out
    }
}

pub const fn job(job: Job, field: JobField) -> u64 {
    let (hp, mp, str, vit, dex, int, mnd) = if let Some(v) = embed_data!(
        "./src/math/data_job.csv",
        job,
        enum Job,
        u64
    ) {
        v
    } else {
        (0, 0, 0, 0, 0, 0, 0)
    };
    match field {
        JobField::HP => hp,
        JobField::MP => mp,
        JobField::STR => str,
        JobField::VIT => vit,
        JobField::DEX => dex,
        JobField::INT => int,
        JobField::MND => mnd,
    }
}

pub const fn level(level: u8, field: LevelField) -> u64 {
    let (mp, main, sub, div, hp, threat) =
        if let Some(v) = embed_data!("./src/math/data_level.csv", level, u8, u64) {
            v
        } else {
            (0, 0, 0, 0, 0, 0)
        };
    match field {
        LevelField::MP => mp,
        LevelField::MAIN => main,
        LevelField::SUB => sub,
        LevelField::DIV => div,
        LevelField::HP => hp,
        LevelField::THREAT => threat,
    }
}

pub const fn clan(clan: Clan, field: ClanField) -> i8 {
    let (str, dex, vit, int, mnd) = if let Some(v) = embed_data!(
        "./src/math/data_clan.csv",
        clan,
        enum Clan,
        i8
    ) {
        v
    } else {
        (0, 0, 0, 0, 0)
    };
    match field {
        ClanField::STR => str,
        ClanField::VIT => vit,
        ClanField::DEX => dex,
        ClanField::INT => int,
        ClanField::MND => mnd,
    }
}

// Will this be used? Find out in our next episode: "Yuri has a shit memory"
// If anyone finds this and it hasn't been used, feel free to :gnbbap:
#[derive(Copy, Clone, Debug)]
pub struct DataTableProvider {
    pub job: fn(Job, JobField) -> u64,
    pub level: fn(u8, LevelField) -> u64,
    pub clan: fn(Clan, ClanField) -> i8,
}

impl Default for DataTableProvider {
    fn default() -> Self {
        Self { job, level, clan }
    }
}
