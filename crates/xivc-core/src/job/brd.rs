use core::{
    error::Error,
    fmt::{self, Display},
};

use macros::var_consts;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    bool_job_dist,
    enums::{ActionCategory, DamageInstance},
    job::{CastInitInfo, Job, JobAction, JobEvent, JobState},
    job_cd_struct, job_effect_wrapper,
    math::SpeedStat,
    status_effect,
    timing::{DurationInfo, EventCascade, ScaleTime},
    util::{status_proc_error, ActionTargettingExt as _, GaugeU8},
    world::{
        status::{consume_status, JobEffect, StatusEffect, StatusEventExt},
        Action, ActionTargetting, ActorId, ActorRef, DamageEventExt, Event, EventError, EventSink,
        Faction, WorldRef,
    },
};

/// The [`Job`] struct for Bard.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrdJob;

/// The status effect "Straight Shot Ready".
pub const HAWKS_EYE: StatusEffect = status_effect!("Hawk's Eye" 30000);
/// The status effect "Blast Arrow Ready".
pub const BLAST_ARROW: StatusEffect = status_effect!("Blast Arrow Ready" 10000);
/// The status effect "Raging Strikes".
pub const RAGING_STRIKES: StatusEffect = status_effect!(
    "Raging Strikes" 20000 { damage { out = 115 / 100 } }
);
/// The status effect "Barrage".
pub const BARRAGE: StatusEffect = status_effect!("Barrage" 10000);
/// The status effect "Resonant Arrow Ready"
pub const RESONANT_ARROW: StatusEffect = status_effect!("Resonant Arrow Ready" 30000);
/// The status effect "Radiant Encore Ready"
pub const RADIANT_ENCORE: StatusEffect = status_effect!("Radiant Encore Ready" 30000);
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
pub const CAUSTIC_BITE: StatusEffect = status_effect!("Caustic Bite" 45000 multi);
/// The DoT effect "Stormbite".
pub const STORMBITE: StatusEffect = status_effect!("Stormbite" 45000 multi);
// TODO: minne and paean
// this can be done if/when healing is implemented

const SONG_LEN: u16 = 45000;

impl Job for BrdJob {
    type Action = BrdAction;
    type State = BrdState;
    type CastError = BrdError;
    type Event = BrdEvent;
    type CdGroup = BrdCdGroup;
    type CdMap<T> = BrdCdMap<T>;

    fn check_cast<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
        action: Self::Action,
        state: &Self::State,
        _: &'w W,
        event_sink: &mut E,
    ) -> Result<CastInitInfo<Self::CdGroup>, EventError> {
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
            ApexArrow if state.soul < 20 => return Err(BrdError::SoulVoice(20).into()),
            StraightShot | RefulgentArrow | Shadowbite
                if !this.has_own_status(HAWKS_EYE) || !this.has_own_status(BARRAGE) =>
            {
                return Err(BrdError::HawksEye.into());
            }
            PitchPerfect => match &state.song {
                Some((BrdSong::Minuet(x), _)) if *x > 0 => (),
                _ => return Err(BrdError::PitchPerfect.into()),
            },
            BlastArrow if !this.has_own_status(BLAST_ARROW) => {
                return Err(BrdError::BlastArrow.into());
            }
            RadiantFinale if state.coda.count() == 0 => return Err(BrdError::Coda.into()),
            ResonantArrow if !this.has_own_status(RESONANT_ARROW) => {
                return Err(BrdError::ResonantArrow.into());
            }
            RadiantEncore if !this.has_own_status(RADIANT_ENCORE) => {
                return Err(BrdError::RadiantEncore.into());
            }
            _ => (),
        }

        Ok(CastInitInfo {
            gcd,
            lock,
            snap,
            mp: 0,
            cd,
            alt_cd,
        })
    }

    fn cast_snap<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
        action: Self::Action,
        state: &mut Self::State,
        _: &'w W,
        event_sink: &mut E,
    ) -> Result<(), EventError> {
        let this = event_sink.source();
        let this_id = this.id();

        use BrdAction::*;

        let dl = action.effect_delay();

        match action {
            HeavyShot | BurstShot => {
                let t = this.target_enemy(RANGED)?.id();
                if event_sink.random(HawksEyeProc) {
                    event_sink.apply_status(HAWKS_EYE, 1, this_id, 0);
                }
                event_sink.damage(action, DamageInstance::new(220).piercing(), t, dl);
            }
            StraightShot | RefulgentArrow => {
                let t = this.target_enemy(RANGED)?.id();

                if consume_status(event_sink, BARRAGE, 0) {
                    let mut cascade = EventCascade::new(dl, 1);
                    event_sink.damage(
                        action,
                        DamageInstance::new(280).piercing(),
                        t,
                        cascade.next(),
                    );
                    event_sink.damage(
                        action,
                        DamageInstance::new(280).piercing(),
                        t,
                        cascade.next(),
                    );
                    event_sink.damage(
                        action,
                        DamageInstance::new(280).piercing(),
                        t,
                        cascade.next(),
                    );
                } else if consume_status(event_sink, HAWKS_EYE, 0) {
                    event_sink.damage(action, DamageInstance::new(280).piercing(), t, dl);
                } else {
                    return Err(BrdError::HawksEye.into());
                }
            }
            RagingStrikes => {
                event_sink.apply_status(RAGING_STRIKES, 1, this_id, dl);
            }
            VenomousBite | CausticBite => {
                let t = this.target_enemy(RANGED)?.id();
                if event_sink.random(HawksEyeProc) {
                    event_sink.apply_status(HAWKS_EYE, 1, this_id, 0);
                }
                event_sink.apply_dot(
                    CAUSTIC_BITE,
                    DamageInstance::new(20).piercing(),
                    SpeedStat::SkillSpeed,
                    1,
                    t,
                    dl,
                );
                event_sink.damage(action, DamageInstance::new(150).piercing(), t, dl);
            }
            Bloodletter | HeartbreakShot => {
                let t = this.target_enemy(RANGED)?.id();
                event_sink.damage(action, DamageInstance::new(110).piercing(), t, dl);
            }
            QuickNock | Ladonsbite => {
                let iter = this
                    .target_enemy_aoe(ActionTargetting::cone(12, 90), EventCascade::new(dl, 1))?
                    .id();
                if event_sink.random(HawksEyeProc) {
                    event_sink.apply_status(HAWKS_EYE, 1, this_id, 0);
                }
                for (t, d) in iter {
                    event_sink.damage(action, DamageInstance::new(130).piercing(), t, d);
                }
            }
            Windbite | Stormbite => {
                let t = this.target_enemy(RANGED)?.id();
                if event_sink.random(HawksEyeProc) {
                    event_sink.apply_status(HAWKS_EYE, 1, this_id, 0);
                }
                event_sink.apply_dot(
                    STORMBITE,
                    DamageInstance::new(25).piercing(),
                    SpeedStat::SkillSpeed,
                    1,
                    t,
                    dl,
                );
                event_sink.damage(action, DamageInstance::new(100).piercing(), t, dl);
            }
            Barrage => {
                event_sink.apply_status(BARRAGE, 1, this_id, 0);
                event_sink.apply_status(RESONANT_ARROW, 1, this_id, 0);
            }
            MagesBallad => {
                if !this.in_combat() {
                    return Err(EventError::InCombat);
                }
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
            }
            ArmysPaeon => {
                if !this.in_combat() {
                    return Err(EventError::InCombat);
                }
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
            }
            RainOfDeath => {
                let iter = this
                    .target_enemy_aoe(
                        ActionTargetting::target_circle(8, 25),
                        EventCascade::new(dl, 1),
                    )?
                    .id();
                for (t, d) in iter {
                    event_sink.damage(action, DamageInstance::new(100).piercing(), t, d);
                }
            }
            BattleVoice => {
                let iter = this
                    .target_party_aoe(ActionTargetting::circle(30), EventCascade::new(dl, 3))?
                    .id();
                for (t, d) in iter {
                    event_sink.apply_status(BATTLE_VOICE, 1, t, d);
                }
            }
            WanderersMinuet => {
                if !this.in_combat() {
                    return Err(EventError::InCombat);
                }
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
            }
            EmpyrealArrow => {
                let t = this.target_enemy(RANGED)?.id();
                repertoire(state, this_id, event_sink);
                event_sink.damage(action, DamageInstance::new(240).piercing(), t, dl);
            }
            IronJaws => {
                let target_actor = this.target_enemy(RANGED)?;
                let t = target_actor.id();

                if event_sink.random(HawksEyeProc) {
                    event_sink.apply_status(HAWKS_EYE, 1, this_id, 0);
                }
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
                event_sink.damage(action, DamageInstance::new(100).piercing(), t, dl);
            }
            Sidewinder => {
                let t = this.target_enemy(RANGED)?.id();
                event_sink.damage(action, DamageInstance::new(320).piercing(), t, dl);
            }
            Shadowbite => {
                let iter = this.target_enemy_aoe(TC5Y, EventCascade::new(dl, 1))?.id();
                let barrage = consume_status(event_sink, BARRAGE, 0);
                if !barrage {
                    event_sink.remove_status(HAWKS_EYE, this_id, 0);
                }
                let potency = if barrage { 270 } else { 170 };
                for (t, d) in iter {
                    event_sink.damage(action, DamageInstance::new(potency).piercing(), t, d);
                }
            }
            ApexArrow => {
                let iter = this.target_enemy_aoe(LINE, EventCascade::new(dl, 1))?.id();
                if state.soul < 20 {
                    return Err(BrdError::SoulVoice(20).into());
                }
                let potency = *state.soul as u64 * 6;
                if state.soul >= 80 {
                    event_sink.apply_status(BLAST_ARROW, 1, this_id, 0);
                }
                state.soul.clear();
                for (t, d) in iter {
                    event_sink.damage(action, DamageInstance::new(potency).piercing(), t, d);
                }
            }
            RadiantFinale => {
                let iter = this
                    .target_party_aoe(ActionTargetting::circle(30), EventCascade::new(dl, 3))?
                    .id();
                let stacks = state.coda.count();
                event_sink.apply_status(RADIANT_ENCORE, stacks, this_id, 0);
                state.coda.clear();
                for (t, d) in iter {
                    event_sink.apply_status(RADIANT_FINALE, stacks, t, d);
                }
            }
            PitchPerfect => {
                let iter = this
                    .target_enemy_aoe(TC5Y, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                let rep_stacks = match &mut state.song {
                    Some((BrdSong::Minuet(rep), _)) => {
                        let out = **rep;
                        rep.clear();
                        out
                    }
                    _ => return Err(BrdError::PitchPerfect.into()),
                };
                let potency = match rep_stacks {
                    1 => 100,
                    2 => 220,
                    3 => 360,
                    _ => return Err(BrdError::PitchPerfect.into()),
                };
                for (t, d, f) in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(potency).piercing().falloff(f),
                        t,
                        d,
                    );
                }
            }
            BlastArrow => {
                let iter = this
                    .target_enemy_aoe(LINE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(40);
                if !consume_status(event_sink, BLAST_ARROW, 0) {
                    return Err(BrdError::BlastArrow.into());
                }
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(600).piercing().falloff(f), t, d);
                }
            }
            RepellingShot | WardensPaean | NaturesMinne | Troubadour => {
                // unimplemented
            }
            ResonantArrow => {
                let iter = this
                    .target_enemy_aoe(TC5Y, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                if !consume_status(event_sink, RESONANT_ARROW, 0) {
                    return Err(BrdError::ResonantArrow.into());
                }
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(600).piercing().falloff(f), t, d)
                }
            }
            RadiantEncore => {
                let iter = this
                    .target_enemy_aoe(TC5Y, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                let potency = if let Some(s) = this.get_own_status(RADIANT_ENCORE) {
                    match s.stack {
                        1 => 500,
                        2 => 600,
                        3 => 900,
                        _ => 0,
                    }
                } else {
                    return Err(BrdError::RadiantEncore.into());
                };
                event_sink.remove_status(RADIANT_ENCORE, this_id, 0);
                for (t, d, f) in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(potency).piercing().falloff(f),
                        t,
                        d,
                    )
                }
            }
        }

        Ok(())
    }

    fn event<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
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
                                                BrdEvent::song_tick(state.song_gen),
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

    fn effect(state: &Self::State) -> Option<&dyn JobEffect> {
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug)]
/// A custom cast error for Bard actions.
pub enum BrdError {
    /// Not enough Soul Voice gauge.
    SoulVoice(u8),
    /// Not under the effect of Straight Shot Ready.
    HawksEye,
    /// Not enough stacks of Repertoire to cast Pitch Perfect.
    PitchPerfect,
    /// Not under the effect of Blast Arrow Ready.
    BlastArrow,
    /// Not enough Coda.
    Coda,
    /// Not under the effect of Resonant Arrow Ready.
    ResonantArrow,
    /// Not under the effect of Radiant Encore Ready.
    RadiantEncore,
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
            Self::HawksEye => status_proc_error(f, HAWKS_EYE),
            Self::PitchPerfect => write!(
                f,
                "Not enough stacks of Repertoire to cast 'Pitch Perfect'."
            ),
            Self::BlastArrow => status_proc_error(f, BLAST_ARROW),
            Self::Coda => write!(f, "Not enough Coda."),
            Self::ResonantArrow => status_proc_error(f, RESONANT_ARROW),
            Self::RadiantEncore => status_proc_error(f, RADIANT_ENCORE),
        }
    }
}

impl Error for BrdError {}

const RANGED: ActionTargetting = ActionTargetting::single(25);
const TC5Y: ActionTargetting = ActionTargetting::target_circle(5, 25);
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
    #[ability]
    #[name = "Heartbreak Shot"]
    HeartbreakShot,
    #[skill]
    #[name = "Resonant Arrow"]
    ResonantArrow,
    #[skill]
    #[name = "Radiant Encore"]
    RadiantEncore,
}

impl JobAction for BrdAction {
    fn category(&self) -> ActionCategory {
        self.category()
    }

    fn gcd(&self) -> bool {
        self.gcd().is_some()
    }
}

impl From<BrdAction> for Action {
    fn from(value: BrdAction) -> Self {
        Action::Job(value.into())
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

fn repertoire<'w, W: WorldRef<'w>>(
    state: &mut BrdState,
    this_id: ActorId,
    event_sink: &mut impl EventSink<'w, W>,
) {
    if let Some((song, _)) = &mut state.song {
        state.soul += 5;
        match song {
            // should this be more explicit and manually saturating_sub the value?
            BrdSong::Ballad => event_sink.event(
                Event::AdvCd(BrdCdGroup::Bloodletter.into(), 7500, this_id),
                0,
            ),
            BrdSong::Paeon(rep) => *rep += 1,
            BrdSong::Minuet(rep) => *rep += 1,
        };
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// The Coda gauge.
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
    /// The cooldown map for Bard actions.
    pub BrdCdMap

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    /// The various cooldown groups a Bard action can be part of.
    pub BrdCdGroup

    "Raging Strikes"
    raging Raging: RagingStrikes;
    "Bloodletter/Heartbreak Shot/Rain of Death"
    bloodletter Bloodletter: Bloodletter RainOfDeath HeartbreakShot;
    // "Repelling Shot"
    // repelling Repelling: RepellingShot;
    "Barrage"
    barrage Barrage: Barrage;
    "Mage's Ballad"
    ballad Ballad: MagesBallad;
    // "The Warden's Paean"
    // paean Paean: WardensPaean;
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
    // "Troubadour"
    // troubadour Troubadour: Troubadour;
    // "Nature's Minne"
    // minne Minne: NaturesMinne;
    "Radiant Finale"
    finale Finale: RadiantFinale;
    "Pitch Perfect"
    pitch Pitch: PitchPerfect;
    "Bloodletter/Heartbreak Shot/Rain of Death Charge"
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
            Self::Bloodletter | Self::RainOfDeath | Self::HeartbreakShot => {
                Some(BrdCdGroup::BloodLetterChg)
            }
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
    pub HawksEyeProc = 35 / 100;
    /// The random event for a Repertoire proc.
    pub RepertoireProc = 8 / 10;
}
