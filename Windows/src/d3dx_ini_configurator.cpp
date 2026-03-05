#include "d3dx_ini_configurator.h"
#include "path_utils.h"
#include "string_utils.h"
#include <stdexcept>

#ifdef _WIN32
#include <windows.h>
#endif

namespace bridge {

D3dxIniConfigurator::D3dxIniConfigurator(const BridgeConfig& config)
    : config_(config) {
    std::wstring ini_path = path_join(config.paths.importer_folder, L"d3dx.ini");
    std::string content = read_file_text(ini_path);
    if (content.empty()) {
        throw std::runtime_error("Failed to read d3dx.ini");
    }
    IniHandlerSettings settings;
    settings.ignore_comments = true;
    settings.option_value_spacing = true;
    ini_ = std::make_unique<IniHandler>(settings, content);
}

void D3dxIniConfigurator::update() {
    // Apply constant settings FIRST (e.g. "core", "enforce_rendering")
    // so that set_target_exe() can override the "loader" value from "core"
    // (which ships as "XXMI Launcher.exe" but must be "ssmt4-bridge.exe").
    for (auto& [setting_name, sections] : config_.d3dx_ini.settings) {
        // Check if this setting is a bool toggle driven by migoto config
        bool is_enabled = false;
        bool is_toggle = false;

        // Determine if this setting maps to a migoto bool config
        if (setting_name == "enforce_rendering") {
            is_toggle = false; // constant — always applied
        } else if (setting_name == "core") {
            is_toggle = false; // constant — always applied
        } else if (setting_name == "calls_logging") {
            is_toggle = true;
            is_enabled = config_.migoto.calls_logging;
        } else if (setting_name == "debug_logging") {
            is_toggle = true;
            is_enabled = config_.migoto.debug_logging;
        } else if (setting_name == "mute_warnings") {
            is_toggle = true;
            is_enabled = config_.migoto.mute_warnings;
        } else if (setting_name == "enable_hunting") {
            is_toggle = true;
            is_enabled = config_.migoto.enable_hunting;
        } else if (setting_name == "dump_shaders") {
            is_toggle = true;
            is_enabled = config_.migoto.dump_shaders;
        } else {
            // Unknown setting — check if any value is a toggle
            // If all values in all sections are toggles, treat as toggle
            bool has_toggles = false;
            for (auto& [sec_name, opts] : sections) {
                for (auto& [opt_name, val] : opts) {
                    if (val.is_toggle) has_toggles = true;
                }
            }
            if (has_toggles) {
                is_toggle = true;
                // Default off for unknown toggles
                is_enabled = false;
            }
        }

        // Apply the settings
        for (auto& [sec_name, opts] : sections) {
            for (auto& [opt_name, val] : opts) {
                if (val.is_toggle) {
                    ini_->set_option(sec_name, opt_name,
                                     is_enabled ? val.on_value : val.off_value);
                } else {
                    ini_->set_option(sec_name, opt_name, val.constant_value);
                }
            }
        }
    }

    // Override AFTER the settings loop so these values aren't overwritten
    // by the "core" config (which has loader = "XXMI Launcher.exe")
    set_target_exe();
    set_init_delay();
    set_screen_resolution();

    // Write back only if modified
    if (ini_->is_modified()) {
        std::wstring ini_path = path_join(config_.paths.importer_folder, L"d3dx.ini");
        write_file_text(ini_path, ini_->to_string());
    }
}

void D3dxIniConfigurator::set_target_exe() {
    std::string exe_name = wide_to_utf8(config_.paths.game_exe);
    if (!exe_name.empty()) {
        ini_->set_option("Loader", "target", exe_name);
    }
    // Set loader to our bridge exe name so 3DMigoto recognizes us as a valid loader
    // (d3dx.ini ships with "XXMI Launcher.exe" which won't match ssmt4-bridge.exe)
    ini_->set_option("Loader", "loader", "ssmt4-bridge.exe");
}

void D3dxIniConfigurator::set_init_delay() {
    // Original XXMI-Launcher sets this in [System], not [Loader].
    // The [System] section value is what d3d11.dll actually uses.
    ini_->set_option("System", "dll_initialization_delay",
                     config_.migoto.xxmi_dll_init_delay);
}

void D3dxIniConfigurator::set_screen_resolution() {
#ifdef _WIN32
    int width = GetSystemMetrics(SM_CXSCREEN);
    int height = GetSystemMetrics(SM_CYSCREEN);
    if (width > 0 && height > 0) {
        // Original XXMI-Launcher writes to [System] (monitor resolution fallback)
        ini_->set_option("System", "screen_width", width);
        ini_->set_option("System", "screen_height", height);
    }
#endif
}

} // namespace bridge
