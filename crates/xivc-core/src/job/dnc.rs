use core::fmt::{self, Display};

use macros::var_consts;

use rand::{distributions::Distribution, seq::SliceRandom, Rng};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    bool_job_dist,
    enums::{ActionCategory, DamageInstance},
    err,
    job::{CastInitInfo, Job, JobAction, JobState},
    job_cd_struct, status_effect,
    timing::{DurationInfo, EventCascade, ScaleTime},
    util::{combo_pot, ActionTargettingExt as _, ComboState, GaugeU8},
    world::{
        status::{consume_status, StatusEffect, StatusEventExt, StatusEventKind},
        Action, ActionTargetting, ActorId, ActorRef, DamageEventExt, Event, EventError, EventSink,
        WorldRef,
    },
};

/// The [`Job`] struct for Dancer.
#[derive(Clone, Copy, Debug, Default)]
pub struct DncJob;

// TODO: Status Effect Definitions
/// The status effect "Silken Symmetry".
pub static SILKEN_SYMM: StatusEffect = status_effect!("Silken Symmetry" 30000);
/// The status effect "Silken Flow".
pub static SILKEN_FLOW: StatusEffect = status_effect!("Silken Flow" 30000);
/// The status effect "Standard Step".
pub static STANDARD_STEP: StatusEffect = status_effect!("Standard Step" 15000);
/// The status effect "Standard Finish".
pub static STANDARD_FINISH: StatusEffect = status_effect!(
    "Standard Finish" 60000 { damage { out = |s, d, _, _| {
        d * match s.stack {
            1 => 102,
            2 => 105,
            _ => 0,
        } / 100
    } } }
);
/// The status effect "Esprit" from Standard Finish.
pub static STANDARD_ESPIT: StatusEffect = status_effect!("Esprit" 60000);
/// The status effect "Closed Position".
pub static CLOSED_POSITION: StatusEffect = status_effect!("Closed Position" permanent);
/// The status effect "Dance Partner".
pub static DANCE_PARTNER: StatusEffect = status_effect!("Dance Partner" permanent);
/// The status effect "Devilment".
pub static DEVILMENT: StatusEffect = status_effect!(
    "Devilment" 20000 { crit { out = 200 } dhit { out = 200 } }
);
/// The status effect "Flourishing Starfall".
pub static STARFALL: StatusEffect = status_effect!("Flourishing Starfall" 20000);
/// The status effect "Threefold Fan Dance".
pub static FAN_DANCE_3: StatusEffect = status_effect!("Threefold Fan Dance" 30000);
/// The status effect "Technical Step".
pub static TECHNICAL_STEP: StatusEffect = status_effect!("Technical Step" 15000);
/// The status effect "Technical Finish".
pub static TECHNICAL_FINISH: StatusEffect = status_effect!(
    "Technical Finish" 20000 { damage { out = |s, d, _, _| {
        d * match s.stack {
            1 => 101,
            2 => 102,
            3 => 103,
            4 => 105,
            _ => 0,
        } / 100
    } } }
);
/// The status effect "Esprit" from Technical Finish.
pub static TECHNICAL_ESPIT: StatusEffect = status_effect!("Esprit" 20000);
/// The status effect "Flourishing Finish".
pub static FLOURISH_FINISH: StatusEffect = status_effect!("Flourishing Finish" 30000);
/// The status effect "Flourishing Symmetry".
pub static FLOURISH_SYMM: StatusEffect = status_effect!("Flourishing Symmetry" 30000);
/// The status effect "Flourishing Flow".
pub static FLOURISH_FLOW: StatusEffect = status_effect!("Flourishing Flow" 30000);
/// The status effect "Fourfold Fan Dance".
pub static FAN_DANCE_4: StatusEffect = status_effect!("Fourfold Fan Dance" 30000);
/// The status effect "Last Dance Ready".
pub static LAST_DANCE_READY: StatusEffect = status_effect!("Last Dance Ready" 30000);
/// The status effect "Finishing Move Ready".
pub static FINISHING_MOVE_READY: StatusEffect = status_effect!("Finishing Move Ready" 30000);
/// The status effect "Dance of the Dawn Ready".
pub static DANCE_OF_THE_DAWN_READY: StatusEffect = status_effect!("Dance of the Dawn Ready" 30000);

impl Job for DncJob {
    type Action = DncAction;
    type State = DncState;
    type CastError = DncError;
    type Event = ();
    type CdGroup = DncCdGroup;
    type CdMap<T> = DncCdMap<T>;

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

        use DncAction::*;

        if !action.step_valid()
            && (this.has_own_status(STANDARD_STEP) || this.has_own_status(TECHNICAL_STEP))
        {
            return Err(DncError::StepInvalid.into());
        }

        match action {
            FanDance | FanDance2 if *state.feathers == 0 => {
                err!(DncError::Feather);
            }
            SaberDance | DanceOfTheDawn if *state.esprit < 50 => {
                err!(DncError::Esprit);
            }
            FanDance3 if !this.has_own_status(FAN_DANCE_3) => {
                err!(DncError::Fan3);
            }
            FanDance4 if !this.has_own_status(FAN_DANCE_4) => {
                err!(DncError::Fan4);
            }
            StarfallDance if !this.has_own_status(STARFALL) => {
                err!(DncError::Starfall);
            }
            ReverseCascade | RisingWindmill => {
                if !this.has_own_status(SILKEN_SYMM) && !this.has_own_status(FLOURISH_SYMM) {
                    err!(DncError::Symmetry);
                }
            }
            Fountainfall | Bloodshower => {
                if !this.has_own_status(SILKEN_FLOW) && !this.has_own_status(FLOURISH_FLOW) {
                    err!(DncError::Flow);
                }
            }
            Tillana if !this.has_own_status(FLOURISH_FINISH) => {
                err!(DncError::Tillana);
            }
            StandardFinish if !this.has_own_status(STANDARD_STEP) => {
                err!(DncError::StandardStep);
            }
            TechnicalFinish if !this.has_own_status(TECHNICAL_STEP) => {
                err!(DncError::TechnicalStep);
            }
            Jete | Pirouette | Emboite | Entrechat => {
                if !this.has_own_status(STANDARD_STEP) && !this.has_own_status(TECHNICAL_STEP) {
                    err!(DncError::Step);
                }
            }
            Flourish if !this.in_combat() => {
                err!(EventError::InCombat);
            }
            ClosedPosition if state.partner.is_some() => {
                err!(DncError::PartnerActive);
            }
            Ending if state.partner.is_none() => {
                err!(DncError::PartnerInactive);
            }
            LastDance if !this.has_own_status(LAST_DANCE_READY) => {
                err!(DncError::DanceOfTheDawn);
            }
            _ => (),
        }

        Ok(CastInitInfo {
            gcd,
            lock,
            snap,
            mp: 0,
            cd,
            alt_cd: None,
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

        use DncAction::*;

        let esprit = |state: &mut DncState, val: u8| {
            if this.has_own_status(STANDARD_ESPIT) || this.has_own_status(TECHNICAL_ESPIT) {
                state.esprit += val;
            }
        };

        let dl = action.effect_delay();

        match action {
            Cascade => {
                let t = this.target_enemy(RANGED)?.id();
                if event_sink.random(SymmFlowProc) {
                    event_sink.apply_status(SILKEN_SYMM, 1, this_id, 0);
                }
                esprit(state, 5);
                state.combos.main.set(MainCombo::Cascade);
                event_sink.damage(action, DamageInstance::new(220).slashing(), t, dl);
            }
            Fountain => {
                let t = this.target_enemy(RANGED)?.id();
                let combo = state.combos.check_main_for(action);
                if combo && event_sink.random(SymmFlowProc) {
                    event_sink.apply_status(SILKEN_FLOW, 1, this_id, 0);
                }
                // apparently you get esprit from uncomboed gcds
                esprit(state, 5);
                state.combos.main.reset();
                event_sink.damage(
                    action,
                    DamageInstance::new(combo_pot(100, 280, combo)).slashing(),
                    t,
                    dl,
                );
            }
            Windmill => {
                let iter = this
                    .target_enemy_aoe(CIRCLE, EventCascade::new(dl, 1))?
                    .id();
                let mut hit = false;
                for (t, d) in iter {
                    hit = true;
                    event_sink.damage(action, DamageInstance::new(100).slashing(), t, d);
                }
                if hit {
                    if event_sink.random(SymmFlowProc) {
                        event_sink.apply_status(SILKEN_SYMM, 1, this_id, 0);
                    }
                    esprit(state, 5);
                    state.combos.main.set(MainCombo::Windmill);
                } else {
                    state.combos.main.reset();
                }
            }
            StandardStep => {
                event_sink.apply_status(STANDARD_STEP, 1, this_id, 0);
                state.step = StepGauge::Std {
                    steps: event_sink.random(StdStepSeqence),
                    completed: 0,
                }
            }
            ReverseCascade => {
                let t = this.target_enemy(RANGED)?.id();
                if !consume_status(event_sink, SILKEN_SYMM, 0)
                    && !consume_status(event_sink, FLOURISH_SYMM, 0)
                {
                    err!(DncError::Symmetry);
                }
                event_sink.damage(action, DamageInstance::new(280).slashing(), t, dl);
                if event_sink.random(FeatherProc) {
                    state.feathers += 1;
                }
                esprit(state, 10);
            }
            Bladeshower => {
                let iter = this
                    .target_enemy_aoe(CIRCLE, EventCascade::new(dl, 1))?
                    .id();
                let combo = state.combos.check_main_for(action);
                let mut hit = false;
                for (t, d) in iter {
                    hit = true;
                    event_sink.damage(
                        action,
                        DamageInstance::new(combo_pot(100, 140, combo)).slashing(),
                        t,
                        d,
                    );
                }
                if hit {
                    if combo && event_sink.random(SymmFlowProc) {
                        event_sink.apply_status(SILKEN_SYMM, 1, this_id, 0);
                    }
                    esprit(state, 5);
                }
                state.combos.main.reset();
            }
            FanDance => {
                let t = this.target_enemy(RANGED)?.id();
                if !state.feathers.consume(1) {
                    err!(DncError::Feather);
                }
                if event_sink.random(FanDance3Proc) {
                    event_sink.apply_status(FAN_DANCE_3, 1, this_id, 0);
                }
                event_sink.damage(action, DamageInstance::new(150).slashing(), t, dl);
            }
            RisingWindmill => {
                let iter = this
                    .target_enemy_aoe(CIRCLE, EventCascade::new(dl, 1))?
                    .id();
                if !consume_status(event_sink, SILKEN_SYMM, 0)
                    && !consume_status(event_sink, FLOURISH_SYMM, 0)
                {
                    err!(DncError::Symmetry);
                }
                let mut hit = false;
                for (t, d) in iter {
                    hit = true;
                    event_sink.damage(action, DamageInstance::new(140).slashing(), t, d);
                }
                if hit {
                    if event_sink.random(FeatherProc) {
                        state.feathers += 1;
                    }
                    esprit(state, 10);
                }
            }
            Fountainfall => {
                let t = this.target_enemy(RANGED)?.id();
                if !consume_status(event_sink, SILKEN_FLOW, 0)
                    && !consume_status(event_sink, FLOURISH_FLOW, 0)
                {
                    err!(DncError::Flow);
                }
                event_sink.damage(action, DamageInstance::new(340).slashing(), t, dl);
                if event_sink.random(FeatherProc) {
                    state.feathers += 1;
                }
                esprit(state, 10);
            }
            Bloodshower => {
                let iter = this
                    .target_enemy_aoe(CIRCLE, EventCascade::new(dl, 1))?
                    .id();
                if !consume_status(event_sink, SILKEN_FLOW, 0)
                    && !consume_status(event_sink, FLOURISH_FLOW, 0)
                {
                    err!(DncError::Flow);
                }
                let mut hit = false;
                for (t, d) in iter {
                    hit = true;
                    event_sink.damage(action, DamageInstance::new(180).slashing(), t, d);
                }
                if hit {
                    if event_sink.random(FeatherProc) {
                        state.feathers += 1;
                    }
                    esprit(state, 10);
                }
            }
            FanDance2 => {
                let iter = this
                    .target_enemy_aoe(CIRCLE, EventCascade::new(dl, 1))?
                    .id();
                if !state.feathers.consume(1) {
                    err!(DncError::Feather);
                }
                let mut hit = false;
                for (t, d) in iter {
                    hit = true;
                    event_sink.damage(action, DamageInstance::new(100).slashing(), t, d);
                }
                if hit && event_sink.random(FanDance3Proc) {
                    event_sink.apply_status(FAN_DANCE_3, 1, this_id, 0);
                }
            }
            ClosedPosition => {
                let t = this.target_party(ActionTargetting::single(30))?.id();
                if t == this_id {
                    err!(EventError::NoTarget);
                }

                // remove partner. this is to maintain a consistent state
                // if the action is executed despite an error being present.
                if let Some(partner) = state.partner {
                    event_sink.remove_status(DANCE_PARTNER, partner, 0);
                    event_sink.remove_status(CLOSED_POSITION, this_id, 0);
                }
                event_sink.apply_status(DANCE_PARTNER, 1, t, dl);
                event_sink.apply_status(CLOSED_POSITION, 1, this_id, dl);
                state.partner = Some(t);
            }
            Ending => {
                if let Some(partner) = state.partner {
                    event_sink.remove_status(DANCE_PARTNER, partner, 0);
                    event_sink.remove_status(CLOSED_POSITION, this_id, 0);
                    state.partner = None;
                }
            }
            Devilment => {
                event_sink.apply_status(DEVILMENT, 1, this_id, 0);
                event_sink.apply_status(STARFALL, 1, this_id, 0);
                if let Some(partner) = state.partner {
                    event_sink.apply_status(DEVILMENT, 1, partner, 0);
                }
            }
            FanDance3 => {
                let iter = this
                    .target_enemy_aoe(TG_CIRCLE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                if !consume_status(event_sink, FAN_DANCE_3, 0) {
                    err!(DncError::Fan3);
                }
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(100).slashing().falloff(f), t, d);
                }
            }
            TechnicalStep => {
                event_sink.apply_status(TECHNICAL_STEP, 1, this_id, 0);
                state.step = StepGauge::Tech {
                    steps: event_sink.random(TechStepSeqence),
                    completed: 0,
                }
            }
            Flourish => {
                event_sink.apply_status(FLOURISH_SYMM, 1, this_id, 0);
                event_sink.apply_status(FLOURISH_FLOW, 1, this_id, 0);
                event_sink.apply_status(FAN_DANCE_3, 1, this_id, 0);
                event_sink.apply_status(FAN_DANCE_4, 1, this_id, 0);
                event_sink.apply_status(FINISHING_MOVE_READY, 1, this_id, 0);
            }
            SaberDance => {
                let iter = this
                    .target_enemy_aoe(TG_CIRCLE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                if !state.esprit.consume(50) {
                    err!(DncError::Esprit);
                }
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(520).slashing().falloff(f), t, d);
                }
            }
            FanDance4 => {
                let iter = this
                    .target_enemy_aoe(ActionTargetting::cone(15, 90), EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                if !consume_status(event_sink, FAN_DANCE_4, 0) {
                    err!(DncError::Fan4);
                }
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(300).slashing().falloff(f), t, d);
                }
            }
            StarfallDance => {
                let iter = this
                    .target_enemy_aoe(ActionTargetting::line(25), EventCascade::new(dl, 1))?
                    .id()
                    .falloff(25);
                if !consume_status(event_sink, STARFALL, 0) {
                    err!(DncError::Starfall);
                }
                for (t, d, f) in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(600)
                            .slashing()
                            .force_crit()
                            .force_dhit()
                            .falloff(f),
                        t,
                        d,
                    );
                }
            }
            Emboite => state.step.execute(Step::Emboite),
            Entrechat => state.step.execute(Step::Entrechat),
            Jete => state.step.execute(Step::Jete),
            Pirouette => state.step.execute(Step::Pirouette),
            StandardFinish => {
                let completed = match state.step {
                    StepGauge::Std { completed, .. } => completed,
                    _ => err!(DncError::StandardStep),
                };
                event_sink.remove_status(STANDARD_STEP, this_id, 0);
                let potency = match completed {
                    0 => 360,
                    1 => 540,
                    2 => 850,
                    _ => 0,
                };

                event_sink.apply_status(LAST_DANCE_READY, 1, this_id, 0);
                // this might?? potentially work the same way as
                // technical step, but i'm not sure and in reality
                // it should not be encountered.
                if completed > 0 {
                    event_sink.apply_status(STANDARD_FINISH, completed, this_id, 0);
                    event_sink.apply_status(STANDARD_ESPIT, 1, this_id, 0);
                    if let Some(partner) = state.partner {
                        event_sink.apply_status(STANDARD_FINISH, completed, partner, 0);
                        event_sink.apply_status(STANDARD_ESPIT, 1, partner, 0);
                    }
                }
                let iter = this
                    .target_enemy_aoe(DANCE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(25);
                for (t, d, f) in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(potency).slashing().falloff(f),
                        t,
                        d,
                    );
                }
            }
            // this is the worst thing i've ever written
            TechnicalFinish => {
                let completed = match state.step {
                    StepGauge::Tech { completed, .. } => completed,
                    _ => err!(DncError::TechnicalStep),
                };
                event_sink.remove_status(TECHNICAL_STEP, this_id, 0);
                let potency = match completed {
                    1 => 540,
                    2 => 720,
                    3 => 900,
                    4 => 1300,
                    _ => 360, // game says 350 but i think thats a typo
                };
                let iter = this
                    .target_enemy_aoe(DANCE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(25);
                for (t, d, f) in iter {
                    event_sink.damage(
                        action,
                        DamageInstance::new(potency).slashing().falloff(f),
                        t,
                        d,
                    );
                }

                event_sink.apply_status(FLOURISH_FINISH, 1, this_id, 0);
                event_sink.apply_status(DANCE_OF_THE_DAWN_READY, 1, this_id, 0);

                if completed > 0 {
                    // TODO: Verify technical step buff delay
                    let delay = 650;
                    let iter = this.target_party_aoe(
                        ActionTargetting::circle(30),
                        EventCascade::new(delay, 3),
                    )?;
                    for (t, d) in iter {
                        let t_id = t.id();
                        event_sink.apply_status_cascade_remove(
                            TECHNICAL_FINISH,
                            completed,
                            t_id,
                            delay,
                            d,
                        );
                        if !t.has_status(STANDARD_ESPIT, this_id) {
                            event_sink.apply_status_cascade_remove(
                                TECHNICAL_ESPIT,
                                1,
                                t_id,
                                delay,
                                d,
                            );
                        }
                    }
                }
                event_sink.apply_status(FLOURISH_FINISH, 1, this_id, 0);
            }
            Tillana => {
                let iter = this
                    .target_enemy_aoe(DANCE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                let mut hit = false;
                for (t, d, f) in iter {
                    hit = true;
                    event_sink.damage(action, DamageInstance::new(600).slashing().falloff(f), t, d);
                }
                // TODO: Verify this logic is correct.
                if hit && this.has_own_status(STANDARD_ESPIT) {
                    state.esprit += 50;
                }
            }
            LastDance => {
                let iter = this
                    .target_enemy_aoe(TG_CIRCLE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                if !consume_status(event_sink, LAST_DANCE_READY, 0) {
                    err!(DncError::LastDance);
                }
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(520).slashing().falloff(f), t, d);
                }
            }
            FinishingMove => {
                if !consume_status(event_sink, FINISHING_MOVE_READY, 0) {
                    err!(DncError::FinishingMove);
                }
                event_sink.apply_status(LAST_DANCE_READY, 1, this_id, 0);
                event_sink.apply_status(STANDARD_FINISH, 2, this_id, 0);
                event_sink.apply_status(STANDARD_ESPIT, 1, this_id, 0);
                if let Some(partner) = state.partner {
                    event_sink.apply_status(STANDARD_FINISH, 2, partner, 0);
                    event_sink.apply_status(STANDARD_ESPIT, 1, partner, 0);
                }
                let iter = this
                    .target_enemy_aoe(DANCE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(25);
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(850).slashing().falloff(f), t, d);
                }
            }
            DanceOfTheDawn => {
                let iter = this
                    .target_enemy_aoe(TG_CIRCLE, EventCascade::new(dl, 1))?
                    .id()
                    .falloff(50);
                if state.esprit < 50 {
                    err!(DncError::Esprit);
                } else if !this.has_own_status(DANCE_OF_THE_DAWN_READY) {
                    err!(DncError::DanceOfTheDawn);
                }
                state.esprit -= 50;
                event_sink.remove_status(DANCE_OF_THE_DAWN_READY, this_id, 0);
                for (t, d, f) in iter {
                    event_sink.damage(action, DamageInstance::new(520).slashing().falloff(f), t, d);
                }
            }
            EnAvant | CuringWaltz | ShieldSamba | Improvisation | ImprovisedFinish => {
                // todo: implement healing/utility skills
            }
        }

        Ok(())
    }

    fn event<'w, W: WorldRef<'w>, E: EventSink<'w, W>>(
        state: &mut Self::State,
        _: &'w W,
        event: &Event,
        event_sink: &mut E,
    ) {
        match event {
            Event::Status(event) => {
                if event.target == event_sink.source().id() && matches!(event.kind, StatusEventKind::Remove) {
                    if event.status == STANDARD_STEP || event.status == TECHNICAL_STEP {
                        state.step = StepGauge::None;
                    }
                }
            }
            _ => ()
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug)]
/// A custom cast error for Dancer actions.
pub enum DncError {
    /// The feather gauge was empty.
    Feather,
    /// Not under the effect of Threefold Fan Dance.
    Fan3,
    /// Not under the effect of Fourfold Fan Dance.
    Fan4,
    /// Not enough esprit.
    Esprit,
    /// Not under the effect of Flourishing Starfall.
    Starfall,
    /// Not under the effect of Flourishing/Silken Symmetry.
    Symmetry,
    /// Not under the effect of Flourishing/Silken Flow.
    Flow,
    /// Not under the effect of Flourishing Finish.
    Tillana,
    // Improvisation is not active.
    // ImprovFinish,
    /// Not under the effect of Standard Step.
    StandardStep,
    /// Not under the effect of Technical Step.
    TechnicalStep,
    /// Not under the effect of Standard/Technical Step.
    Step,
    /// Under the effect of Standard/Technical Step.
    StepInvalid,
    /// Closed Position is not the active action.
    PartnerActive,
    /// Ending is not the active action.
    PartnerInactive,
    /// Not under the effect of Last Dance Ready.
    LastDance,
    /// Not under the effect of Finishing Move Ready.
    FinishingMove,
    /// Not under the effect of Dance of the Dawn Ready.
    DanceOfTheDawn,
}

impl From<DncError> for EventError {
    fn from(value: DncError) -> Self {
        Self::Job(value.into())
    }
}

impl Display for DncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::match_single_binding)]
        match self {
            _ => {
                let _ = f;
                todo!()
            } // TODO: Error Display
        }
    }
}

const RANGED: ActionTargetting = ActionTargetting::single(25);
const CIRCLE: ActionTargetting = ActionTargetting::circle(5);
const DANCE: ActionTargetting = ActionTargetting::circle(15);
const TG_CIRCLE: ActionTargetting = ActionTargetting::target_circle(5, 25);

#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
#[var_consts {
    /// Returns `true` if the action may be used while under the effect of
    /// Standard/Technical Finish.
    pub const step_valid;
    /// Returns the base GCD recast time, or `None` if the action is not a gcd.
    pub const gcd: ScaleTime? = ScaleTime::skill(2500);
    /// Returns the human friendly name of the action.
    pub const name: &'static str = "";
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
pub enum DncAction {
    #[skill]
    #[name = "Cascade"]
    Cascade,
    #[skill]
    #[name = "Fountain"]
    Fountain,
    #[skill]
    #[name = "Windmill"]
    Windmill,
    #[category = ActionCategory::Weaponskill]
    #[gcd = ScaleTime::none(1500)]
    #[cooldown = 30000]
    #[name = "Standard Step"]
    StandardStep,
    #[skill]
    #[name = "ReverseCascade"]
    ReverseCascade,
    #[skill]
    #[name = "Bladeshower"]
    Bladeshower,
    #[ability]
    #[cooldown = 1000]
    #[name = "Fan Dance"]
    FanDance,
    #[skill]
    #[name = "Rising Windmill"]
    RisingWindmill,
    #[skill]
    #[name = "Fountainfall"]
    Fountainfall,
    #[skill]
    #[name = "Bloodshower"]
    Bloodshower,
    #[ability]
    #[cooldown = 1000]
    #[name = "Fan Dance II"]
    FanDance2,
    #[step_valid]
    #[ability]
    #[cooldown = 30000]
    #[cd_charges = 3]
    #[name = "En Avant"]
    EnAvant,
    #[step_valid]
    #[ability]
    #[cooldown = 60000]
    #[name = "Curing Waltz"]
    CuringWaltz,
    #[step_valid]
    #[ability]
    #[cooldown = 90000]
    #[name = "Shield Samba"]
    ShieldSamba,
    #[ability]
    #[cooldown = 30000]
    #[name = "Closed Position"]
    ClosedPosition,
    #[ability]
    #[cooldown = 1000]
    #[name = "Ending"]
    Ending,
    #[ability]
    #[cooldown = 120000]
    #[name = "Devilment"]
    Devilment,
    #[ability]
    #[cooldown = 1000]
    #[name = "Fan Dance III"]
    FanDance3,
    #[category = ActionCategory::Weaponskill]
    #[gcd = ScaleTime::none(1500)]
    #[cooldown = 120000]
    #[name = "Technical Step"]
    TechnicalStep,
    #[ability]
    #[cooldown = 60000]
    #[name = "Flourish"]
    Flourish,
    #[skill]
    #[name = "Saber Dance"]
    SaberDance,
    #[ability]
    #[cooldown = 120000]
    #[name = "Improvisation"]
    Improvisation,
    #[ability]
    #[cooldown = 1000]
    #[name = "Fan Dance IV"]
    FanDance4,
    #[skill]
    #[name = "Starfall Dance"]
    StarfallDance,
    #[step_valid]
    #[ability]
    #[gcd = ScaleTime::none(1000)]
    #[name = "Emboite"]
    Emboite,
    #[step_valid]
    #[ability]
    #[gcd = ScaleTime::none(1000)]
    #[name = "Entrechat"]
    Entrechat,
    #[step_valid]
    #[ability]
    #[gcd = ScaleTime::none(1000)]
    #[name = "Jete"]
    Jete,
    #[step_valid]
    #[ability]
    #[gcd = ScaleTime::none(1000)]
    #[name = "Pirouette"]
    Pirouette,
    #[step_valid]
    #[category = ActionCategory::Weaponskill]
    #[gcd = ScaleTime::none(1500)]
    #[name = "Standard Finish"]
    StandardFinish,
    #[step_valid]
    #[category = ActionCategory::Weaponskill]
    #[gcd = ScaleTime::none(1500)]
    #[name = "Technical Finish"]
    TechnicalFinish,
    #[ability]
    #[cooldown = 1500]
    #[name = "Improvised Finish"]
    ImprovisedFinish,
    #[skill]
    #[name = "Tillana"]
    Tillana,
    #[skill]
    #[name = "Last Dance"]
    LastDance,
    #[skill]
    #[cooldown = 30000]
    #[name = "Finishing Move"]
    FinishingMove,
    #[skill]
    #[name = "Dance of the Dawn"]
    DanceOfTheDawn,
}

impl JobAction for DncAction {
    fn category(&self) -> ActionCategory {
        self.category()
    }

    fn gcd(&self) -> bool {
        self.gcd().is_some()
    }
}

impl From<DncAction> for Action {
    fn from(value: DncAction) -> Self {
        Action::Job(value.into())
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// The state of the Dancer job gauges and cooldowns.
pub struct DncState {
    /// The combos for Dancer.
    pub combos: DncCombos,
    /// The Fourfold Feathers gauge.
    pub feathers: GaugeU8<4>,
    /// The Esprit gauge.
    pub esprit: GaugeU8<100>,
    /// The Dance Step gauge.
    pub step: StepGauge,
    /// The [`ActorId`] of the dance partner, or [`None`]
    /// if there is none.
    pub partner: Option<ActorId>,
}

impl JobState for DncState {
    fn advance(&mut self, _: u32) {}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// The Dance Step gauge.
pub enum StepGauge {
    #[default]
    /// No dance step is active.
    None,
    /// Technical step is active.
    Tech {
        /// The sequence of steps.
        steps: [Step; 4],
        /// The number of steps completed.
        completed: u8,
    },
    /// Standard step is active.
    Std {
        /// The sequence of steps.
        steps: [Step; 2],
        /// The number of steps completed.
        completed: u8,
    },
}

impl StepGauge {
    /// Executes the specified step.
    pub fn execute(&mut self, step: Step) {
        match self {
            StepGauge::Std { steps, completed } => {
                if let Some(next) = steps.get(*completed as usize) {
                    if *next == step {
                        *completed += 1;
                    }
                }
            }
            StepGauge::Tech { steps, completed } => {
                if let Some(next) = steps.get(*completed as usize) {
                    if *next == step {
                        *completed += 1;
                    }
                }
            }
            _ => (),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
/// A dance step
pub enum Step {
    Emboite,
    Entrechat,
    Jete,
    Pirouette,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// The combos for Dancer.
pub struct DncCombos {
    /// The main combo.
    pub main: ComboState<MainCombo>,
}

impl DncCombos {
    /// Checks that the main combo prerequisite is met for a certain action.
    pub fn check_main_for(&self, action: DncAction) -> bool {
        let c = match action {
            DncAction::Fountain => MainCombo::Cascade,
            DncAction::Bladeshower => MainCombo::Windmill,
            _ => return true,
        };
        self.main.check(c)
    }

    /// Advances the combos forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        self.main.advance(time);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
/// The possible states the main combo can be in.
pub enum MainCombo {
    /// Combo Action: Cascade is met.
    Cascade,
    /// Combo Action: Windmill is met.
    Windmill,
}

job_cd_struct! {
    DncAction =>

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Clone, Debug, Default)]
    /// The cooldown map for Dancer actions.
    pub DncCdMap

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, Debug)]
    /// The various cooldown groups a Dancer action can be part of.
    pub DncCdGroup

    "Standard Step"
    standard Standard: StandardStep FinishingMove;
    "Fan Dance"
    fan_1 Fan1: FanDance;
    "Fan Dance II"
    fan_2 Fan2: FanDance2;
    // "En Avant"
    // en_avant EnAvant: EnAvant;
    // "Curing Waltz"
    // waltz Waltz: CuringWaltz;
    // "Shield Samba"
    // samba Samba: ShieldSamba;
    "Closed Position"
    closed Closed: ClosedPosition Ending;
    "Devilment"
    devilment Devilment: Devilment;
    "Fan Dance III"
    fan_3 Fan3: FanDance3;
    "Technical Step"
    tech Tech: TechnicalStep;
    "Flourish"
    flourish Flourish: Flourish;
    // "Improvisation"
    // improv Improv: Improvisation;
    "Fan Dance IV"
    fan_4 Fan4: FanDance4;
    "Improvised Finish"
    improv_finish ImprovFinish: ImprovisedFinish;
}

bool_job_dist! {
    /// The random event for a Silken Symmetry/Flow proc.
    pub SymmFlowProc = 1 / 2;
    /// The random event for a Fourfold Feather proc.
    pub FeatherProc = 1 / 2;
    /// The random event for a Threefold Fan Dance proc.
    pub FanDance3Proc = 1 / 2;
}

/// The random event for a standard step sequence.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub struct StdStepSeqence;
impl Distribution<[Step; 2]> for StdStepSeqence {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> [Step; 2] {
        use Step::*;
        let mut steps = [Emboite, Entrechat, Jete, Pirouette];
        steps.partial_shuffle(rng, 2);
        [steps[0], steps[1]]
    }
}

/// The random event for a technical step sequence.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub struct TechStepSeqence;
impl Distribution<[Step; 4]> for TechStepSeqence {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> [Step; 4] {
        use Step::*;
        let mut steps = [Emboite, Entrechat, Jete, Pirouette];
        steps.shuffle(rng);
        steps
    }
}
