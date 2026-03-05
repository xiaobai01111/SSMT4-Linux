#include "local_storage.h"
#include "path_utils.h"
#include "string_utils.h"
#include <sqlite3.h>
#include <stdexcept>
#include <cstring>

namespace bridge {

LocalStorage::LocalStorage(const std::wstring& db_path)
    : db_path_(db_path) {
}

LocalStorage::~LocalStorage() {
    disconnect();
}

void LocalStorage::connect() {
    if (db_) return;

    std::string path_utf8 = wide_to_utf8(db_path_);
    int rc = sqlite3_open(path_utf8.c_str(), &db_);
    if (rc != SQLITE_OK) {
        std::string err = db_ ? sqlite3_errmsg(db_) : "unknown error";
        sqlite3_close(db_);
        db_ = nullptr;
        throw std::runtime_error("Failed to open database: " + err);
    }

    ensure_table();
}

void LocalStorage::disconnect() {
    if (db_) {
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

void LocalStorage::save() {
    // SQLite auto-commits; this is a no-op but kept for interface compatibility
}

void LocalStorage::ensure_table() {
    if (!db_) return;
    const char* sql = "CREATE TABLE IF NOT EXISTS LocalStorage ("
                      "key TEXT PRIMARY KEY NOT NULL, "
                      "value TEXT NOT NULL)";
    char* err = nullptr;
    int rc = sqlite3_exec(db_, sql, nullptr, nullptr, &err);
    if (rc != SQLITE_OK) {
        std::string msg = err ? err : "unknown error";
        sqlite3_free(err);
        throw std::runtime_error("Failed to create table: " + msg);
    }
}

std::string LocalStorage::get_value(const std::string& key) {
    if (!db_) throw std::runtime_error("Database not connected");

    sqlite3_stmt* stmt = nullptr;
    const char* sql = "SELECT value FROM LocalStorage WHERE key = ?";
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return "";

    sqlite3_bind_text(stmt, 1, key.c_str(), -1, SQLITE_STATIC);

    std::string result;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        const char* val = (const char*)sqlite3_column_text(stmt, 0);
        if (val) result = val;
    }
    sqlite3_finalize(stmt);
    return result;
}

void LocalStorage::set_value(const std::string& key, const std::string& value) {
    if (!db_) throw std::runtime_error("Database not connected");

    const char* sql = "INSERT OR REPLACE INTO LocalStorage (key, value) VALUES (?, ?)";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        throw std::runtime_error("Failed to prepare INSERT: " +
                                 std::string(sqlite3_errmsg(db_)));
    }

    sqlite3_bind_text(stmt, 1, key.c_str(), -1, SQLITE_STATIC);
    sqlite3_bind_text(stmt, 2, value.c_str(), -1, SQLITE_STATIC);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);
    if (rc != SQLITE_DONE) {
        throw std::runtime_error("Failed to set value: " +
                                 std::string(sqlite3_errmsg(db_)));
    }
    modified_ = true;
}

void LocalStorage::delete_value(const std::string& key) {
    if (!db_) throw std::runtime_error("Database not connected");

    const char* sql = "DELETE FROM LocalStorage WHERE key = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return;

    sqlite3_bind_text(stmt, 1, key.c_str(), -1, SQLITE_STATIC);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);
}

std::vector<LocalStorage::Trigger> LocalStorage::get_all_triggers() {
    std::vector<Trigger> triggers;
    if (!db_) return triggers;

    const char* sql = "SELECT name, tbl_name, sql FROM sqlite_master WHERE type = 'trigger'";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return triggers;

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        Trigger t;
        const char* name = (const char*)sqlite3_column_text(stmt, 0);
        const char* table = (const char*)sqlite3_column_text(stmt, 1);
        const char* body = (const char*)sqlite3_column_text(stmt, 2);
        if (name) t.name = name;
        if (table) t.table = table;
        if (body) t.body = body;
        triggers.push_back(t);
    }
    sqlite3_finalize(stmt);
    return triggers;
}

LocalStorage::Trigger LocalStorage::get_trigger(const std::string& name) {
    Trigger result;
    if (!db_) return result;

    const char* sql = "SELECT name, tbl_name, sql FROM sqlite_master "
                      "WHERE type = 'trigger' AND name = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return result;

    sqlite3_bind_text(stmt, 1, name.c_str(), -1, SQLITE_STATIC);

    if (sqlite3_step(stmt) == SQLITE_ROW) {
        const char* n = (const char*)sqlite3_column_text(stmt, 0);
        const char* t = (const char*)sqlite3_column_text(stmt, 1);
        const char* b = (const char*)sqlite3_column_text(stmt, 2);
        if (n) result.name = n;
        if (t) result.table = t;
        if (b) result.body = b;
    }
    sqlite3_finalize(stmt);
    return result;
}

void LocalStorage::delete_trigger(const std::string& name) {
    if (!db_) return;
    std::string sql = "DROP TRIGGER IF EXISTS " + name;
    sqlite3_exec(db_, sql.c_str(), nullptr, nullptr, nullptr);
}

void LocalStorage::set_value_lock_trigger(const std::string& trigger_name,
                                           const std::string& key,
                                           const std::string& value) {
    if (!db_) throw std::runtime_error("Database not connected");

    // First delete existing trigger with same name
    delete_trigger(trigger_name);

    // Create new lock trigger
    // This trigger fires AFTER UPDATE on LocalStorage and resets the value
    std::string sql =
        "CREATE TRIGGER " + trigger_name + " "
        "AFTER UPDATE OF value ON LocalStorage "
        "WHEN NEW.key = '" + key + "' "
        "BEGIN "
        "UPDATE LocalStorage SET value = '" + value + "' WHERE key = '" + key + "'; "
        "END";

    char* err = nullptr;
    int rc = sqlite3_exec(db_, sql.c_str(), nullptr, nullptr, &err);
    if (rc != SQLITE_OK) {
        std::string msg = err ? err : "unknown error";
        sqlite3_free(err);
        throw std::runtime_error("Failed to create lock trigger: " + msg);
    }
}

void LocalStorage::set_fps(int value) {
    if (!db_) throw std::runtime_error("Database not connected");

    // Delete any third-party CustomFrameRate triggers
    auto triggers = get_all_triggers();
    for (auto& t : triggers) {
        if (t.body.find("CustomFrameRate") != std::string::npos) {
            delete_trigger(t.name);
        }
    }

    // Set CustomFrameRate value
    set_value("CustomFrameRate", std::to_string(value));

    // Set MenuData JSON with FPS setting
    // This mirrors wwmi_package.py:504-520
    std::string menu_data = "{\"KeyCustomFrameRate\":" + std::to_string(value) + "}";
    set_value("MenuData", menu_data);

    // Set PlayMenuInfo JSON
    std::string play_info = "{\"CustomFrameRate\":" + std::to_string(value) + "}";
    set_value("PlayMenuInfo", play_info);

    // Create lock trigger to persist the FPS setting
    set_value_lock_trigger("lock_custom_frame_rate", "CustomFrameRate", std::to_string(value));
}

void LocalStorage::reset_fps_triggers() {
    if (!db_) return;

    // Remove all CustomFrameRate-related triggers
    auto triggers = get_all_triggers();
    for (auto& t : triggers) {
        if (t.body.find("CustomFrameRate") != std::string::npos) {
            delete_trigger(t.name);
        }
    }
}

} // namespace bridge
