//! Consistency mode: majority writes.
//!
//! A write commits only if the writing device can reach a majority of the
//! devices (2 of 3). Two groups can't both be a majority, so at most one side
//! of a partition can write. Copies never conflict and never need merging: a
//! device that was cut off just adopts the newest version when it reconnects.
//!
//! This stands in for a real consensus protocol with the same majority rule,
//! over a known, fixed set of three devices.

use crate::lock::EditLock;
use crate::network::Network;
use crate::outcome::Outcome;
use crate::{N, join, name};
use serde::Serialize;

/// One device's copy of the record in consistency mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CpReplica {
    /// Increases by one on every committed write.
    pub version: u64,
    pub note: String,
    pub sold: i64,
    /// The edit lock for the note, replicated like everything else.
    pub lock: EditLock,
}

/// True if `group` holds more than half of all devices.
pub fn is_majority(group: &[usize]) -> bool {
    group.len() * 2 > N
}

/// The device in `group` with the newest version.
fn newest(replicas: &[CpReplica; N], group: &[usize]) -> usize {
    *group
        .iter()
        .max_by_key(|&&n| replicas[n].version)
        .expect("group is not empty")
}

/// Apply `change` if device `i` can reach a majority, then copy the result to
/// every device it can reach. `change` can refuse the write with a reason.
pub(crate) fn write(
    replicas: &mut [CpReplica; N],
    net: &Network,
    i: usize,
    label: &str,
    change: impl FnOnce(&mut CpReplica) -> Result<(), String>,
) -> Outcome {
    let group = net.group_of(i);
    if !is_majority(&group) {
        return Outcome::default().refuse(
            "Refused: can’t reach a majority (needs 2 of 3).",
            format!("{} refused a write: it can’t reach a majority.", name(i)),
        );
    }
    let mut next = replicas[newest(replicas, &group)].clone();
    if let Err(why) = change(&mut next) {
        return Outcome::default().refuse(why.clone(), format!("{}: {why}", name(i)));
    }
    next.version += 1;
    let mut out = Outcome::ok(format!("Committed as version {}.", next.version));
    for &n in &group {
        if n != i {
            out.transfers.extend(net.route(i, n));
        }
        replicas[n] = next.clone();
    }
    out.info(format!(
        "{label} Committed as version {} on {}.",
        next.version,
        join(&group)
    ));
    out
}

/// Bring every device in `group` up to the newest version in the group.
pub(crate) fn catch_up(
    replicas: &mut [CpReplica; N],
    net: &Network,
    group: &[usize],
    out: &mut Outcome,
) {
    let newest = newest(replicas, group);
    let best = replicas[newest].clone();
    let behind: Vec<usize> = group
        .iter()
        .copied()
        .filter(|&n| replicas[n].version < best.version)
        .collect();
    for &n in &behind {
        out.transfers.extend(net.route(newest, n));
        replicas[n] = best.clone();
    }
    if !behind.is_empty() {
        out.sync(format!(
            "{} caught up to version {}.",
            join(&behind),
            best.version
        ));
    }
}
