use core::fmt::{Debug, Display};

use crate::world::{ActorId, Event, EventProxy, World};

pub trait Job {
    type Action: JobAction + 'static;
    type State: JobState + 'static;
    type Error: Display + Debug;
    type Event: Clone + Debug;

    fn init_cast<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &mut Self::State,
        world: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    );

    fn cast_snap<'w, E: EventProxy, W: World>(
        action: Self::Action,
        state: &mut Self::State,
        world: &'w W,
        src: &'w W::Actor<'w>,
        event_sink: &mut E,
    );

    #[allow(unused_variables)]
    fn event<E: EventProxy, W: World>(
        state: &mut Self::State,
        world: &W,
        event: &Event,
        src: Option<ActorId>,
        event_sink: &mut E,
    ) {
        // don't require an impl
    }
}

pub trait JobAction: Copy + Debug {
    fn action_type(&self) -> ActionType;
}

pub trait JobState: Clone + Debug + Default {
    fn advance(&mut self, time: u32);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionType {
    Weaponskill,
    Spell,
    Ability,
}
