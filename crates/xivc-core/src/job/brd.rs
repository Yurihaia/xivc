use core::fmt::{self, Display};

use macros::var_consts;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    bool_job_dist,
    enums::{ActionCategory, DamageInstance},
    job::{CastInitInfo, Job, JobState},
    job_cd_struct, job_effect_wrapper,
    math::SpeedStat,
    need_target, status_effect,
    timing::{DurationInfo, EventCascade, ScaleTime},
    util::GaugeU8,
    world::{
        status::{consume_status, JobEffect, StatusEffect, StatusEventExt},
        Action, ActionTargetting, Actor, ActorId, DamageEventExt, Event, EventError, EventSink,
        Faction, World,
    },
};

use super::{JobAction, JobEvent};

/// The [`Job`] struct for Bard.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrdJob;

/// The status effect "Straight Shot Ready".
pub const STRAIGHT_SHOT: StatusEffect = status_effect!("Straight Shot Ready" 30000);
/// The status effect "Shadowbite Ready".
pub const SHADOWBITE: StatusEffect = status_effect!("Shadowbite Ready" 30000);
/// The status effect "Blast Arrow Ready".
pub const BLAST_ARROW: StatusEffect = status_effect!("Blast Arrow Ready" 10000);
/// The status effect "Raging Strikes".
pub const RAGING_STRIKES: StatusEffect = status_effect!(
    "Raging Strikes" 20000 { damage { out = 115 / 100 } }
);
/// The status effect "Barrage".
pub const BARRAGE: StatusEffect = status_effect!("Barrage" 10000);
/// The status effect "Battle Voice".
pub const BATTLE_VOICE: StatusEffect = status_effect!(
    "Battle Voice" 15000 { dhit { out = 200 } }
);
// technically doesn't use stacks in game but
// it shouldn't matter really. it works well enough
/// The status effect "Radiant Finale".
pub const RADIANT_FINALE: StatusEffect = status_effect!(
    "Radiant Finale" 15000 { damage { out = |s, d, _, _| {
        d * (s.stack as u64 * 2 + 100) / 100
    } } }
);
// despite appearing permanent, these are actually buffs with a 5s duration
// that get refreshed every 3 seconds
/// The status effect "Mage's Ballad".
pub const BALLAD: StatusEffect = status_effect!(
    "Mage's Ballad" 5000 { damage { out = 11 / 10 } }
);
/// The status effect "Army's Paeon".
pub const PAEON: StatusEffect = status_effect!(
    "Army's Paeon" 5000 { dhit { out = 30 } }
);
/// The status effect "The Wanderer's Minuet".
pub const MINUET: StatusEffect = status_effect!(
    "The Wanderer's Minuet"  5000 { crit { out = 20 }}
);
/// The status effect "Troubadour".
pub const TROUBADOUR: StatusEffect = status_effect!(
    "Troubadour" 15000 { damage { in = 9 / 10 } }
);
/// The status effect "Army's Ethos".
pub const ETHOS: StatusEffect = status_effect!("Army's Ethos" 30000);
// again, just like radiant finale, use stacks to store the rep stacks
/// The status effect "Army's Muse".
pub const MUSE: StatusEffect = status_effect!(
    "Army's Muse" 10000 { haste { |i| 100 - match i.stack {
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 12,
        _ => 0,
    } } }
);
// dots
/// The DoT effect "Caustic Bite".
pub const CAUSTIC_BITE: StatusEffect = status_effect!("Caustic Bite" 45000);
/// The DoT effect "Stormbite".
pub const STORMBITE: StatusEffect = status_effect!("Stormbite" 45000);
// TODO: minne and paean
// this can be done if/when healing is implemented

const SONG_LEN: u16 = 45000;

impl Job for BrdJob {
    type Action = BrdAction;
    type State = BrdState;
    type CastError = BrdError;
    type Event = BrdEvent;
    type Cds = BrdCds;
    type CdGroup = BrdCdGroup;

    fn check_cast<'w, E: EventSink<'w, W>, W: World>(
        action: Self::Action,
        state: &Self::State,
        _: &'w W,
        event_sink: &mut E,
    ) -> CastInitInfo<Self::CdGroup> {
        let this = event_sink.source();

        let di = this.duration_info();

        let gcd = action.gcd().map(|v| di.scale(v)).unwrap_or_default() as u16;
        let (lock, snap) = di.get_cast(ScaleTime::zero(), 600);

        let cd = action
            .cd_group()
            .map(|v| (v, action.cooldown(), action.cd_charges()));

        let alt_cd = action.cd_group().map(|v| (v, 1000, 1));

        use BrdAction::*;
        match action {
            ApexArrow if state.soul < 20 => BrdError::SoulVoice(20).submit(event_sink),
            StraightShot | RefulgentArrow if !this.has_own_status(STRAIGHT_SHOT) => {
                BrdError::StraightShot.submit(event_sink)
            }
            Shadowbite if !this.has_own_status(SHADOWBITE) => {
                BrdError::Shadowbite.submit(event_sink)
            }
            PitchPerfect => match &state.song {
                Some((BrdSong::Minuet(x), _)) if *x > 0 => (),
                _ => BrdError::PitchPerfect.submit(event_sink),
            },
            BlastArrow if !this.has_own_status(BLAST_ARROW) => {
                BrdError::BlastArrow.submit(event_sink)
            }
            RadiantFinale if state.coda.count() == 0 => BrdError::Coda.submit(event_sink),
            _ => (),
        }

        CastInitInfo {
            gcd,
            lock,
            snap,
            cd,
            alt_cd,
        }
    }

    fn cast_snap<'w, E: EventSink<'w, W>, W: World>(
        action: Self::Action,
        state: &mut Self::State,
        _: &'w W,
        event_sink: &mut E,
    ) {
        let this = event_sink.source();
        let this_id = this.id();

        use BrdAction::*;

        let target_enemy = |t: ActionTargetting| {
            this.actors_for_action(Some(Faction::Enemy), t)
                .map(|a| a.id())
        };

        let dl = action.effect_delay();

        // handle a damage event that can be tripled by Barrage
        let barrage = |event_sink: &mut E, damage: DamageInstance, target: ActorId, delay: u32| {
            let mut cascade = EventCascade::new(delay, 1);
            event_sink.damage(action, damage, target, cascade.next());
            if this.has_own_status(BARRAGE) {
                event_sink.remove_status(BARRAGE, this_id, 0);
                event_sink.damage(action, damage, target, cascade.next());
                event_sink.damage(action, damage, target, cascade.next());
            }
        };

        match action {
            HeavyShot | BurstShot => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                if event_sink.random(StraightShotProc) {
                    event_sink.apply_status(STRAIGHT_SHOT, 1, this_id, 0);
                }
                barrage(event_sink, DamageInstance::new(220).piercing(), t, dl);
            }
            StraightShot | RefulgentArrow => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                // seems to be instantly from testing
                event_sink.remove_status(STRAIGHT_SHOT, this_id, 0);
                barrage(event_sink, DamageInstance::new(280).piercing(), t, dl);
            }
            RagingStrikes => {
                event_sink.apply_status(RAGING_STRIKES, 1, this_id, dl);
            }
            VenomousBite | CausticBite => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                if event_sink.random(StraightShotProc) {
                    event_sink.apply_status(STRAIGHT_SHOT, 1, this_id, 0);
                }
                event_sink.apply_dot(
                    CAUSTIC_BITE,
                    DamageInstance::new(20).piercing(),
                    SpeedStat::SkillSpeed,
                    1,
                    t,
                    dl,
                );
                barrage(event_sink, DamageInstance::new(150).piercing(), t, dl);
            }
            Bloodletter => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                barrage(event_sink, DamageInstance::new(110).piercing(), t, dl);
            }
            QuickNock | Ladonsbite => {
                let iter = need_target!(
                    target_enemy(ActionTargetting::cone(12, 90)),
                    event_sink,
                    uaoe
                );
                let mut cascade = EventCascade::new(dl, 1);
                for t in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(130).piercing(),
                        t,
                        cascade.next(),
                    );
                }
                if event_sink.random(ShadowbiteProc) {
                    event_sink.apply_status(SHADOWBITE, 1, this_id, 0);
                }
            }
            Windbite | Stormbite => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                if event_sink.random(StraightShotProc) {
                    event_sink.apply_status(STRAIGHT_SHOT, 1, this_id, 0);
                }
                event_sink.apply_dot(
                    STORMBITE,
                    DamageInstance::new(25).piercing(),
                    SpeedStat::SkillSpeed,
                    1,
                    t,
                    dl,
                );
                barrage(event_sink, DamageInstance::new(100).piercing(), t, dl);
            }
            Barrage => {
                event_sink.apply_status(BARRAGE, 1, this_id, 0);
                event_sink.apply_status(STRAIGHT_SHOT, 1, this_id, 0);
            }
            MagesBallad => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                // apply army's muse
                if let Some((BrdSong::Paeon(rep), _)) = &state.song {
                    if rep.value() > 0 {
                        event_sink.apply_status(MUSE, rep.value(), this_id, 0);
                    }
                } else if let Some(ethos) = this.get_own_status(ETHOS) {
                    event_sink.remove_status(ETHOS, this_id, 0);
                    event_sink.apply_status(MUSE, ethos.stack, this_id, 0);
                }
                // update song
                state.song = Some((BrdSong::Ballad, SONG_LEN));
                state.song_gen = state.song_gen.wrapping_add(1);
                // update song statuses
                event_sink.remove_status(PAEON, this_id, 0);
                event_sink.remove_status(MINUET, this_id, 0);
                event_sink.apply_status(BALLAD, 1, this_id, 0);
                // create a tick event
                event_sink.event(
                    JobEvent::brd(BrdEvent::song_tick(state.song_gen), this_id),
                    3000,
                );
                // apply damage
                event_sink.damage(action, DamageInstance::new(100).magical(), t, dl);
            }
            ArmysPaeon => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                // don't worry about army's muse because valid uses of paeon won't
                // be able to apply them
                // update song
                state.song = Some((BrdSong::Paeon(Default::default()), SONG_LEN));
                state.song_gen = state.song_gen.wrapping_add(1);
                // update song statuses
                event_sink.remove_status(BALLAD, this_id, 0);
                event_sink.remove_status(MINUET, this_id, 0);
                event_sink.apply_status(PAEON, 1, this_id, 0);
                // create a tick event
                event_sink.event(
                    JobEvent::brd(BrdEvent::song_tick(state.song_gen), this_id),
                    3000,
                );
                // apply damage
                event_sink.damage(action, DamageInstance::new(100).magical(), t, dl);
            }
            RainOfDeath => {
                let iter = need_target!(
                    target_enemy(ActionTargetting::target_circle(8, 25)),
                    event_sink,
                    uaoe
                );
                let mut cascade = EventCascade::new(dl, 1);
                for t in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(100).piercing(),
                        t,
                        cascade.next(),
                    );
                }
            }
            BattleVoice => {
                let iter = this
                    .actors_for_action(Some(Faction::Party), ActionTargetting::circle(30))
                    .map(|a| a.id());
                let mut cascade = EventCascade::new(dl, 3);
                for t in iter {
                    event_sink.apply_status(BATTLE_VOICE, 1, t, cascade.next());
                }
            }
            WanderersMinuet => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                // apply army's muse
                if let Some((BrdSong::Paeon(rep), _)) = &state.song {
                    if rep.value() > 0 {
                        event_sink.apply_status(MUSE, rep.value(), this_id, 0);
                    }
                } else if let Some(ethos) = this.get_own_status(ETHOS) {
                    event_sink.remove_status(ETHOS, this_id, 0);
                    event_sink.apply_status(MUSE, ethos.stack, this_id, 0);
                }
                // update song
                state.song = Some((BrdSong::Minuet(Default::default()), SONG_LEN));
                state.song_gen = state.song_gen.wrapping_add(1);
                // update song statuses
                event_sink.remove_status(BALLAD, this_id, 0);
                event_sink.remove_status(PAEON, this_id, 0);
                event_sink.apply_status(MINUET, 1, this_id, 0);
                // create a tick event
                event_sink.event(
                    JobEvent::brd(BrdEvent::song_tick(state.song_gen), this_id),
                    3000,
                );
                // apply damage
                event_sink.damage(action, DamageInstance::new(100).magical(), t, dl);
            }
            EmpyrealArrow => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                repertoire(state, this_id, event_sink);
                event_sink.damage(action, DamageInstance::new(240).piercing(), t, dl);
            }
            IronJaws => {
                let target_actor = need_target!(
                    this.actors_for_action(Some(Faction::Enemy), RANGED).next(),
                    event_sink
                );
                if event_sink.random(StraightShotProc) {
                    event_sink.apply_status(STRAIGHT_SHOT, 1, this_id, 0);
                }
                let t = target_actor.id();
                // if the target has stormbite/caustic bite, reapply them.
                if target_actor.has_status(STORMBITE, this_id) {
                    event_sink.apply_dot(
                        STORMBITE,
                        DamageInstance::new(25).piercing(),
                        SpeedStat::SkillSpeed,
                        1,
                        t,
                        dl,
                    );
                }
                if target_actor.has_status(CAUSTIC_BITE, this_id) {
                    event_sink.apply_dot(
                        CAUSTIC_BITE,
                        DamageInstance::new(20).piercing(),
                        SpeedStat::SkillSpeed,
                        1,
                        t,
                        dl,
                    );
                }
                barrage(event_sink, DamageInstance::new(100).piercing(), t, dl);
            }
            Sidewinder => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                event_sink.damage(action, DamageInstance::new(320).piercing(), t, dl);
            }
            Shadowbite => {
                let iter = need_target!(
                    target_enemy(ActionTargetting::target_circle(5, 25)),
                    event_sink,
                    uaoe
                );
                let barrage = consume_status(event_sink, BARRAGE, 0);
                event_sink.remove_status(SHADOWBITE, this_id, 0);
                let potency = if barrage { 270 } else { 170 };
                for t in iter {
                    event_sink.damage(action, DamageInstance::new(potency).piercing(), t, dl);
                }
            }
            ApexArrow => {
                let iter = need_target!(target_enemy(LINE), event_sink, uaoe);
                let potency = *state.soul * 5;
                if state.soul >= 80 {
                    event_sink.apply_status(BLAST_ARROW, 1, this_id, 0);
                }
                state.soul.clear();
                let mut cascade = EventCascade::new(dl, 1);
                for t in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(potency as u64).piercing(),
                        t,
                        cascade.next(),
                    );
                }
            }
            RadiantFinale => {
                let iter = this
                    .actors_for_action(Some(Faction::Party), ActionTargetting::circle(30))
                    .map(|a| a.id());
                let mut cascade = EventCascade::new(dl, 3);
                let stacks = state.coda.count();
                state.coda.clear();
                for t in iter {
                    event_sink.apply_status(RADIANT_FINALE, stacks, t, cascade.next());
                }
            }
            PitchPerfect => {
                let t = need_target!(target_enemy(RANGED).next(), event_sink);
                let rep_stacks = match &mut state.song {
                    Some((BrdSong::Minuet(rep), _)) => {
                        let out = **rep;
                        rep.clear();
                        out
                    }
                    _ => 0,
                };
                let potency = match rep_stacks {
                    1 => 100,
                    2 => 220,
                    3 => 360,
                    _ => 0,
                };
                event_sink.damage(action, DamageInstance::new(potency).piercing(), t, dl);
            }
            BlastArrow => {
                let (first, other) = need_target!(target_enemy(LINE), event_sink, aoe);
                event_sink.remove_status(BLAST_ARROW, this_id, 0);
                let mut cascade = EventCascade::new(dl, 1);
                event_sink.damage(
                    action,
                    DamageInstance::new(600).piercing(),
                    first,
                    cascade.next(),
                );
                for t in other {
                    event_sink.damage(
                        action,
                        DamageInstance::new(240).piercing(),
                        t,
                        cascade.next(),
                    );
                }
            }
            RepellingShot | WardensPaean | NaturesMinne | Troubadour => {
                // unimplemented
            }
        }
    }

    fn event<'w, E: EventSink<'w, W>, W: World>(
        state: &mut Self::State,
        world: &'w W,
        event: &Event,
        event_sink: &mut E,
    ) {
        let this = event_sink.source();
        let this_id = this.id();
        match event {
            Event::Job(JobEvent::Brd(event), src_id) => {
                if *src_id == this_id {
                    match event {
                        BrdEvent::SongTick { gen } => {
                            if let Some((song, time)) = &state.song {
                                // the song generations match
                                if *gen == state.song_gen {
                                    debug_assert!(time % 3000 == 0, "song ticks don't line up");
                                    if *time == 0 {
                                        if let BrdSong::Paeon(rep) = song {
                                            // add army's ethos if paeon is falling off
                                            // with more than 1 stack
                                            if rep.value() > 0 {
                                                event_sink.apply_status(
                                                    ETHOS,
                                                    rep.value(),
                                                    this_id,
                                                    0,
                                                );
                                            }
                                        }
                                        // remove the song now
                                        state.song = None;
                                    } else {
                                        // 80% chance for rep proc
                                        if event_sink.random(RepertoireProc) {
                                            repertoire(state, this_id, event_sink);
                                        }

                                        event_sink.event(
                                            JobEvent::brd(
                                                BrdEvent::song_tick(state.song_gen).into(),
                                                this_id,
                                            ),
                                            3000,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // refresh the song buffs on other players' actor tick.
            Event::ActorTick(other_id) => {
                if let Some((song, _)) = &state.song {
                    let other_id = *other_id;
                    if let Some(tick_actor) = world.actor(other_id) {
                        if tick_actor.faction() == Faction::Party
                            && this.within_range(other_id, ActionTargetting::circle(50))
                        {
                            event_sink.apply_status(song.song_buff(), 1, other_id, 0);
                        }
                    }
                }
            }
            _ => (),
        }
    }

    fn effect<'a>(state: &'a Self::State) -> Option<&'a dyn JobEffect> {
        Some(BrdJobEffect::new(state))
    }
}

job_effect_wrapper! {
    #[derive(Debug)]
    struct BrdJobEffect(BrdState);
}
impl JobEffect for BrdJobEffect {
    fn haste(&self) -> u64 {
        100 - if let Some((BrdSong::Paeon(rep), _)) = &self.0.song {
            rep.value() as u64 * 4
        } else {
            0
        }
    }
}

#[derive(Copy, Clone, Debug)]
/// A custom cast error for Bard actions.
pub enum BrdError {
    /// Not enough Soul Voice gauge.
    SoulVoice(u8),
    /// Not under the effect of Straight Shot Ready.
    StraightShot,
    /// Not under the effect of Shadowbite Ready.
    Shadowbite,
    /// Not enough stacks of Repertoire to cast Pitch Perfect.
    PitchPerfect,
    /// Not under the effect of Blast Arrow Ready.
    BlastArrow,
    /// Not enough Coda.
    Coda,
}
impl BrdError {
    /// Submits the cast error into the [`EventSink`].
    pub fn submit<'w, W: World>(self, event_sink: &mut impl EventSink<'w, W>) {
        event_sink.error(self.into())
    }
}

impl From<BrdError> for EventError {
    fn from(value: BrdError) -> Self {
        Self::Job(value.into())
    }
}

impl Display for BrdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SoulVoice(req) => {
                write!(f, "Not enough Soul Voice gauge, needed at least {}.", req)
            }
            Self::StraightShot => write!(f, "Not under the effect of '{}'.", STRAIGHT_SHOT.name),
            Self::Shadowbite => write!(f, "Not under the effect of '{}'.", SHADOWBITE.name),
            Self::PitchPerfect => write!(
                f,
                "Not enough stacks of Repertoire to cast 'Pitch Perfect'."
            ),
            Self::BlastArrow => write!(f, "Not under the effect of '{}'.", BLAST_ARROW.name),
            Self::Coda => write!(f, "Not enough Coda."),
        }
    }
}

const RANGED: ActionTargetting = ActionTargetting::single(25);
const LINE: ActionTargetting = ActionTargetting::line(25);

#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
#[var_consts {
    /// Returns the base GCD recast time, or `None` if the action is not a gcd.
    pub const gcd: ScaleTime?;
    /// Returns the human friendly name of the action.
    pub const name: &'static str;
    /// Returns the cooldown of the skill in milliseconds.
    pub const cooldown: u32 = 0;
    /// Returns the number of charges a skill has, or `1` if it is a single charge skill.
    pub const cd_charges: u8 = 1;
    /// Returns the delay in milliseconds for the damage/statuses to be applied.
    pub const effect_delay: u32 = 0;
    /// Returns the [`ActionCategory`] this action is part of.
    pub const category: ActionCategory;

    pub const skill for {
        category = ActionCategory::Weaponskill;
        gcd = ScaleTime::skill(2500);
    }
    pub const ability for {
        category = ActionCategory::Ability;
    }
}]
#[allow(missing_docs)]
pub enum BrdAction {
    #[skill]
    #[name = "Heavy Shot"]
    HeavyShot,
    #[skill]
    #[name = "Straight Shot"]
    StraightShot,
    #[ability]
    #[cooldown = 120000]
    #[name = "Raging Strikes"]
    RagingStrikes,
    #[skill]
    #[name = "Venomous Bite"]
    VenomousBite,
    #[ability]
    #[cooldown = 15000]
    #[cd_charges = 3]
    #[name = "Bloodletter"]
    Bloodletter,
    #[ability]
    #[cooldown = 30000]
    #[name = "RepellingShot"]
    RepellingShot,
    #[skill]
    #[name = "Quick Nock"]
    QuickNock,
    #[skill]
    #[name = "Windbite"]
    Windbite,
    #[ability]
    #[cooldown = 120000]
    #[name = "Barrage"]
    Barrage,
    #[ability]
    #[cooldown = 120000]
    #[name = "Mage's Ballad"]
    MagesBallad,
    #[ability]
    #[cooldown = 45000]
    #[name = "The Warden's Paean"]
    WardensPaean,
    #[ability]
    #[cooldown = 120000]
    #[name = "Army's Paeon"]
    ArmysPaeon,
    #[ability]
    #[cooldown = 15000]
    #[name = "Rain of Death"]
    RainOfDeath,
    #[ability]
    #[cooldown = 120000]
    #[name = "Battle Voice"]
    BattleVoice,
    #[ability]
    #[cooldown = 120000]
    #[name = "The Wanderer's Minuet"]
    WanderersMinuet,
    #[ability]
    #[cooldown = 15000]
    #[name = "Empyreal Arrow"]
    EmpyrealArrow,
    #[skill]
    #[name = "Iron Jaws"]
    IronJaws,
    #[ability]
    #[cooldown = 60000]
    #[name = "Sidewinder"]
    Sidewinder,
    #[ability]
    #[cooldown = 90000]
    #[name = "Troubadour"]
    Troubadour,
    #[skill]
    #[name = "Caustic Bite"]
    CausticBite,
    #[skill]
    #[name = "Stormbite"]
    Stormbite,
    #[ability]
    #[cooldown = 120000]
    #[name = "Nature's Minne"]
    NaturesMinne,
    #[skill]
    #[name = "Refulgent Arrow"]
    RefulgentArrow,
    #[skill]
    #[name = "Shadowbite"]
    Shadowbite,
    #[skill]
    #[name = "Burst Shot"]
    BurstShot,
    #[skill]
    #[name = "Apex Arrow"]
    ApexArrow,
    #[skill]
    #[name = "Ladonsbite"]
    Ladonsbite,
    #[ability]
    #[cooldown = 110000]
    #[name = "Radiant Finale"]
    RadiantFinale,
    #[ability]
    #[cooldown = 1000]
    #[name = "Pitch Perfect"]
    PitchPerfect,
    #[skill]
    #[name = "Blast Arrow"]
    BlastArrow,
}

impl JobAction for BrdAction {
    fn category(&self) -> ActionCategory {
        self.category()
    }
}

impl From<BrdAction> for Action {
    fn from(value: BrdAction) -> Self {
        Action::Job(value.into())
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The state of the Bard job gauges and cooldowns.
pub struct BrdState {
    /// The Song gauge.
    pub song: Option<(BrdSong, u16)>,
    /// The Soul Voice gauge.
    pub soul: GaugeU8<100>,
    /// The Coda gauge.
    pub coda: Coda,
    /// A counter that specifies the "id" of a song instance.
    ///
    /// This is used to discard unapplicable [bard events].
    ///
    /// [bard events]: BrdEvent
    // things may go wrong with more than 2^16 song cast events queued, but it is such
    // a contrived edge case that there is no reason to care.
    pub song_gen: u16,
}

impl JobState for BrdState {
    fn advance(&mut self, time: u32) {
        if let Some((_, song_time)) = &mut self.song {
            *song_time = (*song_time as u32).saturating_sub(time) as u16;
            // don't remove the song, the event handler will do that
            // to make sure army's muse gets applied
        }
    }
}

fn repertoire<'w, W: World>(
    state: &mut BrdState,
    this_id: ActorId,
    event_sink: &mut impl EventSink<'w, W>,
) {
    match &mut state.song {
        Some((song, _)) => {
            state.soul += 5;
            match song {
                // should this be more explicit and manually saturating_sub the value?
                BrdSong::Ballad => {
                    event_sink.event(Event::AdvCd(BrdCdGroup::Bloodletter.into(), this_id), 0)
                }
                BrdSong::Paeon(rep) => *rep += 1,
                BrdSong::Minuet(rep) => *rep += 1,
            };
        }
        _ => (),
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
/// The state of the Bard Song gauge.
pub enum BrdSong {
    /// Mage's Ballad is active.
    Ballad,
    /// Army's Paeon is active.
    Paeon(GaugeU8<4>),
    /// The Wanderer's Minuet is active.
    Minuet(GaugeU8<3>),
}

impl BrdSong {
    /// Returns the status effect associated with the song.
    pub fn song_buff(&self) -> StatusEffect {
        match self {
            Self::Ballad => BALLAD,
            Self::Paeon(..) => PAEON,
            Self::Minuet(..) => MINUET,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
/// The Sen gauge.
pub struct Coda {
    bits: u8,
}
impl Coda {
    const MAGES: u8 = 1 << 0;
    const ARMYS: u8 = 1 << 1;
    const WANDERERS: u8 = 1 << 2;

    /// Grants Mage's Coda.
    pub fn grant_mages(&mut self) {
        self.bits |= Self::MAGES
    }
    /// Grants Army's Coda.
    pub fn grant_armys(&mut self) {
        self.bits |= Self::ARMYS
    }
    /// Grants Wanderer's Coda.
    pub fn grant_wanderers(&mut self) {
        self.bits |= Self::WANDERERS
    }
    /// Returns the number of Coda present.
    pub fn count(&self) -> u8 {
        self.bits.count_ones() as u8
    }
    /// Clears the Coda gauge.
    pub fn clear(&mut self) {
        self.bits = 0;
    }
}

job_cd_struct! {
    BrdAction =>

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Clone, Debug, Default)]
    /// The active cooldowns for Bard actions.
    pub BrdCds

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    /// The various cooldown groups a Bard action can be part of.
    pub BrdCdGroup

    "Raging Strikes"
    raging Raging: RagingStrikes;
    "Bloodletter/Rain of Death"
    bloodletter Bloodletter: Bloodletter RainOfDeath;
    "Repelling Shot"
    repelling Repelling: RepellingShot;
    "Barrage"
    barrage Barrage: Barrage;
    "Mage's Ballad"
    ballad Ballad: MagesBallad;
    "The Warden's Paean"
    paean Paean: WardensPaean;
    "Army's Paeon"
    paeon Paeon: ArmysPaeon;
    "Battle Voice"
    voice Voice: BattleVoice;
    "The Wanderer's Minuet"
    minuet Minuet: WanderersMinuet;
    "Empyreal Arrow"
    empyreal Empyreal: EmpyrealArrow;
    "Sidewinder"
    sidewinder Sidewinder: Sidewinder;
    "Troubadour"
    troubadour Troubadour: Troubadour;
    "Nature's Minne"
    minne Minne: NaturesMinne;
    "Radiant Finale"
    finale Finale: RadiantFinale;
    "Pitch Perfect"
    pitch Pitch: PitchPerfect;
    "Bloodletter/Rain of Death Charge"
    bloodletter_chg BloodLetterChg;
}

impl BrdAction {
    /// Returns the alternate [cooldown group] that this action is part of.
    ///
    /// Returns `None` if this action does not have an alternate cooldown.
    /// This action is used for the 1s cooldown between uses of charged actions.
    ///
    /// [cooldown group]: BrdCdGroup
    pub const fn alt_cd_group(&self) -> Option<BrdCdGroup> {
        match self {
            Self::Bloodletter | Self::RainOfDeath => Some(BrdCdGroup::BloodLetterChg),
            _ => None,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug)]
/// A custom event for a song repertoire proc.
pub enum BrdEvent {
    /// A song tick will happen.
    SongTick {
        /// The generation of the song.
        ///
        /// See [`BrdState::song_gen`] for more information.
        gen: u16,
    },
}

impl BrdEvent {
    /// Creates a new [`SongTick`] event.
    ///
    /// [`SongTick`]: BrdEvent::SongTick
    pub const fn song_tick(gen: u16) -> Self {
        Self::SongTick { gen }
    }
}

bool_job_dist! {
    /// The random event for a Straight Shot proc.
    pub StraightShotProc = 35 / 100;
    /// The random event for a Straight Shot proc.
    pub ShadowbiteProc = 35 / 100;
    /// The random event for a Repertoire proc.
    pub RepertoireProc = 8 / 10;
}
