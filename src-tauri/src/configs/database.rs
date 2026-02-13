use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

/// 全局数据库连接（存放在 ~/.config/ssmt4/ssmt4.db）
static DB: once_cell::sync::Lazy<Mutex<Connection>> = once_cell::sync::Lazy::new(|| {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("无法打开数据库 {}: {}", db_path.display(), e));
    init_tables(&conn);
    tracing::info!("SQLite 数据库已打开: {}", db_path.display());
    Mutex::new(conn)
});

fn get_db_path() -> PathBuf {
    super::app_config::get_app_config_dir().join("ssmt4.db")
}

fn init_tables(conn: &Connection) {
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
        ",
    )
    .expect("创建数据库表失败");
}

// ============================
//  通用 KV 操作 — settings 表
// ============================

pub fn get_setting(key: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_setting(key: &str, value: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 settings 失败: key={}, err={}", key, e);
        0
    });
}

pub fn get_all_settings() -> Vec<(String, String)> {
    let conn = DB.lock().unwrap();
    let mut stmt = conn.prepare("SELECT key, value FROM settings").unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap();
    rows.filter_map(|r| r.ok()).collect()
}

// ============================
//  游戏配置操作 — game_configs 表
// ============================

pub fn get_game_config(game_name: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT config FROM game_configs WHERE game_name = ?1",
        params![game_name],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_game_config(game_name: &str, config_json: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO game_configs (game_name, config) VALUES (?1, ?2)",
        params![game_name, config_json],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 game_configs 失败: game={}, err={}", game_name, e);
        0
    });
}

pub fn delete_game_config(game_name: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "DELETE FROM game_configs WHERE game_name = ?1",
        params![game_name],
    )
    .unwrap_or_else(|e| {
        tracing::error!("删除 game_configs 失败: game={}, err={}", game_name, e);
        0
    });
}

pub fn list_game_names() -> Vec<String> {
    let conn = DB.lock().unwrap();
    let mut stmt = conn.prepare("SELECT game_name FROM game_configs").unwrap();
    let rows = stmt.query_map([], |row| row.get(0)).unwrap();
    rows.filter_map(|r| r.ok()).collect()
}
