#include "mod_manager.h"
#include "ini_validator.h"
#include "path_utils.h"
#include "string_utils.h"
#include <algorithm>

namespace bridge {

void ModManager::disable_file(const std::wstring& path) {
    std::wstring dir = get_parent_path(path);
    std::wstring name = get_filename(path);
    std::wstring new_name = L"DISABLED_" + name;
    std::wstring new_path = path_join(dir, new_name);

    // If DISABLED_ version already exists, delete it first
    if (file_exists(new_path)) {
        delete_file(new_path);
    }
    rename_path(path, new_path);
}

int ModManager::comment_out_lines(const std::wstring& path,
                                   const std::vector<int>& line_numbers) {
    if (line_numbers.empty()) return 0;

    std::string content = read_file_text(path);
    if (content.empty()) return 0;

    auto lines = split_lines(content);
    int count = 0;

    for (int idx : line_numbers) {
        if (idx < 0 || idx >= (int)lines.size()) continue;
        std::string& line = lines[idx];
        std::string trimmed = trim(line);
        // Skip if already commented
        if (!trimmed.empty() && trimmed[0] != ';' && trimmed[0] != '#') {
            line = "; " + line;
            count++;
        }
    }

    if (count > 0) {
        std::string result;
        for (size_t i = 0; i < lines.size(); ++i) {
            result += lines[i];
            if (i + 1 < lines.size()) result += "\n";
        }
        write_file_text(path, result);
    }
    return count;
}

ModOptimizationResult ModManager::optimize_mods_folder(
    const std::wstring& mods_path,
    const std::wstring& cache_path,
    const std::vector<std::string>& exclude_patterns,
    bool use_cache,
    bool reset_cache) {

    ModOptimizationResult result;
    if (!directory_exists(mods_path)) return result;

    IniValidator validator(mods_path, exclude_patterns, use_cache, reset_cache, cache_path);
    validator.scan();

    // Disable rogue d3dx.ini files
    for (auto& rogue_path : validator.get_rogue_ini_files()) {
        disable_file(rogue_path);
        result.disabled_files_count++;
    }

    // Comment out unwanted triggers (checktextureoverride)
    for (auto& issue : validator.get_unwanted_triggers()) {
        int commented = comment_out_lines(issue.file_path, issue.line_numbers);
        if (commented > 0) {
            result.edited_files_count++;
            result.edited_lines_count += commented;
        }
    }

    // Disable files with global triggers
    for (auto& issue : validator.get_global_triggers()) {
        disable_file(issue.file_path);
        result.disabled_files_count++;
    }

    // Disable duplicate libraries
    for (auto& issue : validator.get_duplicate_libraries()) {
        disable_file(issue.file_path);
        result.disabled_files_count++;
    }

    return result;
}

ModOptimizationResult ModManager::optimize_shaderfixes_folder(
    const std::wstring& shaderfixes_path,
    const std::vector<std::string>& exclude_patterns) {

    ModOptimizationResult result;
    if (!directory_exists(shaderfixes_path)) return result;

    // In ShaderFixes, only look for rogue d3dx.ini files
    auto files = find_files_recursive(shaderfixes_path, L".ini");
    for (auto& file_path : files) {
        std::wstring name = get_filename(file_path);
        std::wstring lower_name;
        for (auto c : name) lower_name += towlower(c);

        if (lower_name == L"d3dx.ini") {
            disable_file(file_path);
            result.disabled_files_count++;
        }
    }

    return result;
}

} // namespace bridge
