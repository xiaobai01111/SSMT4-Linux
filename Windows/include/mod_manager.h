#pragma once

#include "bridge_config.h"
#include <string>
#include <vector>

namespace bridge {

struct ModOptimizationResult {
    int disabled_mods_count = 0;
    int disabled_files_count = 0;
    int edited_files_count = 0;
    int edited_lines_count = 0;
};

class ModManager {
public:
    ModOptimizationResult optimize_mods_folder(
        const std::wstring& mods_path,
        const std::wstring& cache_path,
        const std::vector<std::string>& exclude_patterns,
        bool use_cache = true,
        bool reset_cache = false);

    ModOptimizationResult optimize_shaderfixes_folder(
        const std::wstring& shaderfixes_path,
        const std::vector<std::string>& exclude_patterns);

private:
    void disable_file(const std::wstring& path);
    int comment_out_lines(const std::wstring& path,
                          const std::vector<int>& line_numbers);
};

} // namespace bridge
