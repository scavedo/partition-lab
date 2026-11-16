//! Everything the UI needs to draw the current state, as one serializable
//! snapshot. The page never reads the replicas directly.

use crate::consistency::is_majority;
use crate::crdt::Stamp;
use crate::{Lab, Mode, N};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub mode: Mode,
    pub links: [bool; 3],
    pub groups: Vec<Vec<usize>>,
    pub copies_agree: bool,
    pub devices: Vec<DeviceView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind")]
pub enum DeviceView {
    #[serde(rename = "AP")]
    Ap {
        peers: Vec<usize>,
        note: String,
        stamp: Option<Stamp>,
        sold: i64,
        inc: [u64; N],
        dec: [u64; N],
        naive: i64,
    },
    #[serde(rename = "CP")]
    Cp {
        peers: Vec<usize>,
        majority: bool,
        note: String,
        version: u64,
        sold: i64,
        lock: Option<usize>,
        lock_reachable: bool,
    },
}

impl Lab {
    /// Everything the UI needs to draw the current state.
    pub fn view(&self) -> View {
        let groups = self.net.groups();
        let agree = match self.mode {
            Mode::Availability => self.ap.iter().all(|r| r.visible() == self.ap[0].visible()),
            Mode::Consistency => self.cp.iter().all(|r| r.version == self.cp[0].version),
        };
        let devices = (0..N)
            .map(|i| {
                let group = groups.iter().find(|g| g.contains(&i)).expect("in a group");
                let peers = group.iter().copied().filter(|&n| n != i).collect();
                match self.mode {
                    Mode::Availability => {
                        let r = &self.ap[i];
                        DeviceView::Ap {
                            peers,
                            note: r.note.value.clone(),
                            stamp: r.note.stamp,
                            sold: r.sold.value(),
                            inc: r.sold.inc,
                            dec: r.sold.dec,
                            naive: r.naive.value,
                        }
                    }
                    Mode::Consistency => {
                        let r = &self.cp[i];
                        DeviceView::Cp {
                            peers,
                            majority: is_majority(group),
                            note: r.note.clone(),
                            version: r.version,
                            sold: r.sold,
                            lock: r.lock.holder,
                            lock_reachable: r.lock.holder.is_none_or(|h| group.contains(&h)),
                        }
                    }
                }
            })
            .collect();
        View {
            mode: self.mode,
            links: self.net.links,
            groups,
            copies_agree: agree,
            devices,
        }
    }
}
