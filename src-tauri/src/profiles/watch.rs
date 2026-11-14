//! The status watch after a visible sign-in (#41, „Debug“ on): the terminal stays open and
//! reports nothing back, so the profile's status is read again every [`WATCH_TICK`] until
//! its sign-in facts change or [`WATCH_LIMIT`] passes. Only a cheap read ([`SyncPlan::read`]),
//! never a sync; the host runs it without the service lock and commits only a change.
//! One watch per profile: a new one replaces the running one, which ends at its next tick.
use super::status::{Facts, Key};
use super::SyncPlan;
use crate::models::AgentId;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

pub const WATCH_TICK: Duration = Duration::from_secs(3);
pub const WATCH_LIMIT: Duration = Duration::from_secs(10 * 60);

static WATCHES: Mutex<Vec<(Key, u64)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

fn watches() -> std::sync::MutexGuard<'static, Vec<(Key, u64)>> {
    WATCHES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One running watch; dropping it (also while unwinding) unregisters it, a newer one stays.
pub struct Watch {
    key: Key,
    generation: u64,
    before: Facts,
    started: Instant,
}

/// What a read means for the watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The sign-in facts changed: commit the read, then end.
    Changed,
    Wait,
    /// The limit passed or the profile is gone: end without a commit.
    Stop,
}

/// Starts the watch of `key` from the facts the panel shows now; replaces a running one.
pub fn start(key: Key, before: Facts, now: Instant) -> Watch {
    let generation = NEXT.fetch_add(1, Ordering::Relaxed);
    let mut list = watches();
    list.retain(|(listed, _)| *listed != key);
    list.push((key.clone(), generation));
    Watch {
        key,
        generation,
        before,
        started: now,
    }
}

/// Whether a watch of `key` runs.
pub fn watching(key: &Key) -> bool {
    watches().iter().any(|(listed, _)| listed == key)
}

/// Signed in, account and organization; the sync time is not part of the sign-in.
fn signed_alike(a: &Facts, b: &Facts) -> bool {
    a.signed_in == b.signed_in && a.account == b.account && a.organization == b.organization
}

impl Watch {
    pub fn agent(&self) -> AgentId {
        self.key.0
    }

    /// False once a newer watch of the same profile replaced this one.
    pub fn current(&self) -> bool {
        watches()
            .iter()
            .any(|(key, generation)| *key == self.key && *generation == self.generation)
    }

    /// `read`: the profile's facts just read, `None` when the plan no longer lists it.
    pub fn step(&self, read: Option<&Facts>, now: Instant) -> Step {
        match read {
            None => Step::Stop,
            Some(facts) if !signed_alike(facts, &self.before) => Step::Changed,
            Some(_) if now.saturating_duration_since(self.started) >= WATCH_LIMIT => Step::Stop,
            Some(_) => Step::Wait,
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let generation = self.generation;
        watches().retain(|(_, listed)| *listed != generation);
    }
}

/// What a watch needs from the shell; tests pass a fake clock.
pub trait WatchHost {
    fn sleep(&self, duration: Duration);
    fn now(&self) -> Instant;
    /// The read plan of `agent`'s profiles, taken under the service lock.
    fn plan(&self, agent: AgentId) -> SyncPlan;
    /// Applies the read under the service lock and tells an open panel.
    fn commit(&self, outcome: super::SyncOutcome);
    fn read(&self, plan: &SyncPlan) -> super::SyncOutcome {
        plan.read()
    }
}

/// Runs `watch` to its end. A read that panics ends it; nothing is committed then.
pub fn run(watch: Watch, host: &impl WatchHost) {
    loop {
        host.sleep(WATCH_TICK);
        if !watch.current() {
            return;
        }
        let plan = host.plan(watch.agent());
        let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.read(&plan)));
        let Ok(outcome) = read else {
            return;
        };
        match watch.step(outcome.facts.get(&watch.key), host.now()) {
            Step::Changed => {
                if watch.current() {
                    host.commit(outcome);
                }
                return;
            }
            Step::Wait => {}
            Step::Stop => return,
        }
    }
}

#[cfg(test)]
mod tests;
