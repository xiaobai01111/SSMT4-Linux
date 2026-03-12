#include "bridge_config.h"
#include "path_utils.h"
#include "string_utils.h"
#include <cJSON.h>
#include <stdexcept>
#include <cstring>

namespace bridge {

// Helper: get a JSON string or default
static std::string json_str(const cJSON* obj, const char* key, const char* def = "") {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsString(item) && item->valuestring) return item->valuestring;
    return def;
}

static bool json_bool(const cJSON* obj, const char* key, bool def = false) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsBool(item)) return cJSON_IsTrue(item);
    return def;
}

static int json_int(const cJSON* obj, const char* key, int def = 0) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsNumber(item)) return item->valueint;
    return def;
}

static double json_double(const cJSON* obj, const char* key, double def = 0.0) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsNumber(item)) return item->valuedouble;
    return def;
}

static std::wstring json_wstr(const cJSON* obj, const char* key, const wchar_t* def = L"") {
    std::string s = json_str(obj, key, "");
    if (s.empty()) return def;
    return utf8_to_wide(s);
}

static std::vector<std::wstring> json_wstr_array(const cJSON* obj, const char* key) {
    std::vector<std::wstring> result;
    cJSON* arr = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (arr && cJSON_IsArray(arr)) {
        cJSON* elem = nullptr;
        cJSON_ArrayForEach(elem, arr) {
            if (cJSON_IsString(elem) && elem->valuestring) {
                result.push_back(utf8_to_wide(elem->valuestring));
            }
        }
    }
    return result;
}

// Convert a cJSON value to string representation
static std::string json_value_to_string(const cJSON* item) {
    if (!item) return "";
    if (cJSON_IsString(item)) return item->valuestring ? item->valuestring : "";
    if (cJSON_IsNumber(item)) {
        // Preserve integer formatting when possible
        if (item->valuedouble == (double)item->valueint &&
            item->valuedouble >= -2147483648.0 && item->valuedouble <= 2147483647.0) {
            return std::to_string(item->valueint);
        }
        char buf[64];
        std::snprintf(buf, sizeof(buf), "%g", item->valuedouble);
        return buf;
    }
    if (cJSON_IsBool(item)) return cJSON_IsTrue(item) ? "1" : "0";
    return "";
}

// Parse D3dxIniConfig from JSON
static D3dxIniConfig parse_d3dx_ini(const cJSON* root) {
    D3dxIniConfig config;
    cJSON* d3dx = cJSON_GetObjectItemCaseSensitive(root, "d3dx_ini");
    if (!d3dx || !cJSON_IsObject(d3dx)) return config;

    // Iterate: setting_name -> { section -> { option -> value_or_toggle } }
    cJSON* setting = nullptr;
    cJSON_ArrayForEach(setting, d3dx) {
        if (!cJSON_IsObject(setting)) continue;
        std::string setting_name = setting->string ? setting->string : "";

        std::map<std::string, std::map<std::string, D3dxIniValue>> sections;
        cJSON* section = nullptr;
        cJSON_ArrayForEach(section, setting) {
            if (!cJSON_IsObject(section)) continue;
            std::string section_name = section->string ? section->string : "";

            std::map<std::string, D3dxIniValue> options;
            cJSON* option = nullptr;
            cJSON_ArrayForEach(option, section) {
                std::string option_name = option->string ? option->string : "";
                D3dxIniValue val;

                if (cJSON_IsObject(option)) {
                    // Toggle: {"on": x, "off": y}
                    val.is_toggle = true;
                    cJSON* on_item = cJSON_GetObjectItemCaseSensitive(option, "on");
                    cJSON* off_item = cJSON_GetObjectItemCaseSensitive(option, "off");
                    val.on_value = json_value_to_string(on_item);
                    val.off_value = json_value_to_string(off_item);
                } else {
                    // Constant value
                    val.is_toggle = false;
                    val.constant_value = json_value_to_string(option);
                }
                options[option_name] = val;
            }
            sections[section_name] = options;
        }
        config.settings[setting_name] = sections;
    }
    return config;
}

// Parse GameSpecificConfig from JSON
static GameSpecificConfig parse_game_specific(const cJSON* root, const std::string& importer) {
    GameSpecificConfig config;

    // Look for the importer-specific section (e.g. "wwmi", "zzmi", "gimi")
    std::string lower_name = to_lower(importer);
    cJSON* gs = cJSON_GetObjectItemCaseSensitive(root, lower_name.c_str());
    if (!gs || !cJSON_IsObject(gs)) return config;

    cJSON* item = nullptr;
    cJSON_ArrayForEach(item, gs) {
        std::string key = item->string ? item->string : "";
        if (cJSON_IsObject(item)) {
            // Nested object — could be ini_updates (section -> { key -> val })
            std::map<std::string, std::map<std::string, std::string>> nested;
            cJSON* section = nullptr;
            cJSON_ArrayForEach(section, item) {
                if (!cJSON_IsObject(section)) continue;
                std::string sec_name = section->string ? section->string : "";
                std::map<std::string, std::string> sec_map;
                cJSON* opt = nullptr;
                cJSON_ArrayForEach(opt, section) {
                    sec_map[opt->string ? opt->string : ""] = json_value_to_string(opt);
                }
                nested[sec_name] = sec_map;
            }
            config.ini_updates[key] = nested;
        } else {
            // Flat value
            config.settings[key] = json_value_to_string(item);
        }
    }
    return config;
}

BridgeConfig BridgeConfig::load(const std::wstring& json_path) {
    std::string content = read_file_text(json_path);
    if (content.empty()) {
        throw std::runtime_error("Failed to read bridge-config.json or file is empty");
    }

    cJSON* root = cJSON_Parse(content.c_str());
    if (!root) {
        const char* err = cJSON_GetErrorPtr();
        throw std::runtime_error(std::string("JSON parse error: ") + (err ? err : "unknown"));
    }

    BridgeConfig cfg;

    // Top-level
    cfg.schema_version = json_int(root, "schemaVersion", 1);
    if (cfg.schema_version != 1) {
        cJSON_Delete(root);
        throw std::runtime_error(
            "Unsupported bridge-config.json schemaVersion: " +
            std::to_string(cfg.schema_version));
    }
    cfg.importer = json_str(root, "importer", "");

    // Paths — all provided by frontend, per-game isolation
    cJSON* paths = cJSON_GetObjectItemCaseSensitive(root, "paths");
    if (paths) {
        cfg.paths.app_root        = json_wstr(paths, "app_root");
        cfg.paths.importer_folder = json_wstr(paths, "importer_folder");
        cfg.paths.packages_folder = json_wstr(paths, "packages_folder");
        cfg.paths.game_folder     = json_wstr(paths, "game_folder");
        cfg.paths.game_exe        = json_wstr(paths, "game_exe");
        cfg.paths.cache_folder    = json_wstr(paths, "cache_folder");
    }

    // Game launch settings — all from frontend
    cJSON* game = cJSON_GetObjectItemCaseSensitive(root, "game");
    if (game) {
        cfg.game.start_exe            = json_wstr(game, "start_exe");
        cfg.game.start_args           = json_wstr_array(game, "start_args");
        cfg.game.work_dir             = json_wstr(game, "work_dir");
        cfg.game.process_name         = json_wstr(game, "process_name");
        cfg.game.process_start_method = json_str(game, "process_start_method", "Native");
        cfg.game.process_priority     = json_str(game, "process_priority", "Normal");
        cfg.game.process_timeout      = json_int(game, "process_timeout", 30);
    }

    // Migoto injection settings
    cJSON* migoto = cJSON_GetObjectItemCaseSensitive(root, "migoto");
    if (migoto) {
        cfg.migoto.use_hook           = json_bool(migoto, "use_hook", true);
        cfg.migoto.use_dll_drop       = json_bool(migoto, "use_dll_drop", false);
        cfg.migoto.enforce_rendering  = json_bool(migoto, "enforce_rendering", true);
        cfg.migoto.enable_hunting     = json_bool(migoto, "enable_hunting", false);
        cfg.migoto.dump_shaders       = json_bool(migoto, "dump_shaders", false);
        cfg.migoto.mute_warnings      = json_bool(migoto, "mute_warnings", true);
        cfg.migoto.calls_logging      = json_bool(migoto, "calls_logging", false);
        cfg.migoto.debug_logging      = json_bool(migoto, "debug_logging", false);
        cfg.migoto.unsafe_mode        = json_bool(migoto, "unsafe_mode", false);
        cfg.migoto.xxmi_dll_init_delay = json_int(migoto, "xxmi_dll_init_delay", 500);
    }

    // Game-specific config (generic key-value, not hardcoded per game)
    cfg.game_specific = parse_game_specific(root, cfg.importer);

    // d3dx.ini config mappings
    cfg.d3dx_ini = parse_d3dx_ini(root);

    // Signatures
    cJSON* sigs = cJSON_GetObjectItemCaseSensitive(root, "signatures");
    if (sigs) {
        cfg.signatures.xxmi_public_key = json_str(sigs, "xxmi_public_key", "");
        cJSON* deployed = cJSON_GetObjectItemCaseSensitive(sigs, "deployed_migoto_signatures");
        if (deployed && cJSON_IsObject(deployed)) {
            cJSON* sig = nullptr;
            cJSON_ArrayForEach(sig, deployed) {
                if (cJSON_IsString(sig) && sig->string) {
                    cfg.signatures.deployed_migoto_signatures[sig->string] =
                        sig->valuestring ? sig->valuestring : "";
                }
            }
        }
    }

    // Extra libraries
    cJSON* extra = cJSON_GetObjectItemCaseSensitive(root, "extra_libraries");
    if (extra) {
        cfg.extra_libraries.enabled = json_bool(extra, "enabled", false);
        cfg.extra_libraries.paths   = json_wstr_array(extra, "paths");
    }

    // Custom launch
    cJSON* custom = cJSON_GetObjectItemCaseSensitive(root, "custom_launch");
    if (custom) {
        cfg.custom_launch.enabled     = json_bool(custom, "enabled", false);
        cfg.custom_launch.cmd         = json_str(custom, "cmd", "");
        cfg.custom_launch.inject_mode = json_str(custom, "inject_mode", "Hook");
    }

    // Pre-launch
    cJSON* pre = cJSON_GetObjectItemCaseSensitive(root, "pre_launch");
    if (pre) {
        cfg.pre_launch.enabled = json_bool(pre, "enabled", false);
        cfg.pre_launch.cmd     = json_str(pre, "cmd", "");
        cfg.pre_launch.wait    = json_bool(pre, "wait", true);
    }

    // Post-load
    cJSON* post = cJSON_GetObjectItemCaseSensitive(root, "post_load");
    if (post) {
        cfg.post_load.enabled = json_bool(post, "enabled", false);
        cfg.post_load.cmd     = json_str(post, "cmd", "");
        cfg.post_load.wait    = json_bool(post, "wait", true);
    }

    // Jadeite anti-cheat bypass (for miHoYo games on Linux/Proton)
    cJSON* jadeite = cJSON_GetObjectItemCaseSensitive(root, "jadeite");
    if (jadeite) {
        cfg.jadeite.enabled  = json_bool(jadeite, "enabled", false);
        cfg.jadeite.exe_path = json_wstr(jadeite, "exe_path");
    }

    cJSON_Delete(root);
    return cfg;
}

// GameSpecificConfig accessors
bool GameSpecificConfig::get_bool(const std::string& key, bool default_val) const {
    auto it = settings.find(key);
    if (it == settings.end()) return default_val;
    const std::string& v = it->second;
    return v == "true" || v == "1" || v == "yes";
}

int GameSpecificConfig::get_int(const std::string& key, int default_val) const {
    auto it = settings.find(key);
    if (it == settings.end()) return default_val;
    try { return std::stoi(it->second); } catch (...) { return default_val; }
}

double GameSpecificConfig::get_double(const std::string& key, double default_val) const {
    auto it = settings.find(key);
    if (it == settings.end()) return default_val;
    try { return std::stod(it->second); } catch (...) { return default_val; }
}

std::string GameSpecificConfig::get_string(const std::string& key, const std::string& default_val) const {
    auto it = settings.find(key);
    if (it == settings.end()) return default_val;
    return it->second;
}

} // namespace bridge
