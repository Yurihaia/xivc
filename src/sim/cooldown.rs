use std::mem;

use super::ActionError;


#[derive(Copy, Clone, Debug)]
pub struct CooldownManager<C> {
    global: u32,
    ani_lock: u32,
    custom: C,
    advanced: u32,
}

impl<C> CooldownManager<C> where C: ActionCooldown {
    pub fn new(action: C) -> Self {
        Self {
            advanced: 0,
            ani_lock: 0,
            global: 0,
            custom: action,
        }
    }
    pub fn global(&self) -> u32 {
        self.global
    }
    pub fn ani_lock(&self) -> u32 {
        self.ani_lock
    }
    pub fn action(&self, ac: &C::Action) -> u32 {
        self.custom.get(ac)
    }
    pub fn advanced(&mut self) -> u32 {
        mem::replace(&mut self.advanced, 0)
    }
    pub fn apply_global(&mut self, gcd: u32) {
        self.global = gcd;
    }
    pub fn apply_ani_lock(&mut self, ani_lock: u32) {
        self.ani_lock = ani_lock;
    }
    pub fn apply_action(&mut self, ac: &C::Action, time: u32) {
        self.custom.incr_cd(ac, time);
    }
    pub fn advance(&mut self, time: u32) {
        self.global = self.global.saturating_sub(time);
        self.ani_lock = self.ani_lock.saturating_sub(time);
        self.custom.advance(time);
        self.advanced += time;
    }
    pub fn error<T>(&self, gcd: bool) -> Result<(), ActionError<T>> {
        if gcd && self.global > 0 {
            if self.global >= self.ani_lock {
                Err(ActionError::GlobalCooldown(self.global))
            } else {
                Err(ActionError::AnimationLock(self.ani_lock))
            }
        } else if self.ani_lock > 0 {
            Err(ActionError::AnimationLock(self.ani_lock))
        } else {
            Ok(())
        }
    }
}

pub trait ActionCooldown {
    type Action;

    fn new() -> Self where Self: Sized;
    fn get(&self, ac: &Self::Action) -> u32;
    fn ref_mut(&mut self, ac: &Self::Action) -> &mut u32;
    fn incr_cd(&mut self, ac: &Self::Action, time: u32) {
        *self.ref_mut(ac) += time;
    }
    fn set_cd(&mut self, ac: &Self::Action, time: u32) {
        *self.ref_mut(ac) = time;
    }
    fn advance(&mut self, time: u32);
}