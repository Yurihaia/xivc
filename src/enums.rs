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


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
}