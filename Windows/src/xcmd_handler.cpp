#include "xcmd_handler.h"
#include "path_utils.h"
#include "string_utils.h"
#include <stdexcept>

namespace bridge {

XcmdHandler::XcmdHandler(const std::wstring& xcmd_path) {
    std::string content = read_file_text(xcmd_path);
    if (content.empty()) return;
    IniHandlerSettings settings;
    settings.option_value_spacing = true;
    ini_ = std::make_unique<IniHandler>(settings, content);
}

void XcmdHandler::execute_section(const std::string& section_name,
                                   const std::wstring& importer_path) {
    if (!ini_) return;
    IniSection* sec = ini_->get_section(section_name);
    if (!sec) return;

    for (auto& opt : sec->options) {
        if (opt.name.empty()) continue;
        if (iequals(opt.name, "delete")) {
            cmd_delete(opt.value, importer_path);
        }
    }
}

void XcmdHandler::cmd_delete(const std::string& path, const std::wstring& importer_path) {
    // Security: filter dangerous path components
    if (path.find("..") != std::string::npos) return;
    if (path.find("./") != std::string::npos) return;
    if (path.find(".\\") != std::string::npos) return;

    // Only allow deletions within Core/ and ShaderFixes/
    std::string lower_path = to_lower(path);
    bool allowed = starts_with(lower_path, "core/") ||
                   starts_with(lower_path, "core\\") ||
                   starts_with(lower_path, "shaderfixes/") ||
                   starts_with(lower_path, "shaderfixes\\");
    if (!allowed) return;

    // Don't allow deleting the root directories themselves
    if (iequals(trim(path), "Core") || iequals(trim(path), "ShaderFixes")) return;

    std::wstring full_path = path_join(importer_path, utf8_to_wide(path));

    if (directory_exists(full_path)) {
        delete_directory_recursive(full_path);
    } else if (file_exists(full_path)) {
        delete_file(full_path);
    }
}

} // namespace bridge
