//! Availability mode: every device writes locally and merges with CRDTs.
//!
//! A device never waits for anyone, so it stays writable while cut off. When
//! devices can reach each other they exchange whole states and merge them.
//! Each merge is commutative, associative and idempotent (see `crdt.rs`), so
//! all copies end up equal whatever order the merges happen in.

use crate::crdt::{LwwRegister, Merge, PnCounter, Stamp};
use crate::network::{EDGES, Network};
use crate::outcome::Outcome;
use crate::{N, join, name};
use serde::Serialize;

/// One device's copy of the record in availability mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ApReplica {
    /// Lamport clock: one more than the highest counter this device has seen.
    pub clock: u64,
    /// The cabin note: last write wins, by Lamport stamp.
    pub note: LwwRegister<String>,
    /// Items sold, as a PN-counter: no sale is ever lost.
    pub sold: PnCounter,
    /// The same sales kept as one plain number with last write wins.
    /// Shown next to the PN-counter to make lost updates visible.
    pub naive: LwwRegister<i64>,
}

impl Default for ApReplica {
    fn default() -> Self {
        Self {
            clock: 0,
            note: LwwRegister::new(String::new()),
            sold: PnCounter::default(),
            naive: LwwRegister::new(0),
        }
    }
}

impl ApReplica {
    /// Advance the Lamport clock past everything seen, including `seen`.
    fn tick(&mut self, me: usize, seen: Option<Stamp>) -> Stamp {
        let seen = seen.map_or(0, |s| s.counter);
        self.clock = self.clock.max(seen) + 1;
        Stamp {
            counter: self.clock,
            device: me,
        }
    }

    /// Record one sale (`delta = 1`) or refund (`delta = -1`) made on device `me`.
    pub fn sell(&mut self, me: usize, delta: i64) {
        if delta >= 0 {
            self.sold.increment(me, 1);
        } else {
            self.sold.decrement(me, 1);
        }
        let stamp = self.tick(me, self.naive.stamp);
        let value = self.naive.value + delta.signum();
        self.naive.set(value, stamp);
    }

    /// Write a new note from device `me`. Returns the stamp it was given.
    pub fn set_note(&mut self, me: usize, text: &str) -> Stamp {
        let stamp = self.tick(me, self.note.stamp);
        self.note.set(text.to_string(), stamp);
        stamp
    }

    /// The parts of the state that users see; the clock is excluded.
    pub(crate) fn visible(&self) -> (&LwwRegister<String>, &PnCounter, &LwwRegister<i64>) {
        (&self.note, &self.sold, &self.naive)
    }
}

impl Merge for ApReplica {
    fn merge(&mut self, other: &Self) {
        self.clock = self.clock.max(other.clock);
        self.note.merge(&other.note);
        self.sold.merge(&other.sold);
        self.naive.merge(&other.naive);
    }
}

/// Merge the states of every device in `group`, which can all reach each
/// other, and give each of them the result.
pub(crate) fn merge_group(
    replicas: &mut [ApReplica; N],
    net: &Network,
    group: &[usize],
    out: &mut Outcome,
) {
    let mut merged = replicas[group[0]].clone();
    for &n in &group[1..] {
        merged.merge(&replicas[n]);
    }
    let changed: Vec<usize> = group
        .iter()
        .copied()
        .filter(|&n| replicas[n].visible() != merged.visible())
        .collect();
    for &n in group {
        replicas[n] = merged.clone();
    }
    if changed.is_empty() {
        return;
    }
    for &(a, b) in &EDGES {
        if group.contains(&a) && group.contains(&b) && net.is_up(a, b) {
            out.transfers.push([a, b]);
            out.transfers.push([b, a]);
        }
    }
    out.sync(format!(
        "{} exchanged state and merged. {} updated.",
        group
            .iter()
            .map(|&n| name(n))
            .collect::<Vec<_>>()
            .join(" and "),
        join(&changed)
    ));
}
