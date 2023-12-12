//! Event queue implementations.
//!
//! This module contains various implementations for event queues
//! that can be used in a simulation. TODO write more here
//!
//! # Requirements for a valid event queue
//!
//! An event queue has a number of invariants that must be upheld,
//! both by users of the queue and by implementations. Every event queue in this module
//! will uphold these requirements, and will document any deviations.
//!
//! <h4 id="current"><a href=#current>The "current time" of an event queue</a></h4>
//!
//! Many requirements for an event queue refer to a concept named the "current time".
//! This refers to
//!
//! > **the time of the event most recently retrieved from the queue.**
//!
//! This current time is what the `delay` in [`EventProxy::event`] is relative to.
//! The difference between the current time and the time of the next retrieved event
//! is the value which will be used when calling various `advance` functions.
//!
//! ### Requirements for users
//!
//! An event queue's events must be *monotonically increasing*. This means that it is
//! a logic error for any event to be pushed to the queue with a time less than the
//! **current time** of the event queue. While implementations are encouraged to panic
//! upon this happening, they are not required to, and may result in behavior like incorrect
//! results or non-termination. However, it will never result in undefined behavior.
//!
//! ### Requirements for implementors
//!
//! An event queue's implementation must be a bit stricter than for example,
//! [`BinaryHeap`] in the standard library. Specifically, there is a defined order
//! that successive event retrievals with the same time must follow. If two events are pushed
//! to the queue with the same time, then the event that was pushed later should always be
//! returned first. This comes for free with a queue such as [`RadixEventQueue`], but needs to be
//! specifically implemented for a binary heap based queue.
//!
//! While this property is often not nescessary to rely on because of the system of snapshotting
//! and damage delay that is present, in some cases it may still be nescessary.
//!
//! [`EventProxy`]: super::EventProxy
//! [`EventProxy::event`]: super::EventProxy::event
//! [`event`]: super::EventProxy::event
//! [`BinaryHeap`]: alloc::collections::BinaryHeap
//! [`RadixEventQueue`]: radix::RadixEventQueue
//! [`World`]: super::World

pub mod radix;

pub use radix::RadixEventQueue;