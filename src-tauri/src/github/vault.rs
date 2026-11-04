//! Where the token lives: the operating system's credential vault, service `traygent`, user
//! `github-<id>`. Fails closed: no plaintext, file or in-memory fallback outside tests.
#[cfg(feature = "vault")]
use super::secret::wipe;
use super::secret::SecretToken;
#[cfg(any(feature = "vault", test))]
use std::sync::Mutex;

pub const SERVICE: &str = "traygent";
pub const VAULT_UNAVAILABLE: &str = "Windows-Tresor nicht verfügbar.";

/// The vault entry's user name for a GitHub user id.
pub fn vault_user(id: u64) -> String {
    format!("github-{id}")
}

/// One token per GitHub user id. Calls block on the OS store: run them off the UI thread.
pub trait Vault: Sync {
    /// `Ok(None)` when the vault answered and holds no entry.
    fn read(&self, id: u64) -> Result<Option<SecretToken>, String>;
    fn write(&self, id: u64, token: &SecretToken) -> Result<(), String>;
    /// Deleting a missing entry succeeds.
    fn delete(&self, id: u64) -> Result<(), String>;
}

/// Builds without the `vault` feature: every call fails closed.
pub struct NoVault;

impl Vault for NoVault {
    fn read(&self, _: u64) -> Result<Option<SecretToken>, String> {
        Err(VAULT_UNAVAILABLE.into())
    }
    fn write(&self, _: u64, _: &SecretToken) -> Result<(), String> {
        Err(VAULT_UNAVAILABLE.into())
    }
    fn delete(&self, _: u64) -> Result<(), String> {
        Err(VAULT_UNAVAILABLE.into())
    }
}

// Serializes operations across vault instances; native stores can otherwise process
// concurrent reads and writes of the same entry in an unexpected order.
#[cfg(any(feature = "vault", test))]
static OPERATION_LOCK: Mutex<()> = Mutex::new(());

#[cfg(any(feature = "vault", test))]
/// One native entry. Implementations never format platform errors: some carry secret bytes.
pub(crate) trait Entry {
    fn read(&self) -> Result<Option<Vec<u8>>, ()>;
    fn write(&self, secret: &[u8]) -> Result<(), ()>;
    fn delete(&self) -> Result<(), ()>;
}

#[cfg(any(feature = "vault", test))]
pub(crate) trait Backend: Sync {
    fn entry(&self, service: &str, user: &str) -> Result<Box<dyn Entry>, ()>;
}

#[cfg(any(feature = "vault", test))]
/// The validation and error policy around a native backend; tests drive it with a fake.
pub(crate) struct Store<B> {
    pub(crate) backend: B,
}

#[cfg(any(feature = "vault", test))]
fn user(id: u64) -> Result<String, String> {
    if id == 0 {
        return Err(VAULT_UNAVAILABLE.into());
    }
    Ok(vault_user(id))
}

#[cfg(any(feature = "vault", test))]
fn locked<T>(run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = OPERATION_LOCK
        .lock()
        .map_err(|_| VAULT_UNAVAILABLE.to_string())?;
    run()
}

#[cfg(any(feature = "vault", test))]
impl<B: Backend> Vault for Store<B> {
    fn read(&self, id: u64) -> Result<Option<SecretToken>, String> {
        let user = user(id)?;
        locked(|| {
            let entry = self.backend.entry(SERVICE, &user);
            match entry.and_then(|entry| entry.read()) {
                Ok(Some(bytes)) => SecretToken::from_bytes(bytes)
                    .map(Some)
                    .ok_or_else(|| VAULT_UNAVAILABLE.into()),
                Ok(None) => Ok(None),
                Err(()) => Err(VAULT_UNAVAILABLE.into()),
            }
        })
    }

    fn write(&self, id: u64, token: &SecretToken) -> Result<(), String> {
        let user = user(id)?;
        if !token.is_well_formed() {
            return Err(VAULT_UNAVAILABLE.into());
        }
        locked(|| {
            let entry = self.backend.entry(SERVICE, &user);
            entry
                .and_then(|entry| entry.write(token.bytes()))
                .map_err(|_| VAULT_UNAVAILABLE.into())
        })
    }

    fn delete(&self, id: u64) -> Result<(), String> {
        let user = user(id)?;
        locked(|| {
            let entry = self.backend.entry(SERVICE, &user);
            entry
                .and_then(|entry| entry.delete())
                .map_err(|_| VAULT_UNAVAILABLE.into())
        })
    }
}

/// The OS credential vault through `keyring` v1: Windows Credential Manager (macOS
/// Keychain, Linux Secret Service elsewhere). Construction touches no OS store.
#[cfg(feature = "vault")]
pub struct KeyringVault {
    store: Store<PlatformBackend>,
}

#[cfg(feature = "vault")]
impl KeyringVault {
    pub fn new() -> Self {
        Self {
            store: Store {
                backend: PlatformBackend,
            },
        }
    }
}

#[cfg(feature = "vault")]
impl Default for KeyringVault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "vault")]
impl std::fmt::Debug for KeyringVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyringVault")
            .field("service", &SERVICE)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "vault")]
impl Vault for KeyringVault {
    fn read(&self, id: u64) -> Result<Option<SecretToken>, String> {
        self.store.read(id)
    }
    fn write(&self, id: u64, token: &SecretToken) -> Result<(), String> {
        self.store.write(id, token)
    }
    fn delete(&self, id: u64) -> Result<(), String> {
        self.store.delete(id)
    }
}

#[cfg(feature = "vault")]
struct PlatformBackend;

#[cfg(feature = "vault")]
impl Backend for PlatformBackend {
    fn entry(&self, service: &str, user: &str) -> Result<Box<dyn Entry>, ()> {
        // keyring v1 initialises only the platform's native store; a missing or locked
        // store fails closed.
        let entry = keyring::Entry::new(service, user).map_err(drop_error)?;
        Ok(Box::new(PlatformEntry(entry)))
    }
}

#[cfg(feature = "vault")]
struct PlatformEntry(keyring::Entry);

#[cfg(feature = "vault")]
impl Entry for PlatformEntry {
    fn read(&self) -> Result<Option<Vec<u8>>, ()> {
        match self.0.get_secret() {
            Ok(bytes) => Ok(Some(bytes)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => {
                drop_error(error);
                Err(())
            }
        }
    }

    fn write(&self, secret: &[u8]) -> Result<(), ()> {
        self.0.set_secret(secret).map_err(drop_error)
    }

    fn delete(&self) -> Result<(), ()> {
        match self.0.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => {
                drop_error(error);
                Err(())
            }
        }
    }
}

/// Never formats or returns a platform error; wipes the bytes a bad-encoding error carries.
#[cfg(feature = "vault")]
fn drop_error(error: keyring::Error) {
    if let keyring::Error::BadEncoding(mut bytes) = error {
        wipe(&mut bytes);
    }
}

/// In-memory vault for tests; never compiled into a build.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryVault {
    pub(crate) values: Mutex<std::collections::BTreeMap<u64, String>>,
    pub(crate) fail: bool,
}

#[cfg(test)]
impl Vault for MemoryVault {
    fn read(&self, id: u64) -> Result<Option<SecretToken>, String> {
        if self.fail {
            return Err(VAULT_UNAVAILABLE.into());
        }
        let values = self.values.lock().unwrap();
        Ok(values.get(&id).cloned().map(SecretToken::new))
    }
    fn write(&self, id: u64, token: &SecretToken) -> Result<(), String> {
        if self.fail {
            return Err(VAULT_UNAVAILABLE.into());
        }
        let value = token.with_secret(str::to_owned);
        self.values.lock().unwrap().insert(id, value);
        Ok(())
    }
    fn delete(&self, id: u64) -> Result<(), String> {
        if self.fail {
            return Err(VAULT_UNAVAILABLE.into());
        }
        self.values.lock().unwrap().remove(&id);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
