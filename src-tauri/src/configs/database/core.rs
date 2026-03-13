use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseInitState {
    Uninitialized,
    Ready,
}

static DB_INIT_STATE: once_cell::sync::Lazy<Mutex<DatabaseInitState>> =
    once_cell::sync::Lazy::new(|| Mutex::new(DatabaseInitState::Uninitialized));

pub(super) fn get_db_path() -> PathBuf {
    super::super::app_config::get_app_config_dir().join("ssmt4.db")
}

pub(super) fn open_write_connection() -> Result<Connection, String> {
    ensure_database_ready()?;
    Connection::open(get_db_path()).map_err(|e| format!("无法打开数据库连接: {}", e))
}

pub(super) fn open_read_only_connection() -> Option<Connection> {
    ensure_database_ready().ok()?;
    let db_path = get_db_path();
    if !db_path.exists() {
        return None;
    }

    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

fn ensure_database_ready() -> Result<(), String> {
    ensure_database_ready_with(initialize_database)
}

fn ensure_database_ready_with<F>(initialize: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    let mut state = DB_INIT_STATE
        .lock()
        .map_err(|_| "数据库初始化状态锁已损坏".to_string())?;

    if *state == DatabaseInitState::Ready {
        return Ok(());
    }

    initialize()?;
    *state = DatabaseInitState::Ready;
    Ok(())
}

fn initialize_database() -> Result<(), String> {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("无法创建数据库目录 {}: {}", parent.display(), e))?;
    }

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("无法打开数据库 {}: {}", db_path.display(), e))?;
    init_tables(&conn)?;
    tracing::info!("SQLite 数据库已打开: {}", db_path.display());
    Ok(())
}

fn init_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_configs (
            game_name TEXT PRIMARY KEY,
            config    TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_configs_v2 (
            game_name      TEXT PRIMARY KEY,
            schema_version INTEGER NOT NULL,
            config_json    TEXT NOT NULL,
            updated_at     TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_key_aliases (
            alias_key     TEXT PRIMARY KEY,
            canonical_key TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_identities (
            canonical_key TEXT PRIMARY KEY,
            display_name_en TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_presets (
            id         TEXT PRIMARY KEY,
            preset_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proton_families (
            family_key TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            detect_patterns_json TEXT NOT NULL DEFAULT '[]',
            builtin INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proton_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            family_key TEXT NOT NULL,
            provider TEXT NOT NULL,
            repo TEXT NOT NULL,
            endpoint TEXT NOT NULL DEFAULT '',
            url_template TEXT NOT NULL DEFAULT '',
            asset_index INTEGER NOT NULL DEFAULT -1,
            asset_pattern TEXT NOT NULL DEFAULT '(?i)\\.tar\\.(gz|xz)$',
            tag_pattern TEXT NOT NULL DEFAULT '.*',
            max_count INTEGER NOT NULL DEFAULT 15,
            include_prerelease INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            note TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_proton_sources_family_provider_repo
            ON proton_sources (family_key, provider, repo);

        CREATE TABLE IF NOT EXISTS proton_catalog_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS migration_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS hash_cache (
            file_path TEXT PRIMARY KEY,
            file_size INTEGER NOT NULL,
            mtime_sec INTEGER NOT NULL,
            md5       TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| format!("创建数据库表失败: {}", e))?;

    super::proton::ensure_proton_catalog_schema(conn);
    super::game_catalog_seed::ensure_game_catalog_seed(conn);
    super::proton::ensure_proton_catalog_seed(conn);
    Ok(())
}

#[cfg(test)]
fn reset_database_ready_state_for_test() {
    if let Ok(mut state) = DB_INIT_STATE.lock() {
        *state = DatabaseInitState::Uninitialized;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[test]
    fn init_failure_is_not_cached_forever() {
        let _guard = TEST_GUARD.lock().unwrap();
        reset_database_ready_state_for_test();

        let mut attempts = 0usize;
        let err = ensure_database_ready_with(|| {
            attempts += 1;
            Err("boom".to_string())
        });
        assert_eq!(err.unwrap_err(), "boom");
        assert_eq!(attempts, 1);

        let ok = ensure_database_ready_with(|| {
            attempts += 1;
            Ok(())
        });
        assert!(ok.is_ok());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn successful_init_is_cached_for_process() {
        let _guard = TEST_GUARD.lock().unwrap();
        reset_database_ready_state_for_test();

        let mut attempts = 0usize;
        ensure_database_ready_with(|| {
            attempts += 1;
            Ok(())
        })
        .unwrap();
        ensure_database_ready_with(|| {
            attempts += 1;
            Ok(())
        })
        .unwrap();

        assert_eq!(attempts, 1);
    }
}
