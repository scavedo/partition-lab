//! Partition Lab: a small model of what happens to replicated data when the
//! network splits.
//!
//! Three devices share one record (a note and a count of items sold). The
//! [`Lab`] runs in one of two modes:
//!
//! * **Availability (AP)**: every device always accepts writes. Devices that
//!   can reach each other exchange their full state and merge it with CRDT
//!   rules, so all copies converge once the network heals.
//! * **Consistency (CP)**: a write must reach a majority (2 of 3). A cut-off
//!   device refuses writes, so the copies can never conflict. Editing the note
//!   also needs an edit lock (a mutex) kept in the same majority-replicated state.
//!
//! Each idea lives in its own module:
//!
//! * [`network`]: links, and which devices can reach each other.
//! * [`crdt`]: the last-write-wins register and the PN-counter.
//! * [`availability`]: AP replicas and merging them with the CRDTs.
//! * [`consistency`]: CP replicas and majority writes.
//! * [`lock`]: the edit lock (a mutex with a lease).
//! * [`outcome`] and [`view`]: what goes back to the page.
//!
//! [`Lab`] ties them together: it holds the state and sends each action to
//! the right module for the current mode.
//!
//! The model is deliberately small: sync is instant and whole-state, and there
//! is no real network.

use serde::Serialize;

pub mod availability;
pub mod consistency;
pub mod crdt;
pub mod lock;
pub mod network;
pub mod outcome;
pub mod view;
pub mod wasm;

pub use availability::ApReplica;
pub use consistency::CpReplica;
pub use crdt::{LwwRegister, Merge, PnCounter, Stamp};
pub use lock::EditLock;
pub use network::{EDGES, Network};
pub use outcome::{Event, EventKind, Outcome};
pub use view::{DeviceView, View};

/// Number of devices in the lab.
pub const N: usize = 3;
/// Display names, indexed by device id.
pub const NAMES: [&str; N] = ["A", "B", "C"];

pub(crate) fn name(i: usize) -> &'static str {
    NAMES[i]
}

pub(crate) fn join(ids: &[usize]) -> String {
    ids.iter().map(|&i| name(i)).collect::<Vec<_>>().join(", ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Mode {
    #[serde(rename = "AP")]
    Availability,
    #[serde(rename = "CP")]
    Consistency,
}

#[derive(Clone, Debug)]
pub struct Lab {
    pub mode: Mode,
    pub net: Network,
    pub ap: [ApReplica; N],
    pub cp: [CpReplica; N],
}

impl Lab {
    /// A lab in the given mode, starting with a short, already-synced history:
    /// 6 items sold and one note.
    pub fn new(mode: Mode) -> Self {
        let mut lab = Self {
            mode,
            net: Network::default(),
            ap: Default::default(),
            cp: std::array::from_fn(|_| CpReplica {
                version: 7,
                note: "Cart 2: coffee running low".into(),
                sold: 6,
                lock: EditLock::default(),
            }),
        };
        if mode == Mode::Availability {
            for &dev in &[0, 0, 1, 0, 2, 1] {
                lab.ap[dev].sell(dev, 1);
                lab.sync();
            }
            lab.ap[1].set_note(1, "Cart 2: coffee running low");
            lab.sync();
        }
        lab
    }

    /// Exchange state inside every connected group. Returns what moved.
    pub fn sync(&mut self) -> Outcome {
        let mut out = Outcome {
            ok: true,
            ..Outcome::default()
        };
        for group in self.net.groups() {
            if group.len() < 2 {
                continue;
            }
            match self.mode {
                Mode::Availability => {
                    availability::merge_group(&mut self.ap, &self.net, &group, &mut out)
                }
                Mode::Consistency => {
                    consistency::catch_up(&mut self.cp, &self.net, &group, &mut out)
                }
            }
        }
        out
    }

    fn after(&mut self, mut out: Outcome) -> Outcome {
        let synced = self.sync();
        out.events.extend(synced.events);
        out.transfers.extend(synced.transfers);
        out
    }

    /// A majority write in consistency mode; see [`consistency::write`].
    fn cp_write(
        &mut self,
        i: usize,
        label: &str,
        change: impl FnOnce(&mut CpReplica) -> Result<(), String>,
    ) -> Outcome {
        consistency::write(&mut self.cp, &self.net, i, label, change)
    }

    // ----- network actions -----

    pub fn toggle_link(&mut self, a: usize, b: usize) -> Outcome {
        let now_up = self.net.toggle(a, b);
        let mut out = Outcome {
            ok: true,
            ..Outcome::default()
        };
        out.info(format!(
            "Link {}–{} {}.",
            name(a),
            name(b),
            if now_up { "restored" } else { "cut" }
        ));
        self.after(out)
    }

    pub fn isolate(&mut self, i: usize) -> Outcome {
        self.net.isolate(i);
        let mut out = Outcome {
            ok: true,
            ..Outcome::default()
        };
        out.info(format!("{} is isolated from the others.", name(i)));
        self.after(out)
    }

    pub fn heal(&mut self) -> Outcome {
        self.net.heal();
        let mut out = Outcome {
            ok: true,
            ..Outcome::default()
        };
        out.info("All links restored.");
        self.after(out)
    }

    // ----- data actions -----

    /// Sell one item (`delta >= 0`) or refund one (`delta < 0`) on device `i`.
    pub fn sell(&mut self, i: usize, delta: i64) -> Outcome {
        let verb = if delta >= 0 { "sold" } else { "refunded" };
        match self.mode {
            Mode::Availability => {
                self.ap[i].sell(i, delta);
                let mut out = Outcome::ok(if delta >= 0 {
                    "Sold locally."
                } else {
                    "Refunded locally."
                });
                out.info(format!("{} {verb} 1 item locally.", name(i)));
                self.after(out)
            }
            Mode::Consistency => self.cp_write(i, &format!("{} {verb} 1 item.", name(i)), |s| {
                s.sold += delta.signum();
                Ok(())
            }),
        }
    }

    pub fn set_note(&mut self, i: usize, text: &str) -> Outcome {
        let text = text.trim();
        if text.is_empty() {
            return Outcome::default().refuse(
                "Type a note first.",
                format!("{}: empty note ignored.", name(i)),
            );
        }
        match self.mode {
            Mode::Availability => {
                let stamp = self.ap[i].set_note(i, text);
                let mut out = Outcome::ok("Saved locally.");
                out.info(format!(
                    "{} saved the note “{text}” with stamp ({}, {}).",
                    name(i),
                    stamp.counter,
                    name(stamp.device)
                ));
                self.after(out)
            }
            Mode::Consistency => {
                let owned = text.to_string();
                self.cp_write(
                    i,
                    &format!("{} saved the note “{text}”.", name(i)),
                    move |s| {
                        s.lock.check_held_by(i)?;
                        s.note = owned;
                        Ok(())
                    },
                )
            }
        }
    }

    // ----- lock actions (consistency mode only) -----

    pub fn take_lock(&mut self, i: usize) -> Outcome {
        if self.mode == Mode::Availability {
            return no_locks(i);
        }
        self.cp_write(i, &format!("{} took the edit lock.", name(i)), |s| {
            s.lock.acquire(i)
        })
    }

    pub fn release_lock(&mut self, i: usize) -> Outcome {
        if self.mode == Mode::Availability {
            return no_locks(i);
        }
        self.cp_write(i, &format!("{} released the edit lock.", name(i)), |s| {
            s.lock.release(i)
        })
    }

    /// Clear a lock whose holder can't be reached, as a lease timeout would.
    pub fn expire_lease(&mut self, i: usize) -> Outcome {
        if self.mode == Mode::Availability {
            return no_locks(i);
        }
        let reachable = self.net.group_of(i);
        self.cp_write(
            i,
            &format!(
                "{} expired the lease of an unreachable lock holder.",
                name(i)
            ),
            move |s| s.lock.expire(&reachable),
        )
    }
}

fn no_locks(i: usize) -> Outcome {
    Outcome::default().refuse(
        "Availability mode has no locks: edits merge instead.",
        format!("{}: no locks in availability mode.", name(i)),
    )
}
