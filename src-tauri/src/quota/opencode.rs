//! Reads the one OpenCode Go credential selected by OpenCode v2. No SQL supplied by the UI.
use crate::github::secret::SecretToken;
use reqwest::Url;
use serde::Deserialize;
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs::{self, Metadata},
    io::ErrorKind,
    path::{Path, PathBuf},
    ptr, slice,
};

const UNAVAILABLE: &str = "Die OpenCode-Anmeldedatenbank konnte nicht gelesen werden.";
const LOGIN: &str = "Bitte in OpenCode mit OpenCode Go anmelden.";
const MAX_ROW: c_int = 64 * 1024;
const READONLY: c_int = 0x01;
const URI: c_int = 0x40;
const ROW: c_int = 100;
const DONE: c_int = 101;
const GO_ROW: &CStr = c"SELECT value FROM credential WHERE integration_id='opencode-go' ORDER BY active DESC, time_created DESC, id DESC LIMIT 1";

// The C ABI is shared by WinSQLite (included in Windows 10/11) and libsqlite3.
// Only opening, reading, limiting and releasing are available to production code.
#[cfg_attr(windows, link(name = "winsqlite3", kind = "raw-dylib"))]
#[cfg_attr(
    target_os = "linux",
    link(name = "libsqlite3.so.0", kind = "dylib", modifiers = "+verbatim")
)]
#[cfg_attr(not(any(windows, target_os = "linux")), link(name = "sqlite3"))]
unsafe extern "C" {
    fn sqlite3_open_v2(
        name: *const c_char,
        db: *mut *mut c_void,
        flags: c_int,
        vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_busy_timeout(db: *mut c_void, ms: c_int) -> c_int;
    fn sqlite3_limit(db: *mut c_void, category: c_int, limit: c_int) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut c_void,
        sql: *const c_char,
        len: c_int,
        stmt: *mut *mut c_void,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_type(stmt: *mut c_void, column: c_int) -> c_int;
    fn sqlite3_column_text(stmt: *mut c_void, column: c_int) -> *const u8;
    fn sqlite3_column_bytes(stmt: *mut c_void, column: c_int) -> c_int;
    fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    #[cfg(test)]
    fn sqlite3_exec(
        db: *mut c_void,
        sql: *const c_char,
        callback: Option<
            unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
        >,
        arg: *mut c_void,
        msg: *mut *mut c_char,
    ) -> c_int;
}

struct Db(*mut c_void);
impl Drop for Db {
    fn drop(&mut self) {
        // The statement drops first. SQLite also accepts NULL after an unsuccessful open.
        unsafe { sqlite3_close(self.0) };
    }
}

struct Statement(*mut c_void);
impl Drop for Statement {
    fn drop(&mut self) {
        unsafe { sqlite3_finalize(self.0) };
    }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut file = path.as_os_str().to_owned();
    file.push(suffix);
    file.into()
}

fn regular_file(path: &Path) -> Result<Option<Metadata>, &'static str> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(Some(meta)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        _ => Err(UNAVAILABLE),
    }
}

pub(super) fn go_key(path: &Path) -> Result<SecretToken, &'static str> {
    let before = regular_file(path)?.ok_or(LOGIN)?;
    let wal = sidecar(path, "-wal");
    let shm = sidecar(path, "-shm");
    let journal = sidecar(path, "-journal");
    if regular_file(&journal)?.is_some() {
        return Err(UNAVAILABLE);
    }
    let live = regular_file(&wal)?.is_some();
    if live != regular_file(&shm)?.is_some() {
        return Err(UNAVAILABLE);
    }
    let mut url = Url::from_file_path(path).map_err(|()| UNAVAILABLE)?;
    // Plain mode=ro can create empty sidecars for a checkpointed, closed WAL database.
    // For a live WAL, SQLite's read transaction sees committed frames and locks normally.
    url.set_query(Some(if live {
        "mode=ro&readonly_shm=1"
    } else {
        "mode=ro&immutable=1"
    }));
    let uri = CString::new(url.as_str()).map_err(|_| UNAVAILABLE)?;
    let token = query(&uri)?;
    if !live {
        let after = regular_file(path)?.ok_or(UNAVAILABLE)?;
        if regular_file(&wal)?.is_some()
            || regular_file(&shm)?.is_some()
            || regular_file(&journal)?.is_some()
            || before.len() != after.len()
            || before.modified().map_err(|_| UNAVAILABLE)?
                != after.modified().map_err(|_| UNAVAILABLE)?
        {
            return Err(UNAVAILABLE);
        }
    }
    Ok(token)
}

fn query(uri: &CStr) -> Result<SecretToken, &'static str> {
    let mut db = Db(ptr::null_mut());
    // SQLite owns both handles. The SQL string is static, and the result bytes are borrowed
    // only between SQLITE_ROW and finalize, never across another step or column conversion.
    unsafe {
        if sqlite3_open_v2(uri.as_ptr(), &mut db.0, READONLY | URI, ptr::null()) != 0 {
            return Err(UNAVAILABLE);
        }
        sqlite3_busy_timeout(db.0, 250);
        sqlite3_limit(db.0, 0 /* SQLITE_LIMIT_LENGTH */, MAX_ROW);
        let mut stmt = Statement(ptr::null_mut());
        if sqlite3_prepare_v2(db.0, GO_ROW.as_ptr(), -1, &mut stmt.0, ptr::null_mut()) != 0 {
            return Err(UNAVAILABLE);
        }
        match sqlite3_step(stmt.0) {
            DONE => return Err(LOGIN),
            ROW if sqlite3_column_type(stmt.0, 0) == 3 /* SQLITE_TEXT */ => {}
            _ => return Err(UNAVAILABLE),
        }
        let value = sqlite3_column_text(stmt.0, 0);
        let len = sqlite3_column_bytes(stmt.0, 0);
        if value.is_null() || !(1..=MAX_ROW).contains(&len) {
            return Err(UNAVAILABLE);
        }
        parse_value(slice::from_raw_parts(value, len as usize))
    }
}

fn parse_value(json: &[u8]) -> Result<SecretToken, &'static str> {
    #[derive(Deserialize)]
    struct Value<'a> {
        #[serde(rename = "type")]
        kind: &'a str,
        key: String,
    }
    let value: Value = serde_json::from_slice(json).map_err(|_| LOGIN)?;
    let token = SecretToken::new(value.key);
    if value.kind != "key"
        || !token.is_well_formed()
        || !token.with_secret(|s| s.bytes().all(|b| b.is_ascii_graphic()))
    {
        return Err(LOGIN);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    fn fixture(path: &Path) -> Db {
        let name = CString::new(path.to_str().unwrap()).unwrap();
        let mut db = Db(ptr::null_mut());
        assert_eq!(
            unsafe { sqlite3_open_v2(name.as_ptr(), &mut db.0, 2 | 4, ptr::null()) },
            0
        );
        sql(&db, "CREATE TABLE credential (id TEXT PRIMARY KEY, integration_id TEXT, value TEXT, active INTEGER, time_created INTEGER)");
        db
    }

    fn sql(db: &Db, text: &str) {
        let sql = CString::new(text).unwrap();
        assert_eq!(
            unsafe { sqlite3_exec(db.0, sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut()) },
            0
        );
    }

    // Bytes 120..128 of a WAL index are SQLite's lock slots and hold no data. Windows locks
    // are mandatory: a read across a slot an open connection holds fails with error 33.
    const SHM_LOCKS: std::ops::Range<u64> = 120..128;

    fn files(dir: &TempDir) -> Vec<(String, Vec<u8>)> {
        use std::io::{Read, Seek, SeekFrom};
        dir.entries()
            .into_iter()
            .map(|name| {
                let mut file = fs::File::open(dir.0.join(&name)).unwrap();
                let mut bytes = Vec::new();
                if name.ends_with("-shm") {
                    (&file)
                        .take(SHM_LOCKS.start)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    file.seek(SeekFrom::Start(SHM_LOCKS.end)).unwrap();
                }
                file.read_to_end(&mut bytes).unwrap();
                (name, bytes)
            })
            .collect()
    }

    #[test]
    fn uses_opencodes_selection_and_never_falls_back_to_another_key() {
        let dir = TempDir::new("opencode-selection");
        let path = dir.0.join("Anmeldung #1 & 2.db");
        let db = fixture(&path);
        sql(
            &db,
            r#"INSERT INTO credential VALUES
            ('a','opencode','{"type":"key","key":"synthetic-zen"}',1,100),
            ('b','opencode-go','{"type":"key","key":"synthetic-inactive"}',0,100),
            ('c','opencode-go','{"type":"key","key":"synthetic-active"}',1,10),
            ('d','opencode-go','{"type":"key","key":"synthetic-chosen"}',1,10)"#,
        );
        let before = files(&dir);
        go_key(&path)
            .unwrap()
            .with_secret(|s| assert_eq!(s, "synthetic-chosen"));
        assert!(
            files(&dir) == before,
            "the database and its sidecars must remain byte-identical"
        );
        for invalid in [
            r#"{"type":"oauth","access":"synthetic"}"#,
            "null",
            r#"{"type":"key","key":"a b"}"#,
            r#"{"type":"key","key":321}"#,
        ] {
            sql(
                &db,
                &format!("UPDATE credential SET value='{invalid}' WHERE id='d'"),
            );
            assert!(go_key(&path).is_err());
        }
        sql(
            &db,
            "UPDATE credential SET value=hex(zeroblob(40000)) WHERE id='d'",
        );
        assert!(go_key(&path).is_err());
        sql(
            &db,
            "DELETE FROM credential WHERE integration_id='opencode-go'",
        );
        assert_eq!(go_key(&path).unwrap_err(), LOGIN);
        sql(&db, "DROP TABLE credential");
        assert_eq!(go_key(&path).unwrap_err(), UNAVAILABLE);
        let missing = dir.0.join("missing.db");
        assert!(go_key(&missing).is_err());
        assert!(!missing.exists());
    }

    #[test]
    fn leaves_unsettled_sidecars_alone() {
        let dir = TempDir::new("opencode-sidecars");
        let path = dir.0.join("opencode.db");
        drop(fixture(&path));
        for suffix in ["-journal", "-shm", "-wal"] {
            let side = sidecar(&path, suffix);
            fs::write(&side, b"synthetic pending transaction").unwrap();
            let before = files(&dir);
            assert_eq!(go_key(&path).unwrap_err(), UNAVAILABLE);
            assert!(files(&dir) == before);
            fs::remove_file(side).unwrap();
        }
    }

    #[test]
    fn reads_committed_wal_and_leaves_a_checkpointed_database_without_sidecars() {
        let dir = TempDir::new("opencode-wal");
        let path = dir.0.join("opencode.db");
        let db = fixture(&path);
        sql(
            &db,
            r#"PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
            INSERT INTO credential VALUES ('a','opencode-go','{"type":"key","key":"synthetic-wal"}',1,1)"#,
        );
        sql(
            &db,
            r#"BEGIN; UPDATE credential SET value='{"type":"key","key":"synthetic-uncommitted"}'"#,
        );
        let before = files(&dir);
        assert_eq!(before.len(), 3);
        go_key(&path)
            .unwrap()
            .with_secret(|s| assert_eq!(s, "synthetic-wal"));
        assert!(
            files(&dir) == before,
            "the database and its sidecars must remain byte-identical"
        );
        sql(&db, "ROLLBACK");
        drop(db);
        let before = files(&dir);
        assert_eq!(before.len(), 1);
        go_key(&path)
            .unwrap()
            .with_secret(|s| assert_eq!(s, "synthetic-wal"));
        assert!(
            files(&dir) == before,
            "the database and its sidecars must remain byte-identical"
        );
    }

    /// The -shm as `files` reads it, with its lock slots restored as zeros.
    fn shm_with_zeroed_locks(files: &[(String, Vec<u8>)]) -> Vec<u8> {
        let (_, bytes) = files
            .iter()
            .find(|(name, _)| name.ends_with("-shm"))
            .unwrap();
        let (at, len) = (SHM_LOCKS.start as usize, SHM_LOCKS.end - SHM_LOCKS.start);
        let mut bytes = bytes.clone();
        bytes.splice(at..at, vec![0; len as usize]);
        bytes
    }

    // In one process SQLite shares one wal-index node per database (by file name on Windows,
    // by inode on Unix), so the test above reuses the writer's read-write mapping. A copy under
    // another name is what OpenCode stopped or crashed leaves behind: nobody holds its DMS lock,
    // so readonly_shm=1 gets SQLITE_READONLY_CANTINIT, ignores the -shm content and rebuilds a
    // private wal-index from the -wal up to its last commit frame (os_win.c winLockSharedMemory,
    // wal.c walIndexReadHdr and walBeginShmUnreliable).
    #[test]
    fn rebuilds_the_wal_index_from_the_wal_when_no_writer_holds_the_shm() {
        let dir = TempDir::new("opencode-writer");
        let path = dir.0.join("opencode.db");
        let db = fixture(&path);
        sql(
            &db,
            r#"PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; PRAGMA cache_size=10;
            CREATE TABLE filler (data BLOB);
            INSERT INTO credential VALUES ('a','opencode-go','{"type":"key","key":"synthetic-stale"}',1,1)"#,
        );
        // A valid wal-index that predates the last commit: trusting it would yield the stale key.
        let stale = shm_with_zeroed_locks(&files(&dir));
        sql(
            &db,
            r#"UPDATE credential SET value='{"type":"key","key":"synthetic-committed"}'"#,
        );
        // The small cache spills the uncommitted change into the -wal as frames without a commit.
        sql(
            &db,
            r#"BEGIN; UPDATE credential SET value='{"type":"key","key":"synthetic-uncommitted"}';
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 64)
            INSERT INTO filler SELECT zeroblob(4000) FROM n"#,
        );
        let live = files(&dir);
        let (_, wal) = live
            .iter()
            .find(|(name, _)| name.ends_with("-wal"))
            .unwrap();
        let pending = b"synthetic-uncommitted";
        assert!(wal.windows(pending.len()).any(|w| w == pending));
        assert!(shm_with_zeroed_locks(&live) != stale);
        for shm in [vec![0; 32768], stale] {
            let copy = TempDir::new("opencode-copy");
            let path = copy.0.join("kopie.db");
            for (name, bytes) in &live {
                let suffix = name.strip_prefix("opencode.db").unwrap();
                let bytes = if suffix == "-shm" { &shm } else { bytes };
                fs::write(sidecar(&path, suffix), bytes).unwrap();
            }
            let before = files(&copy);
            assert_eq!(before.len(), 3);
            go_key(&path)
                .unwrap()
                .with_secret(|s| assert_eq!(s, "synthetic-committed"));
            assert!(
                files(&copy) == before,
                "the database and its sidecars must remain byte-identical"
            );
        }
        sql(&db, "ROLLBACK");
    }
}
