#pragma once

#include <string>
#include <vector>
#include <cstdint>

struct sqlite3;

namespace bridge {

class LocalStorage {
public:
    explicit LocalStorage(const std::wstring& db_path);
    ~LocalStorage();

    LocalStorage(const LocalStorage&) = delete;
    LocalStorage& operator=(const LocalStorage&) = delete;

    void connect();
    void save();

    std::string get_value(const std::string& key);
    void set_value(const std::string& key, const std::string& value);
    void delete_value(const std::string& key);

    struct Trigger {
        std::string name;
        std::string table;
        std::string body;
    };

    std::vector<Trigger> get_all_triggers();
    Trigger get_trigger(const std::string& name);
    void set_value_lock_trigger(const std::string& trigger_name,
                                 const std::string& key,
                                 const std::string& value);
    void delete_trigger(const std::string& name);

    void set_fps(int value);
    void reset_fps_triggers();

private:
    std::wstring db_path_;
    sqlite3* db_ = nullptr;
    bool modified_ = false;

    void disconnect();
    void ensure_table();
};

} // namespace bridge
