#pragma once

#include "ini_handler.h"
#include <string>
#include <memory>

namespace bridge {

class XcmdHandler {
public:
    explicit XcmdHandler(const std::wstring& xcmd_path);

    void execute_section(const std::string& section_name,
                         const std::wstring& importer_path);

private:
    std::unique_ptr<IniHandler> ini_;

    void cmd_delete(const std::string& path, const std::wstring& importer_path);
};

} // namespace bridge
