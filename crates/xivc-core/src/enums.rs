//! Various enums for data used in the crate.
//!

use core::fmt::{self, Display};
use macros::var_consts;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::math::ActionStat;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[var_consts {
    /// Returns the human readable name of the clan's base race.
    pub const race_name: &'static str
    /// Returns the human readable name of the clan.
    pub const clan_name: &'static str
}]
/// Clans that a character can be.
pub enum Clan {
    /// The Sea Wolves Roegadyn clan.
    #[race_name = "Roegadyn"]
    #[clan_name = "Sea Wolves"]
    SeaWolves,
    /// The Hellsguard Roegadyn clan.
    #[race_name = "Roegadyn"]
    #[clan_name = "Hellsguard"]
    Hellsguard,
    /// The Highlander Hyur clan.
    #[race_name = "Hyur"]
    #[clan_name = "Highlander"]
    Highlander,
    /// The Midlander Hyur clan.
    #[race_name = "Hyur"]
    #[clan_name = "Midlander"]
    Midlander,
    /// The Wildwood Elezen clan.
    #[race_name = "Elezen"]
    #[clan_name = "Wildwood"]
    Wildwood,
    /// The Duskwight Elezen clan.
    #[race_name = "Elezen"]
    #[clan_name = "Duskwight"]
    Duskwight,
    /// The Seeker of the Sun Miqo'te clan.
    #[race_name = "Miqo'te"]
    #[clan_name = "Seeker of the Sun"]
    Sun,
    /// The Keeper of the Moon Miqo'te clan.
    #[race_name = "Miqo'te"]
    #[clan_name = "Keeper of the Moon"]
    Moon,
    /// The Plainsfolk Lalafell clan.
    #[race_name = "Lalafell"]
    #[clan_name = "Plainsfolk"]
    Plainsfolk,
    /// The Dunesfolk Lalafell clan.
    #[race_name = "Lalafell"]
    #[clan_name = "Dunesfolk"]
    Dunesfolk,
    /// The Xaela Au Ra clan.
    #[race_name = "Au Ra"]
    #[clan_name = "Xaela"]
    Xaela,
    /// The Raen Au Ra clan.
    #[race_name = "Au Ra"]
    #[clan_name = "Raen"]
    Raen,
    /// The Rava Viera clan.
    #[race_name = "Viera"]
    #[clan_name = "Rava"]
    Rava,
    /// The Veena Viera clan.
    #[race_name = "Viera"]
    #[clan_name = "Veena"]
    Veena,
    /// The Helion Hrothgar clan.
    #[race_name = "Hrothgar"]
    #[clan_name = "Helion"]
    Helion,
    /// The Lost Hrothgar clan.
    #[race_name = "Hrothgar"]
    #[clan_name = "The Lost"]
    TheLost,
}
impl Display for Clan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} / {}", self.race_name(), self.clan_name())
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[allow(clippy::upper_case_acronyms)] // this is literally the way FF14 does it so I'm not gonna change it :)))
#[var_consts {
    /// Returns `true` if the job is a tank.
    pub const tank
    /// Returns `true` if the job is a healer.
    pub const healer
    /// Returns `true` if the job is a melee DPS.
    pub const melee
    /// Returns `true` if the job is a physical ranged DPS.
    pub const ranged
    /// Returns `true` if the job is a magical ranged DPS.
    pub const caster
    /// Returns `true` if the job is a limited job.
    pub const limited
    /// Returns `true` if the job has an associated soul crystal.
    pub const job: bool = true
    /// Returns the human friendly name of the job.
    pub const name: &'static str
}]
/// Jobs that a character can be.
pub enum Job {
    /// The tank class Gladiator.
    #[tank]
    #[name = "Gladiator"]
    #[job = false]
    GLA,
    /// The melee DPS class Pugilist.
    #[melee]
    #[name = "Pugilist"]
    #[job = false]
    PGL,
    /// The tank class Marauder.
    #[tank]
    #[name = "Marauder"]
    #[job = false]
    MRD,
    /// The melee DPS class Lancer.
    #[melee]
    #[name = "Lancer"]
    #[job = false]
    LNC,
    /// The physical ranged DPS class Archer.
    #[ranged]
    #[name = "Archer"]
    #[job = false]
    ARC,
    /// The healer class Conjurer.
    #[healer]
    #[name = "Conjurer"]
    #[job = false]
    CNJ,
    /// The magical ranged DPS class Thaumaturge.
    #[caster]
    #[name = "Thaumaturge"]
    #[job = false]
    THM,
    /// The tank job Paladin.
    #[tank]
    #[name = "Paladin"]
    PLD,
    /// The melee DPS job Monk.
    #[melee]
    #[name = "Monk"]
    MNK,
    /// The tank job Warrior.
    #[tank]
    #[name = "Warrior"]
    WAR,
    /// The melee DPS job Dragoon.
    #[melee]
    #[name = "Dragoon"]
    DRG,
    /// The physical ranged DPS job Bard.
    #[ranged]
    #[name = "Bard"]
    BRD,
    /// The healer job White Mage.
    #[healer]
    #[name = "White Mage"]
    WHM,
    /// The magical ranged DPS job Black Mage.
    #[caster]
    #[name = "Black Mage"]
    BLM,
    /// The magical ranged DPS class Arcanist.
    #[caster]
    #[name = "Arcanist"]
    #[job = false]
    ACN,
    /// The magical ranged DPS job Summoner.
    #[caster]
    #[name = "Summoner"]
    SMN,
    /// The healer job Scholar.
    #[healer]
    #[name = "Scholar"]
    SCH,
    /// The melee DPS class Rogue.
    #[melee]
    #[name = "Rogue"]
    #[job = false]
    ROG,
    /// The melee DPS job Ninja.
    #[melee]
    #[name = "Ninja"]
    NIN,
    /// The physical ranged DPS job Machinist.
    #[ranged]
    #[name = "Machinist"]
    MCH,
    /// The tank job Dark Knight.
    #[tank]
    #[name = "Dark Knight"]
    DRK,
    /// The healer job Astrologian.
    #[healer]
    #[name = "Astrologian"]
    AST,
    /// The melee DPS job Samurai.
    #[melee]
    #[name = "Samurai"]
    SAM,
    /// The magical ranged DPS job Red Mage.
    #[caster]
    #[name = "Red Mage"]
    RDM,
    /// The limited magical ranged DPS job Blue Mage.
    #[caster]
    #[name = "Blue Mage"]
    #[limited]
    BLU,
    /// The tank job Gunbreaker.
    #[tank]
    #[name = "Gunbreaker"]
    GNB,
    /// The physical ranged DPS job Dancer.
    #[ranged]
    #[name = "Dancer"]
    DNC,
    /// The melee DPS job Reaper.
    #[melee]
    #[name = "Reaper"]
    RPR,
    /// The healer job Sage.
    #[healer]
    #[name = "Sage"]
    SGE,
}
impl Job {
    /// Returns the stat used to attack for the job.
    pub const fn attack_stat(&self) -> ActionStat {
        if self.caster() || self.healer() {
            ActionStat::AttackMagic
        } else {
            ActionStat::AttackPower
        }
    }
}
impl Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[allow(missing_docs)]
/// Elements that damage can be.
pub enum DamageElement {
    None,
    Fire,
    Earth,
    Ice,
    Water,
    Wind,
    Lightning,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[var_consts {
    /// Returns `true` if the damage type is physical.
    pub const physical
    /// Returns `true` if the damage type is magical.
    pub const magical
    /// Returns `true` if the damage type is unique.
    pub const unique
}]
/// The types that damage can be.
pub enum DamageType {
    /// Slashing physical damage.
    #[physical]
    Slashing,
    /// Piercing physical damage.
    #[physical]
    Piercing,
    /// Blunt physical damage.
    #[physical]
    Blunt,
    /// Magical damage.
    #[magical]
    Magical,
    /// Unique damage.
    ///
    /// This has also been known as "Darkness" damage.
    /// Often instances of damage that do a fixed amount will be this type.
    #[unique]
    Unique,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
/// A instance of premodified damage.
pub struct DamageInstance {
    /// The potency of the damage instance.
    pub potency: u64,
    /// The element of the damage.
    pub dmg_el: DamageElement,
    /// The type of the damage.
    pub dmg_ty: DamageType,
    /// Should the damage force a critical hit.
    pub force_ch: bool,
    /// Should the damage force a direct hit.
    pub force_dh: bool,
}

impl DamageInstance {
    /// Creates a new damage instance.
    ///
    /// The default damage type is [`Unique`], and the default element is [`None`][elnone].
    /// Every single job action will want to set one of [`slashing`], [`piercing`], [`blunt`],
    /// or [`magical`].
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageElement, DamageType, DamageInstance};
    /// let damage = DamageInstance::new(100);
    ///
    /// assert_eq!(damage.dmg_el, DamageElement::None);
    /// assert_eq!(damage.dmg_ty, DamageType::Unique);
    /// assert_eq!(damage.force_ch, false);
    /// assert_eq!(damage.force_dh, false);
    /// ```
    ///
    /// [`Unique`]: DamageType::Unique
    /// [elnone]: DamageElement::None
    /// [`slashing`]: DamageInstance::slashing
    /// [`piercing`]: DamageInstance::piercing
    /// [`blunt`]: DamageInstance::blunt
    /// [`magical`]: DamageInstance::magical
    pub const fn new(potency: u64) -> Self {
        Self {
            potency,
            dmg_ty: DamageType::Unique,
            dmg_el: DamageElement::None,
            force_ch: false,
            force_dh: false,
        }
    }
    /// Sets the damage type of this damage instance to physical slashing damage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageType, DamageInstance};
    /// let damage = DamageInstance::new(300).slashing();
    ///
    /// assert_eq!(damage.dmg_ty, DamageType::Slashing);
    /// ```
    pub const fn slashing(mut self) -> Self {
        self.dmg_ty = DamageType::Slashing;
        self
    }
    /// Sets the damage type of this damage instance to physical piercing damage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageType, DamageInstance};
    /// let damage = DamageInstance::new(520).piercing();
    ///
    /// assert_eq!(damage.dmg_ty, DamageType::Piercing);
    /// ```
    pub const fn piercing(mut self) -> Self {
        self.dmg_ty = DamageType::Piercing;
        self
    }
    /// Sets the damage type of this damage instance to physical blunt damage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageType, DamageInstance};
    /// let damage = DamageInstance::new(150).blunt();
    ///
    /// assert_eq!(damage.dmg_ty, DamageType::Blunt);
    /// ```
    pub const fn blunt(mut self) -> Self {
        self.dmg_ty = DamageType::Blunt;
        self
    }
    /// Sets the damage type of this damage instance to magical damage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageType, DamageInstance};
    /// let damage = DamageInstance::new(200).magical();
    ///
    /// assert_eq!(damage.dmg_ty, DamageType::Magical);
    /// ```
    pub const fn magical(mut self) -> Self {
        self.dmg_ty = DamageType::Magical;
        self
    }
    /// Sets the damage type of this damage instance to unique damage.
    ///
    /// This is not strictly needed as the default damage type is unique damage.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageType, DamageInstance};
    /// let damage = DamageInstance::new(500).unique();
    ///
    /// assert_eq!(damage.dmg_ty, DamageType::Unique);
    /// ```
    pub const fn unique(mut self) -> Self {
        self.dmg_ty = DamageType::Unique;
        self
    }
    /// Sets the damage element of this damage instance to the specified element.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::{DamageElement, DamageInstance};
    /// let damage = DamageInstance::new(300).element(DamageElement::Fire);
    ///
    /// assert_eq!(damage.dmg_el, DamageElement::Fire);
    /// ```
    pub const fn element(mut self, element: DamageElement) -> Self {
        self.dmg_el = element;
        self
    }
    /// Forces this damage instance to critical hit.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::DamageInstance;
    /// let damage = DamageInstance::new(1100).force_crit();
    ///
    /// assert_eq!(damage.force_ch, true);
    /// ```
    pub const fn force_crit(mut self) -> Self {
        self.force_ch = true;
        self
    }
    /// Forces this damage instance to direct hit.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::enums::DamageInstance;
    /// let damage = DamageInstance::new(620).force_dhit();
    ///
    /// assert_eq!(damage.force_dh, true);
    /// ```
    pub const fn force_dhit(mut self) -> Self {
        self.force_dh = true;
        self
    }
}
