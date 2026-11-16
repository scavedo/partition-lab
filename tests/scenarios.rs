//! The scenarios the page invites you to try, as tests.

use partition_lab::{DeviceView, Lab, Mode};

const A: usize = 0;
const C: usize = 2;

fn ap_sold(lab: &Lab) -> Vec<(i64, i64)> {
    lab.view()
        .devices
        .iter()
        .map(|d| match d {
            DeviceView::Ap { sold, naive, .. } => (*sold, *naive),
            _ => unreachable!(),
        })
        .collect()
}

#[test]
fn starts_synced_with_six_sales() {
    let lab = Lab::new(Mode::Availability);
    assert!(lab.view().copies_agree);
    assert_eq!(ap_sold(&lab), vec![(6, 6); 3]);
}

#[test]
fn pn_counter_keeps_concurrent_sales_but_plain_number_loses_one() {
    let mut lab = Lab::new(Mode::Availability);
    lab.isolate(C);
    lab.sell(A, 1);
    lab.sell(A, 1);
    lab.sell(C, 1);
    assert!(!lab.view().copies_agree);
    lab.heal();
    assert!(lab.view().copies_agree);
    // All three sales survive in the counter; the plain number keeps only
    // the write with the highest stamp (A's 8), losing C's sale.
    assert_eq!(ap_sold(&lab), vec![(9, 8); 3]);
}

#[test]
fn concurrent_notes_settle_on_the_same_winner() {
    let mut lab = Lab::new(Mode::Availability);
    lab.isolate(C);
    lab.set_note(A, "from A");
    lab.set_note(C, "from C");
    lab.heal();
    let notes: Vec<String> = lab
        .view()
        .devices
        .iter()
        .map(|d| match d {
            DeviceView::Ap { note, .. } => note.clone(),
            _ => unreachable!(),
        })
        .collect();
    assert!(notes.iter().all(|n| n == &notes[0]));
}

#[test]
fn cut_off_device_stays_writable_in_availability_mode() {
    let mut lab = Lab::new(Mode::Availability);
    lab.isolate(C);
    assert!(lab.sell(C, 1).ok);
    assert!(lab.set_note(C, "still working").ok);
}

#[test]
fn minority_refuses_writes_in_consistency_mode() {
    let mut lab = Lab::new(Mode::Consistency);
    lab.isolate(C);
    let out = lab.sell(C, 1);
    assert!(!out.ok);
    assert!(lab.sell(A, 1).ok, "the majority keeps working");
}

#[test]
fn minority_catches_up_after_heal() {
    let mut lab = Lab::new(Mode::Consistency);
    lab.isolate(C);
    lab.sell(A, 1);
    assert_eq!(lab.cp[C].version, 7);
    lab.heal();
    assert_eq!(lab.cp[C], lab.cp[A]);
    assert_eq!(lab.cp[C].sold, 7);
}

#[test]
fn note_needs_the_lock_in_consistency_mode() {
    let mut lab = Lab::new(Mode::Consistency);
    assert!(!lab.set_note(A, "no lock").ok);
    assert!(lab.take_lock(A).ok);
    assert!(lab.set_note(A, "with lock").ok);
    assert!(!lab.take_lock(C).ok, "only one holder at a time");
    assert!(lab.release_lock(A).ok);
    assert!(lab.take_lock(C).ok);
}

#[test]
fn lock_holder_cut_off_blocks_everyone_until_lease_expires() {
    let mut lab = Lab::new(Mode::Consistency);
    lab.take_lock(C);
    lab.isolate(C);
    assert!(!lab.set_note(C, "holder, but minority").ok);
    assert!(!lab.set_note(A, "majority, but no lock").ok);
    assert!(!lab.expire_lease(C).ok, "a minority can't expire anything");
    assert!(lab.expire_lease(A).ok);
    assert!(lab.take_lock(A).ok);
    assert!(lab.set_note(A, "recovered").ok);
    lab.heal();
    assert_eq!(lab.cp[C].lock.holder, Some(A), "C learns it lost the lock");
    assert_eq!(lab.cp[C].note, "recovered");
}

#[test]
fn cannot_expire_a_reachable_holder() {
    let mut lab = Lab::new(Mode::Consistency);
    lab.take_lock(C);
    assert!(!lab.expire_lease(A).ok);
}

#[test]
fn isolating_everyone_leaves_no_majority() {
    let mut lab = Lab::new(Mode::Consistency);
    for device in 0..3 {
        lab.isolate(device);
    }
    assert_eq!(lab.net.links, [false; 3], "isolate keeps earlier cuts");
    for device in 0..3 {
        assert!(!lab.sell(device, 1).ok, "no device has a majority");
    }
    assert!(lab.cp.iter().all(|r| r.version == 7), "nothing was written");
}
