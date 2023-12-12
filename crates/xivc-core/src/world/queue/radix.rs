//! An event queue implemented as a radix heap.

use alloc::{alloc::Layout, boxed::Box, vec::Vec};
use core::{iter::FusedIterator, mem};

/// An event queue implemented as a radix heap.
#[derive(Clone, Debug)]
pub struct RadixEventQueue<E> {
    time: u32,
    // the head vec. the `min` field is just the `time` field here
    head: Vec<E>,
    // this is stored on the heap to make this struct far cheaper to move around
    buckets: Box<Buckets<E>>,
    // a bitmap of the buckets from 0..32 that are not empty
    filled: u32,
}

#[derive(Clone, Debug)]
struct Bucket<E> {
    min: u32,
    vec: Vec<(u32, E)>,
}

type Buckets<E> = [Bucket<E>; 32];

impl<E> Bucket<E> {
    const fn new() -> Self {
        Self {
            min: u32::MAX,
            vec: Vec::new(),
        }
    }

    // push an event into the bucket
    // updating the minimum if relevant
    fn push(&mut self, time: u32, event: E) {
        self.min = self.min.min(time);
        self.vec.push((time, event));
    }
}

impl<E> RadixEventQueue<E> {
    /// Creates a new event queue.
    /// 
    /// # Examples
    /// ```
    /// # use xivc_core::world::queue::RadixEventQueue;
    /// let mut queue = RadixEventQueue::<u32>::new();
    /// 
    /// assert!(queue.is_empty());
    /// assert_eq!(queue.pop(), None);
    /// assert_eq!(queue.time(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            time: 0,
            head: Vec::new(),
            buckets: new_buckets(),
            filled: 0,
        }
    }

    /// Pushes an event to the queue at the specified time.
    ///
    /// The most recently pushed event at any specific time will
    /// always be the first to be [popped].
    ///
    /// # Panics
    /// Panics if the `time` if less than the current time.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::world::queue::RadixEventQueue;
    /// let mut queue = RadixEventQueue::new();
    /// 
    /// queue.push(0, "first");
    /// queue.push(1, "second");
    /// queue.push(5, "fourth"); // note the reversed order here
    /// queue.push(5, "third");
    /// 
    /// assert_eq!(queue.pop(), Some((0, "first")));
    /// assert_eq!(queue.pop(), Some((1, "second")));
    /// assert_eq!(queue.pop(), Some((5, "third")));
    /// assert_eq!(queue.pop(), Some((5, "fourth")));
    /// assert_eq!(queue.pop(), None);
    /// ```
    ///
    /// [popped]: RadixEventQueue::pop
    pub fn push(&mut self, time: u32, event: E) {
        assert!(self.time <= time);
        // a radix dist of 0 is the head bucket, while radix dists of 1..=32
        // is the index of the bucket in `buckets` + 1
        if let Some(bucket) = radix_dist(self.time, time).checked_sub(1) {
            self.buckets[bucket as usize].push(time, event);
            self.filled |= 1 << bucket;
        } else {
            self.head.push(event);
        }
    }

    /// Pushes a sequence of events to the queue at the specified time.
    ///
    /// These events will be popped in the same order as they are currently in.
    /// Specifically, if `push_ordered` is called and then no more events are added
    /// to the queue, the first event [popped] with a matcing `time` will be `events[0]`,
    /// then `events[1]`, `events[2]`, etc.
    ///
    /// Note that the iterator must be a [`DoubleEndedIterator`] because of how the queue
    /// is internally implemented. If you cannot get an appropriate iterator,
    /// you may achieve the desired effect by calling [`push`] with the values in the
    /// opposite order.
    ///
    /// # Panics
    /// Panics if the `time` if less than the current time.
    ///
    /// # Examples
    /// ```
    /// # use xivc_core::world::queue::RadixEventQueue;
    /// let mut queue = RadixEventQueue::<u8>::new();
    /// queue.push_ordered(10, [0, 1, 2, 3]);
    ///
    /// assert_eq!(queue.pop(), Some((10, 0)));
    /// assert_eq!(queue.pop(), Some((10, 1)));
    /// assert_eq!(queue.pop(), Some((10, 2)));
    /// assert_eq!(queue.pop(), Some((10, 3)));
    /// ```
    ///
    /// [popped]: RadixEventQueue::pop
    /// [`push`]: RadixEventQueue::push
    pub fn push_ordered<I>(&mut self, time: u32, events: I)
    where
        I: IntoIterator<Item = E>,
        I::IntoIter: DoubleEndedIterator,
    {
        assert!(self.time <= time);
        
        if let Some(bucket) = radix_dist(self.time, time).checked_sub(1) {
            // iterate through the events backwards.
            for event in events.into_iter().rev() {
                // radix_dist always returns a value in 0..=32
                self.buckets[bucket as usize].push(time, event);
                
                // !!! This is done here to make sure the queue is in a consistent state
                //     in case the iterator panics
                
                // if the event was not added to the first bucket,
                // set the filled bit for that bucket.
                self.filled |= 1 << bucket;
            }
        } else {
            for event in events.into_iter().rev() {
                self.head.push(event);
            }
        }
    }

    /// Pop an `event` from the event queue.
    pub fn pop(&mut self) -> Option<(u32, E)> {
        if self.is_empty() {
            return None;
        }
        // if bucket 0 does not have any elements,
        // reassign the elements in the queue, then try again.
        // if it still doesn't, the queue is empty.
        let out = match self.head.pop() {
            None => {
                self.reassign();
                self.head.pop()
            }
            v => v,
        };
        out.map(|v| (self.time, v))
    }

    /// Drains all `event`s happening at the current time from the queue.
    /// If the queue is empty, the returned time will be the current time of the queue.
    pub fn drain_top(&mut self) -> (u32, DrainTop<'_, E>) {
        // if the first bucket is empty,
        // reassign all of the elements to match
        if self.buckets[0].vec.is_empty() {
            self.reassign();
        };
        let head = &mut self.buckets[0];
        (
            head.min,
            DrainTop {
                inner: head.vec.drain(..),
            },
        )
    }

    /// Returns the [current time] of the event queue.
    /// 
    /// # Examples
    /// ```
    /// # use xivc_core::world::queue::RadixEventQueue;
    /// let mut queue = RadixEventQueue::new();
    /// queue.push(10, ());
    /// queue.push(5, ());
    /// 
    /// assert_eq!(queue.time(), 0);
    /// 
    /// queue.pop();
    /// assert_eq!(queue.time(), 5);
    /// 
    /// queue.pop();
    /// assert_eq!(queue.time(), 10);
    /// ```
    ///
    /// [current time]: crate::world::queue#current
    pub fn time(&self) -> u32 {
        self.time
    }

    /// Returns `true` if the event queue is empty.
    /// 
    /// # Examples
    /// ```
    /// # use xivc_core::world::queue::RadixEventQueue;
    /// let mut queue = RadixEventQueue::new();
    /// 
    /// assert!(queue.is_empty());
    /// 
    /// queue.push(3, "A");
    /// assert!(!queue.is_empty());
    /// 
    /// queue.pop();
    /// assert!(queue.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.head.is_empty() && self.filled == 0
    }
    
    // Advances the time to the next present time
    // and reassigns events accordingly
    fn reassign(&mut self) {
        // if none of the buckets are filled, return from the function
        if self.filled == 0 {
            return;
        }

        let next_index = self.filled.trailing_zeros();
        // next_index must be 0..=31

        let (start, end) = self.buckets.split_at_mut(next_index as usize);
        // next_index was a valid index, therefore end must contain it
        // so this will not panic
        let next = &mut end[0];

        // get the minimum value from the bucket
        let min = next.min;
        // reset the minimum value
        next.min = u32::MAX;
        // drain the next non-empty bucket and unset the filled bit
        let drain = next.vec.drain(..);
        self.filled ^= 1 << next_index;

        // redistribute each element in the bucket
        for (time, event) in drain {
            if let Some(bucket) = radix_dist(min, time).checked_sub(1) {
                start[bucket as usize].push(time, event);
                self.filled |= 1 << bucket;
            } else {
                self.head.push(event);
            }
        }
        // set the time to the time of the elements in bucket 0
        self.time = min;
    }
}

/// An iterator for [`RadixEventQueue<E>`] that drains all of the events
/// with a time matching the current smallest time in the queue.
///
/// This `struct` is created by [`RadixEventQueue::drain_top`]. See its documentation for more.
pub struct DrainTop<'a, E> {
    inner: alloc::vec::Drain<'a, (u32, E)>,
}

impl<'a, E> Iterator for DrainTop<'a, E> {
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, e) = self.inner.next_back()?;
        Some(e)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, E> ExactSizeIterator for DrainTop<'a, E> {}
impl<'a, E> FusedIterator for DrainTop<'a, E> {}

impl<E> Default for RadixEventQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}

// used to initialize the heap allocated array of buckets
fn new_buckets<E>() -> Box<Buckets<E>> {
    let layout = Layout::new::<Buckets<E>>();
    assert!(mem::size_of::<Buckets<E>>() != 0);
    // Safety: layout should never have a size of zero
    let p = unsafe { alloc::alloc::alloc(layout) }.cast::<Bucket<E>>();
    if p.is_null() {
        alloc::alloc::handle_alloc_error(layout);
    }
    for x in 0..32 {
        // Safety
        // will stay inside the allocation
        // because an array of size 33 was allocated and this will never go past 32
        unsafe { p.add(x).write(Bucket::new()) };
    }
    // Safety
    // p is fully initialized from the above loop
    // the pointer was allocated with the same layout in the global allocator
    unsafe { Box::from_raw(p.cast::<Buckets<E>>()) }
}

// returns a value from 0 to 32
const fn radix_dist(lhs: u32, rhs: u32) -> u32 {
    let radix_sim = (lhs ^ rhs).leading_zeros();
    // should never happen
    if radix_sim > 32 {
        unreachable!();
    }
    32 - radix_sim
}
