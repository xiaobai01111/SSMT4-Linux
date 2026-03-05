#include "ini_validator.h"
#include "ini_handler.h"
#include "path_utils.h"
#include "string_utils.h"
#include <cJSON.h>
#include <algorithm>

namespace bridge {

IniValidator::IniValidator(const std::wstring& folder_path,
                           const std::vector<std::string>& exclude_patterns,
                           bool use_cache, bool new_cache,
                           const std::wstring& cache_path)
    : folder_path_(folder_path)
    , exclude_patterns_(exclude_patterns)
    , use_cache_(use_cache)
    , new_cache_(new_cache)
    , cache_path_(cache_path) {
    if (use_cache_ && !new_cache_) {
        load_cache();
    }
}

void IniValidator::load_cache() {
    std::string content = read_file_text(cache_path_);
    if (content.empty()) return;

    cJSON* root = cJSON_Parse(content.c_str());
    if (!root) return;

    cJSON* entry = nullptr;
    cJSON_ArrayForEach(entry, root) {
        if (!cJSON_IsObject(entry) || !entry->string) continue;
        std::wstring path = utf8_to_wide(entry->string);
        CacheEntry ce;
        ce.mtime = (uint64_t)cJSON_GetObjectItemCaseSensitive(entry, "mtime")->valuedouble;
        ce.size = (uint64_t)cJSON_GetObjectItemCaseSensitive(entry, "size")->valuedouble;
        cache_[path] = ce;
    }
    cJSON_Delete(root);
}

void IniValidator::save_cache() {
    if (!use_cache_) return;

    cJSON* root = cJSON_CreateObject();
    for (auto& [path, entry] : cache_) {
        cJSON* obj = cJSON_CreateObject();
        cJSON_AddNumberToObject(obj, "mtime", (double)entry.mtime);
        cJSON_AddNumberToObject(obj, "size", (double)entry.size);
        cJSON_AddItemToObject(root, wide_to_utf8(path).c_str(), obj);
    }
    char* json = cJSON_PrintUnformatted(root);
    if (json) {
        ensure_directory(get_parent_path(cache_path_));
        write_file_text(cache_path_, json);
        cJSON_free(json);
    }
    cJSON_Delete(root);
}

bool IniValidator::is_excluded(const std::wstring& path) const {
    std::wstring name = get_filename(path);
    for (auto& pattern : exclude_patterns_) {
        if (matches_glob(name, utf8_to_wide(pattern))) return true;
    }
    // Also check parent directory name
    std::wstring parent = get_filename(get_parent_path(path));
    for (auto& pattern : exclude_patterns_) {
        if (matches_glob(parent, utf8_to_wide(pattern))) return true;
    }
    return false;
}

void IniValidator::validate_file(const std::wstring& file_path) {
    std::string content = read_file_text(file_path);
    if (content.empty()) return;

    IniHandlerSettings settings;
    settings.ignore_comments = false;
    settings.option_value_spacing = true;
    IniHandler ini(settings, content);

    auto lines = split_lines(content);

    // Check each section for issues
    for (size_t line_idx = 0; line_idx < lines.size(); ++line_idx) {
        std::string trimmed = trim(lines[line_idx]);
        std::string lower = to_lower(trimmed);

        // Check for checktextureoverride triggers
        if (starts_with(lower, "checktextureoverride")) {
            IniValidationIssue issue;
            issue.file_path = file_path;
            issue.issue_type = "unwanted_trigger";
            issue.line_numbers.push_back((int)line_idx);
            issue.details = "checktextureoverride trigger found";
            unwanted_triggers_.push_back(issue);
        }
    }
}

void IniValidator::scan() {
    if (!directory_exists(folder_path_)) return;

    auto files = find_files_recursive(folder_path_, L".ini");

    for (auto& file_path : files) {
        if (is_excluded(file_path)) continue;

        std::wstring name = get_filename(file_path);
        std::wstring lower_name;
        for (auto c : name) lower_name += towlower(c);

        // Check for rogue d3dx.ini in subfolders
        if (lower_name == L"d3dx.ini") {
            rogue_ini_files_.push_back(file_path);
            continue;
        }

        // Check cache validity
        if (use_cache_ && !new_cache_) {
            auto it = cache_.find(file_path);
            if (it != cache_.end()) {
                uint64_t mtime = get_file_mtime(file_path);
                uint64_t size = get_file_size(file_path);
                if (it->second.mtime == mtime && it->second.size == size) {
                    // Use cached results
                    for (auto& issue : it->second.issues) {
                        if (issue.issue_type == "unwanted_trigger")
                            unwanted_triggers_.push_back(issue);
                        else if (issue.issue_type == "global_trigger")
                            global_triggers_.push_back(issue);
                        else if (issue.issue_type == "duplicate_library")
                            duplicate_libraries_.push_back(issue);
                    }
                    continue;
                }
            }
        }

        validate_file(file_path);

        // Update cache entry
        if (use_cache_) {
            CacheEntry ce;
            ce.mtime = get_file_mtime(file_path);
            ce.size = get_file_size(file_path);
            // Collect issues for this file
            for (auto& issue : unwanted_triggers_) {
                if (issue.file_path == file_path) ce.issues.push_back(issue);
            }
            for (auto& issue : global_triggers_) {
                if (issue.file_path == file_path) ce.issues.push_back(issue);
            }
            for (auto& issue : duplicate_libraries_) {
                if (issue.file_path == file_path) ce.issues.push_back(issue);
            }
            cache_[file_path] = ce;
        }
    }

    save_cache();
}

std::vector<std::wstring> IniValidator::get_rogue_ini_files() const {
    return rogue_ini_files_;
}

std::vector<IniValidationIssue> IniValidator::get_unwanted_triggers() const {
    return unwanted_triggers_;
}

std::vector<IniValidationIssue> IniValidator::get_global_triggers() const {
    return global_triggers_;
}

std::vector<IniValidationIssue> IniValidator::get_duplicate_libraries() const {
    return duplicate_libraries_;
}

} // namespace bridge
