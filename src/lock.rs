//! The edit lock: a mutex for the cabin note, used in consistency mode.
//!
//! On one machine a mutex lives in shared memory. Here it lives in the
//! replicated record, so taking or releasing it is an ordinary majority write
//! (see `consistency.rs`). That keeps it safe: two devices can never both hold
//! it, because only one side of a partition can have a majority.
//!
//! It also makes it fragile. A holder that gets cut off keeps the lock but
//! can't write, and nobody else can take it. The way out is a lease: once the
//! holder has been unreachable long enough, the majority clears the lock. The
//! lab has no timer, so you expire the lease by hand.

use crate::name;
use serde::Serialize;

/// Which device holds the lock, if any. Serializes as just the holder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct EditLock {
    pub holder: Option<usize>,
}

impl EditLock {
    /// Device `me` takes the lock. Fails if anyone already holds it.
    pub fn acquire(&mut self, me: usize) -> Result<(), String> {
        match self.holder {
            Some(h) if h == me => Err("You already hold the lock.".into()),
            Some(h) => Err(format!("The lock is held by {}.", name(h))),
            None => {
                self.holder = Some(me);
                Ok(())
            }
        }
    }

    /// Device `me` gives the lock back. Only the holder can release it.
    pub fn release(&mut self, me: usize) -> Result<(), String> {
        if self.holder == Some(me) {
            self.holder = None;
            Ok(())
        } else {
            Err("You don’t hold the lock.".into())
        }
    }

    /// Clear the lock because its lease ran out. Refused while the holder is
    /// still in `reachable`: a reachable holder should be asked to release.
    pub fn expire(&mut self, reachable: &[usize]) -> Result<(), String> {
        match self.holder {
            None => Err("Nobody holds the lock.".into()),
            Some(h) if reachable.contains(&h) => {
                Err(format!("{} is reachable: ask it to release.", name(h)))
            }
            Some(_) => {
                self.holder = None;
                Ok(())
            }
        }
    }

    /// Check that device `me` holds the lock before it edits the note.
    pub fn check_held_by(&self, me: usize) -> Result<(), String> {
        match self.holder {
            Some(h) if h == me => Ok(()),
            Some(h) => Err(format!("The note is locked by {}.", name(h))),
            None => Err("Take the edit lock first.".into()),
        }
    }
}
