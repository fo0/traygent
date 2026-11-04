//! The connected GitHub account and its repository listing (docs/adr/0005).
//!
//! The slow parts (`/user`, gh, the vault write, `/user/repos`) run in [`crate::github`]
//! without the service; these methods only commit their results, so the shell never holds
//! the service lock on the network or a process. A listing mirrors the detection: `loading`
//! dedupes it, every change bumps `revision`, and a listing that finishes after its account
//! was disconnected or replaced is dropped.
use super::{Persisted, Service};
use crate::github::{self, Vault};
use crate::models::{GithubAccount, RepoSummary, Snapshot};
use chrono::{DateTime, Utc};

/// A repository listing [`Service::begin_listing`] started: the caller reads the account's
/// repositories off the lock and hands the result to [`Service::finish_listing`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub account: GithubAccount,
    generation: u64,
}

impl Service {
    pub fn github_account(&self) -> Option<&GithubAccount> {
        self.github.account.as_ref()
    }

    /// Commits an account [`github::connect_token`] or [`github::import_local`] verified and
    /// vaulted, and begins its listing. It replaces the connected account: another account's
    /// vault entry is deleted and its listing cleared; the same account keeps its listing.
    pub fn connect_github(&mut self, account: GithubAccount, vault: &dyn Vault) -> Listing {
        if let Some(previous) = self.github.account.take() {
            if previous.id != account.id {
                // The new account is vaulted already; a stale entry is only reported.
                if let Err(error) = github::disconnect(&previous, vault) {
                    eprintln!("Traygent: {error}");
                }
                self.clear_listing();
            }
        }
        self.github.error = None;
        self.commit_account(Some(account.clone()));
        self.listing_for(account)
    }

    /// Deletes Traygent's vault copy of the token and forgets the account and its listing;
    /// gh stays signed in. A vault that refuses keeps everything as it is.
    pub fn disconnect_github(&mut self, vault: &dyn Vault) -> Result<Snapshot, String> {
        let Some(account) = self.github.account.clone() else {
            return Ok(self.snapshot());
        };
        github::disconnect(&account, vault)?;
        self.clear_listing();
        self.commit_account(None);
        Ok(self.snapshot())
    }

    /// Marks a listing of the connected account as running; `None` without an account or
    /// while one runs for it.
    pub fn begin_listing(&mut self) -> Option<Listing> {
        if self.github.loading {
            return None;
        }
        let account = self.github.account.clone()?;
        Some(self.listing_for(account))
    }

    /// Ends `listing`: repositories replace the last ones, an error (e.g.
    /// [`github::TOKEN_INVALID`]) keeps the account and the last repositories. `None` when
    /// the account changed since it began: the result is dropped and nothing changes.
    pub fn finish_listing(
        &mut self,
        listing: Listing,
        result: Result<Vec<RepoSummary>, String>,
        now: DateTime<Utc>,
    ) -> Option<Snapshot> {
        if !self.is_running(&listing) {
            return None;
        }
        self.github.loading = false;
        match result {
            Ok(repos) => {
                self.github.repos = repos;
                self.github.listed_at = Some(crate::recents::timestamp(now));
                self.github.error = None;
            }
            Err(error) => self.github.error = Some(error),
        }
        self.touch();
        Some(self.snapshot())
    }

    /// Clears a listing that never ran (its thread could not start).
    pub fn abort_listing(&mut self, listing: &Listing) {
        if self.is_running(listing) {
            self.github.loading = false;
            self.touch();
        }
    }

    fn is_running(&self, listing: &Listing) -> bool {
        self.github.loading && listing.generation == self.github_generation
    }

    fn listing_for(&mut self, account: GithubAccount) -> Listing {
        self.github.loading = true;
        self.touch();
        Listing {
            account,
            generation: self.github_generation,
        }
    }

    fn clear_listing(&mut self) {
        self.github.repos.clear();
        self.github.listed_at = None;
        self.github.error = None;
    }

    /// Records the account in `state.json` and drops every listing begun before. The vault
    /// already changed, so a failed save only becomes the snapshot's notice.
    fn commit_account(&mut self, account: Option<GithubAccount>) {
        let next = Persisted {
            github: account.clone(),
            ..self.state.clone()
        };
        self.persist_or_notice(&next);
        self.state = next;
        self.github.account = account;
        self.github.loading = false;
        self.github_generation = self.github_generation.wrapping_add(1);
        self.touch();
    }
}

#[cfg(test)]
mod tests;
