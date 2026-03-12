use super::core::open_write_connection;
use rusqlite::params;

pub fn get_cached_md5(file_path: &str, file_size: i64, mtime_sec: i64) -> Option<String> {
    let conn = open_write_connection().ok()?;
    conn.query_row(
        "SELECT md5 FROM hash_cache WHERE file_path = ?1 AND file_size = ?2 AND mtime_sec = ?3",
        params![file_path, file_size, mtime_sec],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_cached_md5(file_path: &str, file_size: i64, mtime_sec: i64, md5: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    conn.execute(
        "INSERT OR REPLACE INTO hash_cache (file_path, file_size, mtime_sec, md5) VALUES (?1, ?2, ?3, ?4)",
        params![file_path, file_size, mtime_sec, md5],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 hash_cache 失败: path={}, err={}", file_path, e);
        0
    });
}
