//! Utilities for interacting with datasheets.
//!
//! This module contains functions for querying various
//! precompiled CSV files related to the math the game does.
//! It generally will not be helpful, and should instead
//! be used through the main [`XivMath`] type
//!
//! [`XivMath`]: super::XivMath

use macros::embed_data;

use crate::enums::{Clan, Job};

/// Returns the [`JobField`] for the stat the job uses for Attack Power.
pub const fn attack_power(job: Job) -> JobField {
    use Job::*;
    match job {
        ROG | NIN | ARC | BRD | MCH | DNC => JobField::DEX,
        _ => JobField::STR,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
// This is the naming conventions used and its not even really an acronym
#[allow(clippy::upper_case_acronyms)]
#[allow(missing_docs)]
/// A field in the job stat datasheet.
pub enum JobField {
    HP,
    MP,
    STR,
    VIT,
    DEX,
    INT,
    MND,
}

// Levels is just an int from 0..=90

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
// This is the naming conventions used and its not even really an acronym
#[allow(clippy::upper_case_acronyms)]
#[allow(missing_docs)]
/// A field in the level datasheet.
pub enum LevelField {
    MAIN,
    SUB,
    DIV,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
// This is the naming conventions used and its not even really an acronym
#[allow(clippy::upper_case_acronyms)]
#[allow(missing_docs)]
/// A field in the clan stat datasheet.
pub enum ClanField {
    STR,
    VIT,
    DEX,
    INT,
    MND,
}

// Omega cursed function
/// Returns the attack modifier for a job at a specific level.
///
/// Currently, only level `90` is supported.
pub const fn atk_mod(job: Job, level: u8) -> u64 {
    const NM_MOD: u64 = 195;
    const TK_MOD: u64 = 156;
    if level != 90 {
        return 0;
    }
    // for the future
    // let out = match level {
    //     0..=50 => 75,
    //     51..=70 => (level as u64 - 50) * 5 / 2 + 75,
    //     71..=80 => (level as u64 - 70) * 4 + 125,
    //     _ => (level as u64 - 80) * 3 + 165,
    // };
    if job.tank() {
        TK_MOD
    } else {
        NM_MOD
    }
}

/// Returns the entry for the field of a job in the job stat datasheet.
pub const fn job(job: Job, field: JobField) -> u64 {
    let (hp, mp, str, vit, dex, int, mnd) = if let Some(v) = embed_data!(
        "./sheets/data_job.csv",
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

/// Returns the entry for the field of a leve in the level datasheet.
pub const fn level(level: u8, field: LevelField) -> u64 {
    if level != 90 {
        return 0;
    }
    // just forcing level 90 for now, not like anyone actually gives a shit about lower levels
    // fuck you ucob speedrunners i guess
    let (main, sub, div) = if let Some(v) = embed_data!("./sheets/data_level.csv", level, u8, u64) {
        v
    } else {
        (0, 0, 0)
    };
    match field {
        LevelField::MAIN => main,
        LevelField::SUB => sub,
        LevelField::DIV => div,
    }
}

/// Returns the entry for the field of a clan in the clan stat datasheet.
pub const fn clan(clan: Clan, field: ClanField) -> i8 {
    let (str, dex, vit, int, mnd) = if let Some(v) = embed_data!(
        "./sheets/data_clan.csv",
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
