//! One sync at a time: [`busy`] tells the background schedule that one is in flight.
//! Synthetic data only.
use super::super::sync::busy;
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Barrier;

/// Holds its first call until the test has looked, then links like the test linker.
struct HeldLinker {
    entered: Barrier,
    release: Barrier,
    held: AtomicBool,
}

impl Linker for HeldLinker {
    fn link(&self, links: &[Link]) {
        if !self.held.swap(true, Ordering::SeqCst) {
            self.entered.wait();
            self.release.wait();
        }
        crate::testing::TestLinker.link(links);
    }
}

#[test]
fn the_turn_is_busy_while_a_sync_runs() {
    let world = World::new("profiles-busy");
    let linker = HeldLinker {
        entered: Barrier::new(2),
        release: Barrier::new(2),
        held: AtomicBool::new(false),
    };
    let share = ProfileShare::default();
    let reports = std::thread::scope(|scope| {
        let syncing = scope.spawn(|| {
            sync_agent(
                &world.homes,
                CLAUDE,
                &[profile("team")],
                &share,
                &linker,
                at(1),
            )
        });
        linker.entered.wait();
        // Looked at before the release, asserted after it: a failed assert never strands
        // the sync on the barrier.
        let during = busy();
        linker.release.wait();
        let reports = syncing.join().unwrap();
        assert!(during);
        reports
    });
    assert_eq!(reports[0].errors, Vec::<String>::new());
}
