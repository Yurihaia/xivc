#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::math::SpeedStat;

pub trait DurationInfo {
    fn get_cast(&self, base: ScaleTime, lock: u16) -> (u16, u16) {
        let cast = self.get_duration(base) as u16;
        (
            match cast {
                0 => lock + self.extra_ani_lock(),
                v => v + 10,
            },
            match cast {
                0 => 0,
                v => v.saturating_sub(50),
            },
        )
    }
    fn extra_ani_lock(&self) -> u16;
    fn get_duration(&self, duration: ScaleTime) -> u64;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct CastInfo {
    pub lock: u64,
    pub snap: u64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default)]
pub struct ActionCd {
    pub cd: u32,
}

impl ActionCd {
    pub fn apply(&mut self, cd: u32, charges: u8) {
        self.cd = (self.cd + cd).min(cd * charges as u32)
    }
    pub fn available(&self, cd: u32, charges: u8) -> bool {
        self.cd <= cd * charges as u32
    }
    pub fn advance(&mut self, time: u32) {
        self.cd = self.cd.saturating_sub(time)
    }
}

#[macro_export]
macro_rules! job_cd_struct {
    (
        $acty:ty =>

        $(#[$cds_meta:meta])*
        $cds_vis:vis $cds_id:ident

        $(#[$cdg_meta:meta])*
        $cdg_vis:vis $cdg_id:ident

        $(
            $cdsf_name:ident $cdgv_name:ident: $($aci:ident)+;
        )*
    ) => {
        $(#[$cds_meta])*
        $cds_vis struct $cds_id {
            $(
                $cdsf_name: $crate::timing::ActionCd,
            )*
        }

        $(#[$cdg_meta])*
        $cdg_vis enum $cdg_id {
            $(
                $cdgv_name,
            )*
        }

        impl $cds_id {
            pub fn apply(&mut self, cdg: $cdg_id, cooldown: u32, charges: u8) {
                match cdg {
                    $(
                        $cdg_id::$cdgv_name => self.$cdsf_name.apply(cooldown, charges),
                    )*
                }
            }

            pub fn available(&self, cdg: $cdg_id, cooldown: u32, charges: u8) -> bool {
                match cdg {
                    $(
                        $cdg_id::$cdgv_name => self.$cdsf_name.available(cooldown, charges),
                    )*
                }
            }

            pub fn advance(&mut self, time: u32) {
                $(self.$cdsf_name.advance(time);)*
            }

            pub fn cd_for(&self, group: $cdg_id) -> u32 {
                match group {
                    $(
                        $cdg_id::$cdgv_name => self.$cdsf_name.cd,
                    )*
                }
            }
        }
        
        impl $acty {
            pub fn cd_group(self) -> Option<$cdg_id> {
                Some(match self {
                    $(
                        $(Self::$aci)|+ => $cdg_id::$cdgv_name,
                    )*
                    _ => return None
                })
            }
        }
    };
}

pub struct EventCascade {
    amount: u32,
    time: u32,
}

impl EventCascade {
    pub const fn new(start: u32) -> Self {
        Self {
            time: start,
            amount: 133,
        }
    }
    pub const fn with_amount(start: u32, amount: u32) -> Self {
        Self {
            time: start,
            amount,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u32 {
        let out = self.time;
        self.time += self.amount;
        out
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ScaleTime(pub u32, pub Option<SpeedStat>, pub bool);

impl ScaleTime {
    pub const fn zero() -> Self {
        Self(0, None, false)
    }

    pub const fn skill(duration: u32) -> Self {
        Self(duration, Some(SpeedStat::SkillSpeed), true)
    }

    pub const fn spell(duration: u32) -> Self {
        Self(duration, Some(SpeedStat::SpellSpeed), true)
    }

    pub const fn none(duration: u32) -> Self {
        Self(duration, None, false)
    }

    pub const fn duration(&self) -> u32 {
        self.0
    }

    pub const fn stat(&self) -> Option<SpeedStat> {
        self.1
    }

    pub const fn effectable(&self) -> bool {
        self.2
    }

    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
}
