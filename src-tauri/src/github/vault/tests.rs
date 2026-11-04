use super::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
struct FakeState {
    values: BTreeMap<(String, String), Vec<u8>>,
    entries: Vec<(String, String)>,
    fail: bool,
}

struct FakeBackend(Arc<Mutex<FakeState>>);

struct FakeEntry {
    key: (String, String),
    state: Arc<Mutex<FakeState>>,
}

impl Backend for FakeBackend {
    fn entry(&self, service: &str, user: &str) -> Result<Box<dyn Entry>, ()> {
        let mut state = self.0.lock().unwrap();
        state.entries.push((service.into(), user.into()));
        if state.fail {
            return Err(());
        }
        Ok(Box::new(FakeEntry {
            key: (service.into(), user.into()),
            state: self.0.clone(),
        }))
    }
}

impl Entry for FakeEntry {
    fn read(&self) -> Result<Option<Vec<u8>>, ()> {
        Ok(self.state.lock().unwrap().values.get(&self.key).cloned())
    }
    fn write(&self, secret: &[u8]) -> Result<(), ()> {
        let mut state = self.state.lock().unwrap();
        state.values.insert(self.key.clone(), secret.to_vec());
        Ok(())
    }
    fn delete(&self) -> Result<(), ()> {
        self.state.lock().unwrap().values.remove(&self.key);
        Ok(())
    }
}

fn store(state: &Arc<Mutex<FakeState>>) -> Store<FakeBackend> {
    Store {
        backend: FakeBackend(state.clone()),
    }
}

fn token(value: &str) -> SecretToken {
    SecretToken::new(value.into())
}

#[test]
fn entries_live_under_service_traygent_and_user_github_id() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let vault = store(&state);
    vault.write(1, &token("synthetic-one")).unwrap();
    vault.write(2, &token("synthetic-two")).unwrap();
    let read = |id| {
        vault
            .read(id)
            .unwrap()
            .map(|t| t.with_secret(str::to_owned))
    };
    assert_eq!(read(1).as_deref(), Some("synthetic-one"));
    assert_eq!(read(2).as_deref(), Some("synthetic-two"));
    vault.delete(1).unwrap();
    assert_eq!(read(1), None);
    assert_eq!(read(2).as_deref(), Some("synthetic-two"));
    // Deleting a missing entry succeeds.
    vault.delete(1).unwrap();
    let state = state.lock().unwrap();
    assert!(state
        .entries
        .iter()
        .all(|(service, _)| service == "traygent"));
    assert!(state
        .values
        .contains_key(&("traygent".into(), "github-2".into())));
}

#[test]
fn malformed_ids_and_tokens_never_reach_the_backend() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let vault = store(&state);
    assert_eq!(vault.read(0).unwrap_err(), VAULT_UNAVAILABLE);
    assert_eq!(
        vault.write(0, &token("synthetic")).unwrap_err(),
        VAULT_UNAVAILABLE
    );
    assert_eq!(vault.delete(0).unwrap_err(), VAULT_UNAVAILABLE);
    for bad in ["", "synthetic\ntoken", "with space", &"x".repeat(2049)] {
        assert_eq!(vault.write(7, &token(bad)).unwrap_err(), VAULT_UNAVAILABLE);
    }
    assert!(state.lock().unwrap().entries.is_empty());
}

#[test]
fn unreadable_entries_and_unavailable_stores_fail_closed_without_the_secret() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let vault = store(&state);
    let key = ("traygent".to_string(), "github-3".to_string());
    for bytes in [vec![0xff, 0x00], Vec::new(), b"two words".to_vec()] {
        state.lock().unwrap().values.insert(key.clone(), bytes);
        assert_eq!(vault.read(3).unwrap_err(), VAULT_UNAVAILABLE);
    }
    state.lock().unwrap().fail = true;
    let secret = token("synthetic-private-token");
    let error = vault.write(3, &secret).unwrap_err();
    assert_eq!(error, VAULT_UNAVAILABLE);
    assert!(!format!("{error:?} {secret:?}").contains("synthetic-private-token"));
    assert!(vault.read(3).is_err());
    assert!(vault.delete(3).is_err());
}

#[test]
fn no_vault_fails_closed_and_memory_vault_round_trips() {
    assert_eq!(NoVault.read(1).unwrap_err(), VAULT_UNAVAILABLE);
    assert!(NoVault.write(1, &token("synthetic")).is_err());
    assert!(NoVault.delete(1).is_err());
    let memory = MemoryVault::default();
    memory.write(9, &token("synthetic")).unwrap();
    assert!(memory.read(9).unwrap().is_some());
    memory.delete(9).unwrap();
    assert!(memory.read(9).unwrap().is_none());
}

#[cfg(feature = "vault")]
#[test]
fn keyring_vault_constructs_without_touching_the_os_store_and_hides_platform_errors() {
    let vault = KeyringVault::new();
    assert_eq!(
        format!("{vault:?}"),
        "KeyringVault { service: \"traygent\", .. }"
    );
    drop_error(keyring::Error::BadEncoding(b"synthetic-secret".to_vec()));
}
