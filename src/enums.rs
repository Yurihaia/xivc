use std::fmt::{self, Display};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Clan {
    SeaWolves,
    Hellsguard,
    Highlander,
    Midlander,
    Wildwood,
    Duskwight,
    Sun,
    Moon,
    Plainsfolk,
    Dunesfolk,
    Xaela,
    Raen,
    Rava,
    Veena,
    Helion,
    TheLost,
}
impl Display for Clan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.clan_name())
    }
}
impl Clan {
    pub const fn race_name(&self) -> &'static str {
        use Clan::*;
        match self {
            SeaWolves | Hellsguard => "Roegadyn",
            Highlander | Midlander => "Hyur",
            Wildwood | Duskwight => "Elezen",
            Sun | Moon => "Miqo'te",
            Plainsfolk | Dunesfolk => "Lalafell",
            Xaela | Raen => "Au Ra",
            Rava | Veena => "Viera",
            Helion | TheLost => "Hrothgar",
        }
    }
    pub const fn clan_name(&self) -> &'static str {
        use Clan::*;
        match self {
            SeaWolves => "Sea Wolves",
            Hellsguard => "Hellguard",
            Highlander => "Highlander",
            Midlander => "Midlander",
            Wildwood => "Wildwood",
            Duskwight => "Duskwight",
            Sun => "Seeker of the Sun",
            Moon => "Keeper of the Moon",
            Plainsfolk => "Plainsfolf",
            Dunesfolk => "Dunesfolk",
            Xaela => "Xaela",
            Raen => "Raen",
            Rava => "Rava",
            Veena => "Veena",
            Helion => "Helion",
            TheLost => "The Lost",
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(clippy::upper_case_acronyms)] // this is literally the way FF14 does it so I'm not gonna change it :)))
pub enum Job {
    GLA,
    PGL,
    MRD,
    LNC,
    ARC,
    CNJ,
    THM,
    PLD,
    MNK,
    WAR,
    DRG,
    BRD,
    WHM,
    BLM,
    ACN,
    SMN,
    SCH,
    ROG,
    NIN,
    MCH,
    DRK,
    AST,
    SAM,
    RDM,
    BLU,
    GNB,
    DNC,
}
impl Job {
    pub const fn tank(&self) -> bool {
        use Job::*;
        matches!(self, GLA | MRD | PLD | WAR | DRK | GNB)
    }
    pub const fn healer(&self) -> bool {
        use Job::*;
        matches!(self, CNJ | WHM | SCH | AST)
    }
    pub const fn melee(&self) -> bool {
        use Job::*;
        matches!(self, PGL | MNK | LNC | DRG | ROG | NIN | SAM)
    }
    pub const fn phys_ranged(&self) -> bool {
        use Job::*;
        matches!(self, ARC | BRD | MCH | DNC)
    }
    pub const fn caster(&self) -> bool {
        use Job::*;
        matches!(self, THM | BLM | ACN | SMN | RDM | BLU)
    }
    pub const fn job_name(&self) -> &'static str {
        use Job::*;
        match self {
            GLA => "Gladiator",
            PGL => "Pugilist",
            MRD => "Marauder",
            LNC => "Lancer",
            ARC => "Archer",
            CNJ => "Conjurer",
            THM => "Thaumaturge",
            PLD => "Paladin",
            MNK => "Monk",
            WAR => "Warrior",
            DRG => "Dragoon",
            BRD => "Bard",
            WHM => "White Mage",
            BLM => "Black Mage",
            ACN => "Arcanist",
            SMN => "Summoner",
            SCH => "Scholar",
            ROG => "Rogue",
            NIN => "Ninja",
            MCH => "Machinist",
            DRK => "Dark Knight",
            AST => "Astrologian",
            SAM => "Samurai",
            RDM => "Red Mage",
            BLU => "Blue Mage",
            GNB => "Gunbreaker",
            DNC => "Dancer",
        }
    }
}
impl Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.job_name())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DamageElement {
    None, Fire, Earth, Ice, Water, Wind, Lightning
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DamageType {
    Slashing, Piercing, Blunt, Magic
}
