//! The two CRDTs the lab uses in availability mode.
//!
//! A state-based CRDT has a `merge` that is commutative, associative and
//! idempotent. If every replica eventually merges every other replica's
//! state, in any order and any number of times, they all end up equal.
//! `tests/laws.rs` checks those properties with proptest.

use crate::N;
use serde::Serialize;

/// A state-based CRDT.
pub trait Merge {
    /// Fold `other`'s state into `self`.
    fn merge(&mut self, other: &Self);
}

/// A Lamport timestamp: `(counter, device)`.
///
/// Ordered by counter first, then by device id, so any two stamps compare
/// the same way on every replica. That is what lets concurrent writes pick
/// the same winner everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Stamp {
    pub counter: u64,
    pub device: usize,
}

/// Last-write-wins register: keeps the value with the highest stamp.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LwwRegister<T> {
    pub value: T,
    /// `None` until the first write. `None` sorts below every stamp.
    pub stamp: Option<Stamp>,
}

impl<T> LwwRegister<T> {
    pub fn new(value: T) -> Self {
        Self { value, stamp: None }
    }

    /// Write a value. The caller supplies a stamp newer than any it has seen.
    pub fn set(&mut self, value: T, stamp: Stamp) {
        self.value = value;
        self.stamp = Some(stamp);
    }
}

impl<T: Clone> Merge for LwwRegister<T> {
    fn merge(&mut self, other: &Self) {
        if other.stamp > self.stamp {
            self.value = other.value.clone();
            self.stamp = other.stamp;
        }
    }
}

/// Increment/decrement counter: one increment slot and one decrement slot per
/// device. A device only ever raises its own slots, and merge takes the
/// maximum of each slot, so no update is ever counted twice or lost.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PnCounter {
    pub inc: [u64; N],
    pub dec: [u64; N],
}

impl PnCounter {
    pub fn increment(&mut self, device: usize, by: u64) {
        self.inc[device] += by;
    }

    pub fn decrement(&mut self, device: usize, by: u64) {
        self.dec[device] += by;
    }

    pub fn value(&self) -> i64 {
        let inc: u64 = self.inc.iter().sum();
        let dec: u64 = self.dec.iter().sum();
        inc as i64 - dec as i64
    }
}

impl Merge for PnCounter {
    fn merge(&mut self, other: &Self) {
        for d in 0..N {
            self.inc[d] = self.inc[d].max(other.inc[d]);
            self.dec[d] = self.dec[d].max(other.dec[d]);
        }
    }
}
