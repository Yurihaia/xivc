use alloc::{alloc::Layout, boxed::Box, vec::Vec};
use core::{iter::FusedIterator, mem};

pub struct EventQueue<E> {
    time: u32,
    // this is stored on the heap to make this struct far cheaper to move around
    buckets: Box<Buckets<E>>,
    // a bitmap of the buckets from 1..=32 that are not empty
    // bit 0 = 1 means buckets[1] is not empty, etc.
    // therefore, the first non-empty bucket will have the index filled.trailing_zeros() + 1
    filled: u32,
}

struct Bucket<E> {
    min: u32,
    vec: Vec<(u32, E)>,
}

type Buckets<E> = [Bucket<E>; 33];

impl<E> Bucket<E> {
    fn new() -> Self {
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

impl<E> EventQueue<E> {
    /// Creates a new event queue.
    pub fn new() -> Self {
        Self {
            time: 0,
            buckets: new_buckets(),
            filled: 0,
        }
    }

    /// Push an `event` to the event queue, to happen at the specified `time`.
    /// Panics if the time is less than the current time.
    pub fn push(&mut self, time: u32, event: E) {
        assert!(self.time <= time);
        let bucket = radix_dist(self.time, time) as usize;
        // radix_dist always returns a value in 0..=32
        debug_assert!(bucket < 33);
        unsafe { self.buckets.get_unchecked_mut(bucket) }.push(time, event);

        // if the event was not added to the first bucket,
        // set the filled bit for that bucket.
        if let Some(nzb) = bucket.checked_sub(1) {
            self.filled |= 1 << nzb;
        }
    }

    /// Pop an `event` from the event queue.
    pub fn pop(&mut self) -> Option<(u32, E)> {
        // get a value from bucket 0
        let bucket = &mut self.buckets[0];
        // if bucket 0 does not have any elements,
        // reassign the elements in the queue, then try again.
        // if it still doesn't, the queue is empty.
        match bucket.vec.pop() {
            None => {
                self.reassign();
                self.buckets[0].vec.pop()
            }
            v => v,
        }
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

    /// Returns the time of the event queue.
    pub fn time(&self) -> u32 {
        self.time
    }

    fn reassign(&mut self) {
        // if none of the buckets are filled, return from the function
        if self.filled == 0 {
            return;
        }

        let next_index = self.filled.trailing_zeros() + 1;
        // now, next_index must be 1..=33

        // reset the min value for the head bucket
        self.buckets[0].min = u32::MAX;

        // get the
        let (start, end) = self.buckets.split_at_mut(next_index as usize);
        // Safety: next_index was a valid index, therefore end must contain it
        let next = unsafe { end.get_unchecked_mut(0) };

        // get the minimum value from the bucket
        let min = next.min;
        // reset the minimum value
        next.min = u32::MAX;
        // drain the next non-empty bucket and unset the filled bit
        let drain = next.vec.drain(..);
        self.filled ^= 1 << (next_index - 1);

        // redistribute each element in the bucket
        for (time, event) in drain {
            let bucket = radix_dist(min, time) as usize;
            // cannot be unchecked because of potential logic errors
            // this will panic when the heap is in an invalid state
            start[bucket].push(time, event);

            if let Some(nzb) = bucket.checked_sub(1) {
                self.filled |= 1 << nzb;
            }
        }
        // set the time to the time of the elements in bucket 0
        self.time = min;
    }
}

pub struct DrainTop<'a, E> {
    inner: alloc::vec::Drain<'a, (u32, E)>,
}

impl<'a, E> Iterator for DrainTop<'a, E> {
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, e) = self.inner.next()?;
        Some(e)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, E> ExactSizeIterator for DrainTop<'a, E> {}
impl<'a, E> FusedIterator for DrainTop<'a, E> {}

impl<E> Default for EventQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}

fn new_buckets<E>() -> Box<Buckets<E>> {
    let layout = Layout::new::<Buckets<E>>();
    assert!(mem::size_of::<Buckets<E>>() != 0);
    // Safety: layout should never have a size of zero
    let p = unsafe { alloc::alloc::alloc(layout) }.cast::<Bucket<E>>();
    if p.is_null() {
        alloc::alloc::handle_alloc_error(layout);
    }
    for x in 0..33 {
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

const fn radix_dist(lhs: u32, rhs: u32) -> u32 {
    32 - (lhs ^ rhs).leading_zeros()
}
