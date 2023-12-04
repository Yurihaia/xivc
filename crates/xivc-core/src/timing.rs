#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    math::SpeedStat,
    world::{EventError, EventProxy},
};

pub trait DurationInfo {
    fn get_cast(&self, base: u64, lock: u64, stat: Option<SpeedStat>) -> CastInfo {
        let cast = stat
            .map(|stat| self.get_duration(base, stat))
            .unwrap_or_else(|| base);
        CastInfo {
            lock: match cast {
                0 => lock + self.extra_ani_lock(),
                v => v + 10,
            },
            snap: match cast {
                0 => 0,
                v => v.saturating_sub(50),
            },
        }
    }
    fn extra_ani_lock(&self) -> u64;
    fn get_duration(&self, base: u64, stat: SpeedStat) -> u64;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct CastInfo {
    pub lock: u64,
    pub snap: u64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default)]
pub struct JobCds<J> {
    pub lock: u16,
    pub gcd: u16,
    pub job: J,
}

impl<J> JobCds<J> {
    pub fn advance(&mut self, time: u32) {
        self.lock = (self.lock as u32).saturating_sub(time) as u16;
        self.gcd = (self.gcd as u32).saturating_sub(time) as u16;
    }

    pub fn set_gcd(
        &mut self,
        p: &mut impl EventProxy,
        di: &impl DurationInfo,
        dur: u16,
        stat: Option<SpeedStat>,
    ) {
        if self.gcd > 0 {
            p.error(EventError::Gcd);
        }
        let gcd = stat
            .map(|v| di.get_duration(dur as u64, v))
            .unwrap_or(dur as u64);

        // "normal" gcd speeds from 1.5s and above cannot be brough down
        // to below 1.5s. however, gcds with a base recast of
        // less than 1.5s are not subject to that of course.
        if dur >= 1500 {
            self.gcd = gcd.max(1500) as u16;
        } else {
            self.gcd = gcd as u16;
        }
    }

    pub fn set_gcd_no_min(
        &mut self,
        p: &mut impl EventProxy,
        di: &impl DurationInfo,
        dur: u16,
        stat: Option<SpeedStat>,
    ) {
        if self.gcd > 0 {
            p.error(EventError::Gcd);
        }
        self.gcd = stat
            .map(|v| di.get_duration(dur as u64, v))
            .unwrap_or(dur as u64) as u16;
    }

    pub fn set_cast_lock(
        &mut self,
        p: &mut impl EventProxy,
        di: &impl DurationInfo,
        cast: u16,
        lock: u16,
        stat: Option<SpeedStat>,
    ) -> u32 {
        if self.lock > 0 {
            p.error(EventError::Lock);
        }
        let cast = di.get_cast(cast as u64, lock as u64, stat);
        self.lock = cast.lock as u16;
        cast.snap as u32
    }
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
            pub fn apply(&mut self, ac: $acty, cd: u32, charges: u8) {
                match ac {
                    $(
                        $(<$acty>::$aci)|+ => self.$cdsf_name.apply(cd, charges),
                    )*
                    _ => (),
                }
            }

            pub fn available(&self, ac: $acty, cd: u32, charges: u8) -> bool {
                match ac {
                    $(
                        $(<$acty>::$aci)|+ => self.$cdsf_name.available(cd, charges),
                    )*
                    _ => true,
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
        
        impl core::convert::TryFrom<$acty> for $cdg_id {
            type Error = ();
            
            fn try_from(value: $acty) -> Result<Self, ()> {
                Ok(match value {
                    $(
                        $(<$acty>::$aci)|+ => Self::$cdgv_name,
                    )*
                    _ => return Err(())
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
