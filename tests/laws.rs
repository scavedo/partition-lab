//! Property-based tests: the merge laws, convergence, and "no split brain".

use partition_lab::{ApReplica, Lab, Merge, Mode, N};
use proptest::prelude::*;

fn merged(a: &ApReplica, b: &ApReplica) -> ApReplica {
    let mut out = a.clone();
    out.merge(b);
    out
}

#[derive(Clone, Debug)]
enum Op {
    Sell { dev: usize, up: bool },
    Note { dev: usize, text: u8 },
    Sync { from: usize, to: usize },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..N, any::<bool>()).prop_map(|(dev, up)| Op::Sell { dev, up }),
        (0..N, 0u8..4).prop_map(|(dev, text)| Op::Note { dev, text }),
        (0..N, 0..N).prop_map(|(from, to)| Op::Sync { from, to }),
    ]
}

/// Three replicas after a random history of local writes and partial syncs.
fn replicas(ops: &[Op]) -> [ApReplica; N] {
    let mut r: [ApReplica; N] = Default::default();
    for op in ops {
        match *op {
            Op::Sell { dev, up } => r[dev].sell(dev, if up { 1 } else { -1 }),
            Op::Note { dev, text } => {
                r[dev].set_note(dev, &format!("note {text}"));
            }
            Op::Sync { from, to } => {
                let s = r[from].clone();
                r[to].merge(&s);
            }
        }
    }
    r
}

#[derive(Clone, Debug)]
enum Action {
    Sell(usize, bool),
    Note(usize, u8),
    Toggle(usize, usize),
    Isolate(usize),
    Heal,
    Take(usize),
    Release(usize),
    Expire(usize),
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => (0..N, any::<bool>()).prop_map(|(d, up)| Action::Sell(d, up)),
        2 => (0..N, 0u8..4).prop_map(|(d, t)| Action::Note(d, t)),
        2 => (0..N, 0..N).prop_filter("distinct", |(a, b)| a != b).prop_map(|(a, b)| Action::Toggle(a, b)),
        1 => (0..N).prop_map(Action::Isolate),
        1 => Just(Action::Heal),
        1 => (0..N).prop_map(Action::Take),
        1 => (0..N).prop_map(Action::Release),
        1 => (0..N).prop_map(Action::Expire),
    ]
}

fn run(lab: &mut Lab, a: &Action) {
    match *a {
        Action::Sell(d, up) => drop(lab.sell(d, if up { 1 } else { -1 })),
        Action::Note(d, t) => drop(lab.set_note(d, &format!("note {t}"))),
        Action::Toggle(x, y) => drop(lab.toggle_link(x, y)),
        Action::Isolate(d) => drop(lab.isolate(d)),
        Action::Heal => drop(lab.heal()),
        Action::Take(d) => drop(lab.take_lock(d)),
        Action::Release(d) => drop(lab.release_lock(d)),
        Action::Expire(d) => drop(lab.expire_lease(d)),
    }
}

proptest! {
    #[test]
    fn merge_is_commutative(ops in prop::collection::vec(op(), 0..40)) {
        let [a, b, _] = replicas(&ops);
        prop_assert_eq!(merged(&a, &b), merged(&b, &a));
    }

    #[test]
    fn merge_is_associative(ops in prop::collection::vec(op(), 0..40)) {
        let [a, b, c] = replicas(&ops);
        prop_assert_eq!(merged(&merged(&a, &b), &c), merged(&a, &merged(&b, &c)));
    }

    #[test]
    fn merge_is_idempotent(ops in prop::collection::vec(op(), 0..40)) {
        let [a, ..] = replicas(&ops);
        prop_assert_eq!(merged(&a, &a), a);
    }

    /// Whatever happens during a split, healing the network makes every copy
    /// identical, and the counter equals the sales actually made.
    #[test]
    fn availability_mode_converges_and_loses_no_sales(actions in prop::collection::vec(action(), 0..60)) {
        let mut lab = Lab::new(Mode::Availability);
        let mut expected = lab.ap[0].sold.value();
        for a in &actions {
            if let Action::Sell(_, up) = a {
                expected += if *up { 1 } else { -1 };
            }
            run(&mut lab, a);
        }
        lab.heal();
        prop_assert!(lab.ap.iter().all(|r| r == &lab.ap[0]));
        prop_assert_eq!(lab.ap[0].sold.value(), expected);
    }

    /// No split brain: two copies at the same version are always identical,
    /// and healing brings every copy to the same version.
    #[test]
    fn consistency_mode_never_diverges(actions in prop::collection::vec(action(), 0..60)) {
        let mut lab = Lab::new(Mode::Consistency);
        for a in &actions {
            run(&mut lab, a);
            for x in 0..N {
                for y in 0..N {
                    if lab.cp[x].version == lab.cp[y].version {
                        prop_assert_eq!(&lab.cp[x], &lab.cp[y]);
                    }
                }
            }
        }
        lab.heal();
        prop_assert!(lab.cp.iter().all(|r| r == &lab.cp[0]));
    }
}
