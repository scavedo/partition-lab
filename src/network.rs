//! The network: which links between the three devices are up, and which
//! devices can reach each other as a result.

use crate::N;
use serde::Serialize;

/// The three links, as (device, device). Index into [`Network::links`].
pub const EDGES: [(usize, usize); 3] = [(0, 1), (1, 2), (0, 2)];

/// Which links are up.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Network {
    pub links: [bool; 3],
}

impl Default for Network {
    fn default() -> Self {
        Self { links: [true; 3] }
    }
}

impl Network {
    fn edge_index(a: usize, b: usize) -> usize {
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        EDGES
            .iter()
            .position(|&e| e == (a, b))
            .expect("devices must be distinct and in range")
    }

    /// True if devices `a` and `b` have a direct, working link.
    pub fn is_up(&self, a: usize, b: usize) -> bool {
        a != b && self.links[Self::edge_index(a, b)]
    }

    /// Flip one link. Returns the new state of the link.
    pub fn toggle(&mut self, a: usize, b: usize) -> bool {
        let e = Self::edge_index(a, b);
        self.links[e] = !self.links[e];
        self.links[e]
    }

    /// Cut every link to device `i`. Other links are left as they are, so
    /// isolating each device in turn cuts the network into three pieces.
    pub fn isolate(&mut self, i: usize) {
        for (k, &(a, b)) in EDGES.iter().enumerate() {
            if a == i || b == i {
                self.links[k] = false;
            }
        }
    }

    pub fn heal(&mut self) {
        self.links = [true; 3];
    }

    /// Groups of devices that can reach each other, directly or through a third.
    pub fn groups(&self) -> Vec<Vec<usize>> {
        let mut seen = [false; N];
        let mut out = Vec::new();
        for start in 0..N {
            if seen[start] {
                continue;
            }
            let mut group = Vec::new();
            let mut queue = vec![start];
            seen[start] = true;
            while let Some(n) = queue.pop() {
                group.push(n);
                for (m, visited) in seen.iter_mut().enumerate() {
                    if !*visited && self.is_up(n, m) {
                        *visited = true;
                        queue.push(m);
                    }
                }
            }
            group.sort_unstable();
            out.push(group);
        }
        out
    }

    pub fn group_of(&self, i: usize) -> Vec<usize> {
        self.groups()
            .into_iter()
            .find(|g| g.contains(&i))
            .expect("every device is in a group")
    }

    /// The route a message takes from `a` to `b`, as hops, if one exists.
    pub(crate) fn route(&self, a: usize, b: usize) -> Vec<[usize; 2]> {
        if self.is_up(a, b) {
            return vec![[a, b]];
        }
        (0..N)
            .find(|&m| m != a && m != b && self.is_up(a, m) && self.is_up(m, b))
            .map(|m| vec![[a, m], [m, b]])
            .unwrap_or_default()
    }
}
