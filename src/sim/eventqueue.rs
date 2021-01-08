use std::mem;

// Event queue implemented as a radix heap.
// The time key is a u32 to preserve space, and if time goes over 6 years you have a real problem
// (and can just fork the project xd)
#[derive(Clone, Debug)]
pub struct EventQueue<E> {
    len: usize,
    time: u32,
    buckets: [Bucket<E>; 32],
}

#[derive(Clone, Debug)]
struct Bucket<E> {
    min: u32,
    items: Vec<(u32, E)>,
}

impl<E> EventQueue<E> {
    pub fn new() -> Self {
        EventQueue {
            len: 0,
            time: 0,
            buckets: <[Bucket<E>;32] as Default>::default(),
        }
    }

    pub fn push(&mut self, time: u32, event: E) {
        assert!(time >= self.time);
        self.place_ev(time, event);
        self.len += 1;
    }

    fn place_ev(&mut self, time: u32, event: E) {
        self.buckets[32 - (time ^ self.time).leading_zeros() as usize].push(time, event);
    }

    pub fn pop(&mut self) -> Option<(u32, E)> {
        let index = self.buckets.iter().enumerate().find(|(_, v)| !v.items.is_empty()).map(|(i, _)| i)?;
        let mut place = mem::replace(&mut self.buckets[index], Bucket::new());
        let min = place.min;
        self.time = min;
        for (t, e) in place.items.drain(..) {
            self.place_ev(t, e);
        }
        self.buckets[0].items.pop()
    }
}

impl<E> Bucket<E> {
    pub const fn new() -> Self {
        Bucket {
            min: u32::MAX,
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, t: u32, ev: E) {
        self.min = self.min.min(t);
        self.items.push((t, ev));
    }
}

// Only used to create the array x-x
impl<E> Default for Bucket<E> {
    fn default() -> Self {
        Self::new()
    }
}
