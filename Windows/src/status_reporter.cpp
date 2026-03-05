#include "status_reporter.h"
#include <cstdio>

namespace bridge {

StatusReporter::~StatusReporter() {
    if (log_file_) {
        std::fclose(log_file_);
        log_file_ = nullptr;
    }
}

void StatusReporter::set_log_file(const std::string& path) {
    if (log_file_) {
        std::fclose(log_file_);
        log_file_ = nullptr;
    }
    log_file_ = std::fopen(path.c_str(), "w");
}

std::string StatusReporter::escape_json_string(const std::string& s) {
    std::string result;
    result.reserve(s.size() + 16);
    for (char c : s) {
        switch (c) {
            case '"':  result += "\\\""; break;
            case '\\': result += "\\\\"; break;
            case '\b': result += "\\b";  break;
            case '\f': result += "\\f";  break;
            case '\n': result += "\\n";  break;
            case '\r': result += "\\r";  break;
            case '\t': result += "\\t";  break;
            default:   result += c;      break;
        }
    }
    return result;
}

void StatusReporter::emit_json(const std::string& json_line) {
    std::fprintf(stdout, "%s\n", json_line.c_str());
    std::fflush(stdout);
    if (log_file_) {
        std::fprintf(log_file_, "%s\n", json_line.c_str());
        std::fflush(log_file_);
    }
}

void StatusReporter::status(const std::string& message) {
    emit_json("{\"type\":\"status\",\"message\":\"" + escape_json_string(message) + "\"}");
}

void StatusReporter::progress(const std::string& stage, int current, int total) {
    emit_json("{\"type\":\"progress\",\"stage\":\"" + escape_json_string(stage) +
              "\",\"current\":" + std::to_string(current) +
              ",\"total\":" + std::to_string(total) + "}");
}

void StatusReporter::warning(const std::string& message) {
    emit_json("{\"type\":\"warning\",\"message\":\"" + escape_json_string(message) + "\"}");
}

void StatusReporter::error(const std::string& code, const std::string& message) {
    emit_json("{\"type\":\"error\",\"code\":\"" + escape_json_string(code) +
              "\",\"message\":\"" + escape_json_string(message) + "\"}");
}

void StatusReporter::inject_result(const std::string& method, bool success, unsigned long pid) {
    emit_json("{\"type\":\"inject_result\",\"method\":\"" + escape_json_string(method) +
              "\",\"success\":" + (success ? "true" : "false") +
              ",\"pid\":" + std::to_string(pid) + "}");
}

void StatusReporter::done(bool success) {
    emit_json("{\"type\":\"done\",\"success\":" + std::string(success ? "true" : "false") + "}");
}

void StatusReporter::log(const std::string& level, const std::string& message) {
    emit_json("{\"type\":\"log\",\"level\":\"" + escape_json_string(level) +
              "\",\"message\":\"" + escape_json_string(message) + "\"}");
}

} // namespace bridge
