#pragma once

#include <string>
#include <vector>
#include <map>
#include <cstdint>

namespace bridge {

struct IniValidationIssue {
    std::wstring file_path;
    std::string issue_type;     // "rogue_ini", "unwanted_trigger", "global_trigger", "duplicate_library"
    std::vector<int> line_numbers;  // lines to comment out (0-indexed)
    std::string details;
};

class IniValidator {
public:
    IniValidator(const std::wstring& folder_path,
                 const std::vector<std::string>& exclude_patterns,
                 bool use_cache, bool new_cache,
                 const std::wstring& cache_path);

    void scan();

    std::vector<std::wstring> get_rogue_ini_files() const;
    std::vector<IniValidationIssue> get_unwanted_triggers() const;
    std::vector<IniValidationIssue> get_global_triggers() const;
    std::vector<IniValidationIssue> get_duplicate_libraries() const;

private:
    std::wstring folder_path_;
    std::vector<std::string> exclude_patterns_;
    bool use_cache_;
    bool new_cache_;
    std::wstring cache_path_;

    std::vector<std::wstring> rogue_ini_files_;
    std::vector<IniValidationIssue> unwanted_triggers_;
    std::vector<IniValidationIssue> global_triggers_;
    std::vector<IniValidationIssue> duplicate_libraries_;

    struct CacheEntry {
        uint64_t mtime;
        uint64_t size;
        std::vector<IniValidationIssue> issues;
    };
    std::map<std::wstring, CacheEntry> cache_;

    void load_cache();
    void save_cache();
    bool is_excluded(const std::wstring& path) const;
    void validate_file(const std::wstring& file_path);
};

} // namespace bridge
