use std::fmt::Debug;

use xivc_core::{jobs::sam, math::SpeedStat, timing::DurationInfo, world::World};

pub struct SimpleTimelineInfo<C> {
    pub gcd: u16,
    pub lock: u16,
    pub cd: Option<(u32, C)>,
}

pub trait TimelineInfoExt {
    type State: Clone + Default + Debug;
    type CooldownGroup: Copy + Debug;

    fn timeline_info<W: World>(
        &self,
        state: &Self::State,
        world: &W,
    ) -> SimpleTimelineInfo<Self::CooldownGroup>;
}

impl TimelineInfoExt for sam::SamAction {
    type State = sam::SamState;
    type CooldownGroup = sam::SamCdGroup;

    fn timeline_info<W: World>(
        &self,
        _: &Self::State,
        world: &W,
    ) -> SimpleTimelineInfo<Self::CooldownGroup> {
        let di = world.duration_info();

        let gcd = if self.gcd() {
            di.get_duration(2500, SpeedStat::SkillSpeed)
        } else {
            0
        } as u16;
        // all sam casts are never modified by skill speed
        let lock = di.get_cast(self.cast() as u64, 600, None).lock as u16;

        let cd = sam::SamCdGroup::try_from(*self)
            .ok()
            .map(|v| (self.cooldown(), v));

        SimpleTimelineInfo { gcd, lock, cd }
    }
}

// macro_rules! default_infext_impl {
//     ($ac_type:ident $is_gcd:ident $base_gcd:ident $base_cd:ident $state:ty ) => {

//     };
// }
