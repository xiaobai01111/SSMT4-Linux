#pragma once

#include <string>
#include <cstdio>

namespace bridge {

class StatusReporter {
public:
    ~StatusReporter();

    /// Enable dual-write: all JSON output goes to both stdout and the given file.
    /// The file is truncated on open. Call this after loading config.
    void set_log_file(const std::string& path);

    void status(const std::string& message);
    void progress(const std::string& stage, int current, int total);
    void warning(const std::string& message);
    void error(const std::string& code, const std::string& message);
    void inject_result(const std::string& method, bool success, unsigned long pid);
    void done(bool success);
    void log(const std::string& level, const std::string& message);

private:
    void emit_json(const std::string& json_line);
    std::string escape_json_string(const std::string& s);

    FILE* log_file_ = nullptr;
};

} // namespace bridge
