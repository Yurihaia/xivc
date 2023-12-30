//! Status effect handling.
//!
//! The term Status Effect encompasses both buffs and debuffs,
//! as well as any "proc" type effects.
//!
//! To define a status effect, you should use the [`status_effect!`] macro.
//! Each status effect has a [`VTable`] that contains the [name] and [duration]
//! of the effect, as well the functions used to modify values, such as [damage]
//! or [crit chance].
//!
//! A specific instant of a status effect on an actor is represented by [`StatusInstance`].
//! This struct contains the [ID] of the actor who applied the status, a reference
//! to the effect [`VTable`], and the remaining duration and stacks of the status.
//!
//! [`VTable`]: StatusVTable
//! [name]: StatusVTable::name
//! [duration]: StatusVTable::duration
//! [damage]: StatusVTable::damage
//! [crit chance]: StatusVTable::crit
//! [ID]: super::ActorId
//! [`status_effect!`]: crate::status_effect

use core::{
    fmt::{self, Debug},
    hash,
    ops::Deref,
};

use crate::{
    enums::{DamageElement, DamageInstance, DamageType},
    math::{Buffs, EotSnapshot, PlayerStats, SpeedStat},
};

use super::{Actor, ActorId, EventSink, World};

#[derive(Debug, Clone, Copy)]
/// A specific instance of a status effect inflicted upon an actor.
pub struct StatusInstance {
    /// The ID of the actor who inflicted this status.
    pub source: ActorId,
    /// A reference to the status VTable.
    pub effect: StatusEffect,
    /// The time remaining on the status.
    pub time: u32,
    /// The number of stacks of the status.
    pub stack: u8,
}

impl StatusInstance {
    /// Creates a new status instance of an effect applied by some source actor.
    ///
    /// The effect duration will be the duration specified by the effect [`VTable`]
    /// and the stacks will be set to `1`.
    ///
    /// [`VTable`]: StatusVTable
    pub const fn new(source: ActorId, effect: StatusEffect) -> Self {
        Self::new_stack(source, effect, 1)
    }
    /// Creates a new status instance of an effect applied by some source actor.
    ///
    /// The effect duration will be the duration specified by the effect [`VTable`]
    /// and the stacks will be set to the value passed in to the function.
    ///
    /// [`VTable`]: StatusVTable
    pub const fn new_stack(source: ActorId, effect: StatusEffect, stack: u8) -> Self {
        Self {
            source,
            effect,
            stack,
            time: effect.0.duration,
        }
    }
    /// Removes the status effect from the actor.
    pub fn remove(&mut self) {
        self.stack = 0;
    }
    /// Decreases the number of stacks of the status, down to a minimum of `0`.
    ///
    /// If the number stacks reaches `0`, the status instance will
    /// be removed from the actor.
    pub fn sub_stacks(&mut self, stacks: u8) {
        self.stack = self.stack.saturating_sub(stacks);
    }
    /// Increases the number of stacks of the status, up to a certain maximum.
    pub fn add_stacks(&mut self, stacks: u8, max: u8) {
        self.stack = (self.stack + stacks).min(max);
    }
    /// Advances the status instance forward by a certain amount of time.
    ///
    /// See TODO: Advance Functions for more information.
    pub fn advance(&mut self, time: u32) {
        if !self.effect.permanent {
            self.time = self.time.saturating_sub(time);
            if self.time == 0 {
                self.stack = 0;
            }
        }
    }
}

#[derive(Copy, Clone)]
/// A reference to a certain [`StatusVTable`].
///
/// This is the struct you should use when storing a status
/// effect somewhere. It implements various useful traits.
/// [`Eq`] and [`Hash`] use the address of the VTable for
/// their respective purposes, not the VTable itself.
///
/// This struct auto-derefs to the inner VTable, so any values from the VTable
/// can easily be accessed through this struct.
///
/// [`Hash`]: hash::Hash
pub struct StatusEffect(&'static StatusVTable);

impl StatusEffect {
    /// Creates a new [`StatusEffect`] from a certain VTable.
    ///
    /// This function will rarely be called, as its primary use
    /// is inside the [`status_effect!`] macro.
    ///
    /// [`status_effect!`]: crate::status_effect
    pub const fn new(vtable: &'static StatusVTable) -> Self {
        Self(vtable)
    }
}
impl PartialEq for StatusEffect {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}
impl Eq for StatusEffect {}
impl Deref for StatusEffect {
    type Target = StatusVTable;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl hash::Hash for StatusEffect {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        <usize as hash::Hash>::hash(&(self.0 as *const _ as usize), state);
    }
}
impl fmt::Debug for StatusEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StatusEffect(\"{}\" @ {:?})",
            self.0.name, self.0 as *const _
        )
    }
}

#[derive(Debug)]
/// The VTable for some specific status effect.
///
/// This struct should almost always be created with
/// the [`status_effect!`] macro. In rare cases where you
/// need to construct it yourself, it should always be
/// behind a `const` or `static` item.
///
/// [`status_effect!`]: crate::status_effect
pub struct StatusVTable {
    /// The name of the status effect.
    pub name: &'static str,
    /// If true, the status effect will last forever.
    pub permanent: bool,
    /// The damage modifier of the status effect.
    pub damage: ValueModifier<DamageModifierFn>,
    /// The Critical Hit modifier of the status effect.
    pub crit: ValueModifier<ProbabilityModifierFn>,
    /// The Direct Hit modifier of the status effect.
    pub dhit: ValueModifier<ProbabilityModifierFn>,
    /// The haste modifier of the status effect.
    pub haste: Option<SpeedModifierFn>,
    /// The player stats modifier of the status effect.
    pub stats: Option<StatsModifierFn>,
    /// The default duration of the status effect.
    pub duration: u32,
}

impl StatusVTable {
    /// Creates a default VTable with the specified name.
    pub const fn empty(name: &'static str) -> Self {
        StatusVTable {
            name,
            permanent: false,
            damage: ValueModifier::empty(),
            crit: ValueModifier::empty(),
            dhit: ValueModifier::empty(),
            duration: 0,
            haste: None,
            stats: None,
        }
    }
}

// this can be a trait but statuseffect shouldn't be
// because status effects always have the same receiver
/// A trait describing a job status effect.
///
/// These effects come from job traits or gauges, for example,
/// Dark Knight's Darkside, Bard's Army's Paeon, etc.
///
/// Note that unlike [`StatusEffect`], this uses normal
/// trait objects.
pub trait JobEffect: Debug {
    /// The damage modifier for the job effect.
    #[allow(unused_variables)]
    fn damage(&self, damage: u64, dmg_ty: DamageType, dmg_el: DamageElement) -> u64 {
        damage
    }
    /// The Critical Hit modifier for the job effect.
    fn crit(&self) -> u64 {
        0
    }
    /// The Direct Hit modifier for the job effect.
    fn dhit(&self) -> u64 {
        0
    }
    /// The haste modifier for the job effect.
    fn haste(&self) -> u64 {
        100
    }
}

#[macro_export]
/// Creates a named wrapper around a job's state that can implement
/// [`JobEffect`].
///
/// This `struct` must be `#[repr(transparent)]`, but otherwise,
/// no other attributes are applied by default.
///
/// TODO: More docs.
macro_rules! job_effect_wrapper {
    (
        $(#[$m:meta])*
        $v:vis struct $etyid:ident ( $fv:vis $stty:ty );
    ) => {
        #[repr(transparent)]
        $(#[$m])*
        $v struct $etyid( $fv $stty );
        impl $etyid where Self: $crate::world::status::JobEffect {
            /// Creates a new `&dyn JobEffect` from a reference to a job's state struct,
            /// using the functions defined by this struct's `JobEffect` implementation.
            ///
            /// TODO: More docs.
            pub const fn new<'a>(value: &'a $stty) -> &'a dyn $crate::world::status::JobEffect {
                // Safety: `ArmysJobEffect` is `repr(transparent)`
                // and the lifetimes are the same,
                // so the resulting reference is valid.
                (unsafe { &*(value as *const _ as *const Self) }) as _
            }
        }
    };
}

#[derive(Debug)]
/// A snapshot of the statuses for some event.
pub struct StatusSnapshot<'a> {
    /// The status effects present on the source of the event.
    pub source: &'a [StatusInstance],
    /// The status effects present on the target of the event.
    ///
    /// This may be empty if no meaningful target exists.
    pub target: &'a [StatusInstance],
    // this is a list of all of the "status effects" that come from
    // job gauge or trait related things
    // some examples of this include: darkside, army's paeon haste, enochian,
    // AF/UI (i believe it goes here), greased lightning, etc.
    // note that Main & Mend (things that say "base action damage")
    // do NOT apply here, they go in "traits" in the damage formula
    /// The job effects present on the source of the event.
    ///
    /// This may be empty if the source has no job effects, or
    /// if the source is not a player.
    pub job: Option<&'a dyn JobEffect>,
}

impl StatusSnapshot<'static> {
    /// Creates an empty status snapshot.
    ///
    /// This is often useful when interacting with the [`math`] module
    /// manually.
    ///
    /// [`math`]: crate::math
    pub const fn empty() -> Self {
        Self {
            source: &[],
            target: &[],
            job: None,
        }
    }
}

impl<'a> Buffs for StatusSnapshot<'a> {
    // doing these functions using iterators was less concise because of the incoming vs outgoing difference
    fn damage(&self, base: u64, dmg_ty: DamageType, dmg_el: DamageElement) -> u64 {
        let mut acc = base;
        if let Some(x) = self.job {
            acc = x.damage(base, dmg_ty, dmg_el);
        }
        for x in self.target {
            if let Some(f) = x.effect.damage.incoming {
                acc = f(*x, acc, dmg_ty, dmg_el);
            }
        }
        for x in self.source {
            if let Some(f) = x.effect.damage.outgoing {
                acc = f(*x, acc, dmg_ty, dmg_el);
            }
        }
        acc
    }

    fn crit_chance(&self, base: u64) -> u64 {
        let mut acc = base;
        if let Some(x) = self.job {
            acc += x.crit();
        }
        for x in self.target {
            if let Some(f) = x.effect.crit.incoming {
                acc += f(*x);
            }
        }
        for x in self.source {
            if let Some(f) = x.effect.crit.outgoing {
                acc += f(*x);
            }
        }
        acc.min(100)
    }

    fn dhit_chance(&self, base: u64) -> u64 {
        let mut acc = base;
        if let Some(x) = self.job {
            acc += x.dhit();
        }
        for x in self.target {
            if let Some(f) = x.effect.dhit.incoming {
                acc += f(*x);
            }
        }
        for x in self.source {
            if let Some(f) = x.effect.dhit.outgoing {
                acc += f(*x);
            }
        }
        acc.min(100)
    }

    fn stats(&self, base: PlayerStats) -> PlayerStats {
        let mut acc = base;
        for x in self.source {
            if let Some(f) = x.effect.stats {
                acc = f(*x, acc);
            }
        }
        acc
    }

    // pretty sure this works
    // doubt it will ever matter bc its not like
    // jobs have more than one speed boost
    fn haste(&self, base: u64) -> u64 {
        let mut acc = 100;
        if let Some(x) = self.job {
            acc *= x.haste();
            acc /= 100;
        }
        for x in self.source {
            if let Some(f) = x.effect.haste {
                acc *= f(*x);
                acc /= 100;
            }
        }
        base * acc / 100
    }
}

/// A function that modifies a specific damage instance.
///
/// This should almost always be multiplicative.
#[rustfmt::skip]
pub type DamageModifierFn = fn(
    status: StatusInstance,
    damage: u64,
    dmg_ty: DamageType,
    dmg_el: DamageElement,
) -> u64;
// Chance is a scaled by 1000 to get probability (every 1 is 0.1%)
// For instance, Chain adds 100
/// A function that returns an increase to the probability of a critical/direct hit occuring.
pub type ProbabilityModifierFn = fn(status: StatusInstance) -> u64;
// for potions and basically potions only?
// technically food but that can usually be put into stats
/// A function that modifies the stats of a player.
pub type StatsModifierFn = fn(status: StatusInstance, stats: PlayerStats) -> PlayerStats;
// haste buffs - returns the buff as a multiplier scaled by 100
/// A function that returns a haste buff/debuff.
pub type SpeedModifierFn = fn(status: StatusInstance) -> u64;

#[macro_export]
/// Creates a new [`StatusEffect`].
///
/// This macro is the primary way to create a new status effect.
///
/// # Examples
///
/// An example of an invocation for this macro is
///
/// ```
/// # use xivc_core::world::status::StatusEffect;
/// # use xivc_core::status_effect;
///
/// // Status effects should always be inside `const` items.
/// // The VTable will be stored in read-only memory and
/// //  the reference will be able to be freely copied around.
/// const MY_EFFECT: StatusEffect = status_effect!(
///     // This string literal is the name of the status effect.
///     "My Custom Effect"
///     // This number is the time in milliseconds that the
///     // status effect should last by default.
///     // Here it is 30 seconds.
///     // It may also be the keyword `permanent` to signify
///     // that the effect's duration should never be
///     // reduced, and the effect will last forever.
///     30000
///     // Next up comes the body of the effect.
///     // This is where all of the modifiers for the effect
///     // are defined. This may be omitted if the effect
///     // does not do anything and is just used for
///     // job logic, like Firestart on Black Mage.
///     {
///         // This `damage` keyword means that the following
///         // modifier is a damage modifier.
///         damage {
///             // The `out` keyword specifies that the outgoing damage
///             // should be modified. It may also be `in` to specify
///             // that incoming damage will be modified.
///             out =
///             // This is the actual damage multiplier. Note that this entire
///             // thing is NOT an expression. The division symbol is part of the
///             // macro invocation. This will be implemented as `(damage * 120) / 100`.
///             // This is because FFXIV does all of its math using integers.
///                 120 / 100
///         }
///         // This `crit` keyword means that the following modifier
///         // will increase the critical hit rate, scaled by 1000.
///         crit {
///             // Again, this means that the outgoing critical hit rate
///             // will be increased by 20%. This modifier will always be
///             // multiplicative, and caps at 100%.
///             out = 200
///         }
///         // This `haste` keyword will modify the gcd cast and recast speed,
///         // as well as auto-attack frequency. Unlike damage, everything inside
///         // the braces is a single expression. This is a multiplier for the
///         // gcd speed. `100 - 13` is how we would write a 13% haste buff.
///         haste { |_| 100 - 13 }
///         // Other options for modifiers are `dhit` which functions
///         // like `crit`, but for direct hit, as well as `stats`,
///         // which is used for Potions and Food. This modifier takes
///         // a function pointer inside the braces with the signature
///         // `fn(StatusInstance, PlayerStats) -> PlayerStats`.
///     }
/// );
/// ```
/// Of course, this is a contrived example for the purpose of documentation.
/// Some examples of real status effects that can be written may look like:
/// ```
/// # use xivc_core::world::status::StatusEffect;
/// # use xivc_core::status_effect;
/// // a damage buff
/// pub const NO_MERCY: StatusEffect = status_effect!(
///     "No Mercy" 20000 { damage { out = 120 / 100 } }
/// );
/// // a proc
/// pub const FAN_DANCE_4: StatusEffect = status_effect!("Fourfold Fan Dance" 30000);
/// // a permanent status
/// pub const Kardia: StatusEffect = status_effect!("Kardia" permanent);
/// ```
macro_rules! status_effect {
    (
        $name:literal $ptk:tt $({
            $(
                $prop:ident { $($t:tt)* }
            )*
        })?
    ) => {
        $crate::world::status::StatusEffect::new(&$crate::world::status::StatusVTable {
            permanent: $crate::__status_effect_inner!(ptk $ptk),
            duration: $crate::__status_effect_inner!(ptd $ptk),
            $($(
                $prop: $crate::__status_effect_inner!(vm $prop { $($t)* }),
            )*)?
            ..$crate::world::status::StatusVTable::empty($name)
        })
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __status_effect_inner {
    (ptk permanent) => { true };
    (ptk $dur:expr) => { false };
    (ptd permanent) => { 0 };
    (ptd $dur:expr) => { $dur };
    (vm damage {
        $k:ident = $mul:literal / $div:literal
    }) => {
        $crate::__status_effect_inner!(modfn $k |_, d, _, _|  d * $mul / $div)
    };
    (vm crit {
        $k:ident = $add:literal
    }) => {
        $crate::__status_effect_inner!(modfn $k |_|$add)
    };
    (vm dhit {
        $k:ident = $add:literal
    }) => {
        $crate::__status_effect_inner!(modfn $k |_| $add)
    };
    (vm $i:ident {
        $k:ident = $e:expr
    }) => {
        $crate::__status_effect_inner!(modfn $k $e)
    };
    (vm stats { $e:expr }) => { Some($e) };
    (vm haste { $e:expr }) => { Some($e) };
    (modfn in $($t:tt)*) => { $crate::world::status::ValueModifier::incoming($($t)*) };
    (modfn out $($t:tt)*) => { $crate::world::status::ValueModifier::outgoing($($t)*) };
}

#[derive(Copy, Clone, Debug)]
/// A modifier for a specific type of value, either incoming or outgoing.
pub struct ValueModifier<T> {
    /// The modifier for incoming events.
    pub incoming: Option<T>,
    /// The modifier for outgoing events.
    pub outgoing: Option<T>,
}
impl<T> Default for ValueModifier<T> {
    fn default() -> Self {
        ValueModifier {
            incoming: None,
            outgoing: None,
        }
    }
}
impl<T> ValueModifier<T> {
    /// Creates a new empty value modifier.
    pub const fn empty() -> Self {
        Self {
            incoming: None,
            outgoing: None,
        }
    }
    /// Creates a new solely outgoing value modifier.
    pub const fn outgoing(t: T) -> Self {
        Self {
            incoming: None,
            outgoing: Some(t),
        }
    }
    /// Creates a new solely incoming value modifier.
    pub const fn incoming(t: T) -> Self {
        Self {
            incoming: Some(t),
            outgoing: None,
        }
    }
}

/// Consumes a status from an actor, returning `true`
/// if the status was successfully consumed.
///
/// The `time` parameter is the delay that the Status Remove event
/// will be submitted at.
pub fn consume_status<'w, W: World + 'w>(
    event_sink: &mut impl EventSink<'w, W>,
    status: StatusEffect,
    time: u32,
) -> bool {
    let actor = event_sink.source();
    let present = actor.has_own_status(status);
    if present {
        event_sink.event(
            StatusEvent::remove(status, actor.id(), actor.id()).into(),
            time,
        );
    }
    present
}

/// Consumes a stack of a status from an actor, returning `true`
/// if the stack was successfully consumed.
///
/// The `time` parameter is the delay that the Status Remove event
/// will be submitted at.
pub fn consume_status_stack<'w, W: World + 'w>(
    event_sink: &mut impl EventSink<'w, W>,
    status: StatusEffect,
    time: u32,
) -> bool {
    let actor = event_sink.source();
    let present = actor.has_own_status(status);
    if present {
        event_sink.event(
            StatusEvent::remove_stacks(status, 1, actor.id(), actor.id()).into(),
            time,
        );
    }
    present
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// An event of status modification.
pub struct StatusEvent {
    /// The status effect to be modified.
    pub status: StatusEffect,
    /// The source actor for the event.
    pub source: ActorId,
    /// The target actor of the event.
    pub target: ActorId,
    /// The kind of modification that will be executed.
    pub kind: StatusEventKind,
}
impl StatusEvent {
    /// Applies a status effect with a certain number of stacks on to a target actor.
    pub const fn apply(status: StatusEffect, stacks: u8, source: ActorId, target: ActorId) -> Self {
        Self {
            kind: StatusEventKind::Apply {
                duration: status.0.duration,
                stacks,
            },
            status,
            source,
            target,
        }
    }
    /// Applies a DoT status effect with a certain number of stacks on to a target actor.
    pub const fn apply_dot(
        // what an awful function signature
        status: StatusEffect,
        snapshot: EotSnapshot,
        stacks: u8,
        source: ActorId,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::ApplyDot {
                duration: status.0.duration,
                snapshot,
                stacks,
            },
            status,
            source,
            target,
        }
    }
    /// Removes a status effect from a target actor.
    pub const fn remove(status: StatusEffect, source: ActorId, target: ActorId) -> Self {
        Self {
            kind: StatusEventKind::Remove,
            status,
            source,
            target,
        }
    }
    /// Removes a certain number of stacks from a status effect from a target actor.
    pub const fn remove_stacks(
        status: StatusEffect,
        stacks: u8,
        source: ActorId,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::RemoveStacks { stacks },
            status,
            source,
            target,
        }
    }
    /// Adds a certain number of stacks from a status effect from a target actor.
    pub const fn add_stacks(
        status: StatusEffect,
        stacks: u8,
        max: u8,
        source: ActorId,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::AddStacks { stacks, max },
            status,
            source,
            target,
        }
    }
    /// Applies a status effect or extends the duration if it already exists on a target actor.
    ///
    /// The number of `stacks` will be overwritten. To add stacks, use [`apply_or_add_stacks`].
    ///
    /// [`apply_or_add_stacks`]: StatusEvent::apply_or_add_stacks
    pub const fn apply_or_extend(
        status: StatusEffect,
        stacks: u8,
        multiple: u32,
        source: ActorId,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::ApplyOrExtend {
                duration: status.0.duration,
                stacks,
                max: status.0.duration * multiple,
            },
            status,
            source,
            target,
        }
    }
    /// Applies a status effect or adds to the number of stacks if it already exists on a target actor.
    ///
    /// The number of `stacks` will be added to the effect instance if it already exists. The duration
    /// will be unchanged. If you need to refresh the duration, use [`apply_or_extend`].
    ///
    /// [`apply_or_extend`]: StatusEvent::apply_or_extend
    pub const fn apply_or_add_stacks(
        status: StatusEffect,
        stacks: u8,
        max: u8,
        source: ActorId,
        target: ActorId,
    ) -> Self {
        Self {
            kind: StatusEventKind::ApplyOrAddStacks {
                duration: status.0.duration,
                stacks,
                max,
            },
            status,
            source,
            target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
/// The kind of modification a status event will do.
///
/// Look at [`StatusEffect`] for more details
/// on what each `StatusEventKind` does.
pub enum StatusEventKind {
    Apply {
        duration: u32,
        stacks: u8,
    },
    ApplyDot {
        duration: u32,
        snapshot: EotSnapshot,
        stacks: u8,
    },
    Remove,
    RemoveStacks {
        stacks: u8,
    },
    AddStacks {
        stacks: u8,
        max: u8,
    },
    ApplyOrExtend {
        duration: u32,
        stacks: u8,
        max: u32,
    },
    ApplyOrAddStacks {
        duration: u32,
        stacks: u8,
        max: u8,
    },
}

/// A helper trait for easily submitting status events on to an event sink.
pub trait StatusEventExt<'w, W: World + 'w>: EventSink<'w, W> {
    /// Applies a status effect with a certain number of stacks on to a target actor.
    fn apply_status(&mut self, status: StatusEffect, stacks: u8, target: ActorId, delay: u32) {
        self.event(
            StatusEvent::apply(status, stacks, self.source().id(), target).into(),
            delay,
        )
    }
    /// Applies a status effect with a certain number of stacks on to a target actor.
    fn apply_status_cascade_remove(
        &mut self,
        status: StatusEffect,
        stacks: u8,
        target: ActorId,
        delay: u32,
        cascade: u32,
    ) {
        self.event(
            StatusEvent {
                source: self.source().id(),
                target,
                status,
                kind: StatusEventKind::Apply {
                    duration: status.duration + cascade,
                    stacks,
                },
            }
            .into(),
            delay,
        )
    }
    /// Applies a DoT status effect with a certain number of stacks on to a target actor.
    fn apply_dot<'a>(
        &mut self,
        status: StatusEffect,
        damage: DamageInstance,
        stat: SpeedStat,
        stacks: u8,
        target: ActorId,
        time: u32,
    ) {
        let actor = self.source();
        let snapshot = actor.dot_damage_snapshot(damage, stat, target);
        self.event(
            StatusEvent::apply_dot(status, snapshot, stacks, actor.id(), target).into(),
            time,
        )
    }
    /// Removes a status effect from a target actor.
    fn remove_status(&mut self, status: StatusEffect, target: ActorId, delay: u32) {
        self.event(
            StatusEvent::remove(status, self.source().id(), target).into(),
            delay,
        )
    }
    /// Removes a certain number of stacks from a status effect from a target actor.
    fn remove_stacks(&mut self, status: StatusEffect, stacks: u8, target: ActorId, delay: u32) {
        self.event(
            StatusEvent::remove_stacks(status, stacks, self.source().id(), target).into(),
            delay,
        )
    }
    /// Adds a certain number of stacks from a status effect from a target actor.
    fn add_stacks(
        &mut self,
        status: StatusEffect,
        stacks: u8,
        max: u8,
        target: ActorId,
        delay: u32,
    ) {
        self.event(
            StatusEvent::add_stacks(status, stacks, max, self.source().id(), target).into(),
            delay,
        )
    }
    /// Applies a status effect or extends the duration if it already exists on a target actor.
    ///
    /// The number of `stacks` will be overwritten. To add stacks, use [`apply_or_add_stacks`].
    ///
    /// [`apply_or_add_stacks`]: StatusEventExt::apply_or_add_stacks
    fn apply_or_extend_status(
        &mut self,
        status: StatusEffect,
        stacks: u8,
        multiple: u32,
        target: ActorId,
        delay: u32,
    ) {
        self.event(
            StatusEvent::apply_or_extend(status, stacks, multiple, self.source().id(), target)
                .into(),
            delay,
        )
    }
    /// Applies a status effect or adds to the number of stacks if it already exists on a target actor.
    ///
    /// The number of `stacks` will be added to the effect instance if it already exists. The duration
    /// will be unchanged. If you need to refresh the duration, use [`apply_or_extend_status`].
    ///
    /// [`apply_or_extend_status`]: StatusEventExt::apply_or_extend_status
    fn apply_or_add_stacks(
        &mut self,
        status: StatusEffect,
        stacks: u8,
        max: u8,
        target: ActorId,
        delay: u32,
    ) {
        self.event(
            StatusEvent::apply_or_add_stacks(status, stacks, max, self.source().id(), target)
                .into(),
            delay,
        )
    }
}

impl<'w, W: World + 'w, E: EventSink<'w, W>> StatusEventExt<'w, W> for E {}
