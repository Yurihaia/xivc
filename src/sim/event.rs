//! Ultilities for working with events.  
//! While none of the traits and structs in this module are needed for the [`Runtime`] to work,
//! they create reusable tools to reduce boilerplate code on most uses of the library.
use super::{ActorId, DamageInstance, EffectInstance, StatusEffect};

pub trait HasEvent<E> {
    fn new(source: E) -> Self where Self: Sized;
    fn get(&self) -> Option<&E>;
    fn get_mut(&mut self) -> Option<&mut E>;
    fn into(self) -> Result<E, Self> where Self: Sized;
}

#[macro_export]
macro_rules! event_wrapper {
    (
        $(#[$m:meta])?
        $v:vis enum $we:ident {
            $($ev:ident $(($p:ty))? ),*
            $(,)?
        }
    ) => {
        $(#[$m])?
        $v enum $we {
            $(
                $ev($crate::__event_wrapper_inner!( vt $ev $($p)? ))
            ),*
        }

        $($crate::__event_wrapper_inner!{
            impl $we $ev $crate::__event_wrapper_inner!( vt $ev $($p)? )
        })*

        impl $crate::sim::EventWrapper for $we {}
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __event_wrapper_inner {
    (vt $ev:ident $p:ty) => {
        $p
    };
    (vt $ev:ident) => {
        $ev
    };
    (impl $we:ident $ev:ident $t:ty) => {
        impl $crate::sim::HasEvent<$t> for $we {
            fn new(source: $t) -> Self {
                $we::$ev(source)
            }

            fn get(&self) -> Option<&$t> {
                match self {
                    Self::$ev(v) => Some(v),
                    _ => None,
                }
            }

            fn get_mut(&mut self) -> Option<&mut $t> {
                match self {
                    Self::$ev(v) => Some(v),
                    _ => None,
                }
            }
        
            fn into(self) -> Result<$t, Self> {
                match self {
                    Self::$ev(v) => Ok(v),
                    _ => Err(self),
                }
            }
        }
    }
}

pub trait EventWrapper {
    fn create<E>(source: E) -> Self where Self: Sized + HasEvent<E> {
        <Self as HasEvent<E>>::new(source)
    }
    fn get<E>(&self) -> Option<&E> where Self: HasEvent<E> {
        <Self as HasEvent<E>>::get(self)
    }
    fn get_mut<E>(&mut self) -> Option<&mut E> where Self: HasEvent<E> {
        <Self as HasEvent<E>>::get_mut(self)
    }
    fn into<E>(self) -> Result<E, Self> where Self: Sized + HasEvent<E> {
        <Self as HasEvent<E>>::into(self)
    }
}

event_wrapper!{
    pub enum CommonEvent {
        DamageEvent,
        CureEvent,
        EffectApplyEvent,
        EffectRemoveEvent,
    }
}

#[derive(Clone, Debug)]
pub struct CastEvent<A> {
    /// The actor that cast the action
    pub source: ActorId,
    /// The target actors that the action hits
    pub targets: Vec<ActorId>,
    /// The action that was cast
    pub action: A,
}

#[derive(Copy, Clone, Debug)]
pub struct DamageEvent {
    /// The actor that applied the damage
    pub source: ActorId,
    /// The target actor that is taking damage
    pub target: ActorId,
    /// The damage that is being applied
    pub damage: DamageInstance,
}

#[derive(Copy, Clone, Debug)]
pub struct CureEvent {
    /// The source of the cure
    pub source: ActorId,
    /// The target actor of the cure
    pub target: ActorId,
    /// The amount of HP restored by the cure
    pub health: u64,
}

#[derive(Copy, Clone, Debug)]
pub struct EffectApplyEvent {
    /// The actor that applied the effect
    pub source: ActorId,
    /// The target actor to apply the effect to
    pub target: ActorId,
    /// The effect to be applied
    pub effect: EffectInstance
}

#[derive(Copy, Clone, Debug)]
pub struct EffectRemoveEvent {
    /// The actor that removed the effect, or `None` if it fell off naturally
    pub source: Option<ActorId>,
    /// The target actor that the effect was removed from
    pub target: ActorId,
    /// The effect to remove from the target
    pub effect: (ActorId, StatusEffect),
}