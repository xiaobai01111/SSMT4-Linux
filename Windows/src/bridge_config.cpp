#include "bridge_config.h"
#include "bridge_contract_generated.h"
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

static cJSON* require_object(const cJSON* obj, const char* key) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsObject(item)) {
        return item;
    }
    throw std::runtime_error(std::string("bridge-config.json missing object field: ") + key);
}

static std::string require_json_str(const cJSON* obj, const char* key) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsString(item) && item->valuestring) {
        return item->valuestring;
    }
    throw std::runtime_error(std::string("bridge-config.json missing string field: ") + key);
}

static bool require_json_bool(const cJSON* obj, const char* key) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsBool(item)) {
        return cJSON_IsTrue(item);
    }
    throw std::runtime_error(std::string("bridge-config.json missing bool field: ") + key);
}

static int require_json_int(const cJSON* obj, const char* key) {
    cJSON* item = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (item && cJSON_IsNumber(item)) {
        return item->valueint;
    }
    throw std::runtime_error(std::string("bridge-config.json missing integer field: ") + key);
}

static std::wstring require_json_wstr(const cJSON* obj, const char* key) {
    return utf8_to_wide(require_json_str(obj, key));
}

static std::vector<std::wstring> require_json_wstr_array(const cJSON* obj, const char* key) {
    cJSON* arr = cJSON_GetObjectItemCaseSensitive(obj, key);
    if (!arr || !cJSON_IsArray(arr)) {
        throw std::runtime_error(std::string("bridge-config.json missing array field: ") + key);
    }

    std::vector<std::wstring> result;
    cJSON* elem = nullptr;
    cJSON_ArrayForEach(elem, arr) {
        if (!cJSON_IsString(elem) || !elem->valuestring) {
            throw std::runtime_error(
                std::string("bridge-config.json field contains non-string array item: ") + key);
        }
        result.push_back(utf8_to_wide(elem->valuestring));
    }
    return result;
}

static bool matches_field_kind(const cJSON* item, bridge_contract::FieldKind kind) {
    switch (kind) {
    case bridge_contract::FieldKind::String:
        return item && cJSON_IsString(item) && item->valuestring;
    case bridge_contract::FieldKind::Boolean:
        return item && cJSON_IsBool(item);
    case bridge_contract::FieldKind::Int:
        return item && cJSON_IsNumber(item);
    case bridge_contract::FieldKind::StringArray:
        if (!item || !cJSON_IsArray(item)) return false;
        {
            cJSON* elem = nullptr;
            cJSON_ArrayForEach(elem, item) {
                if (!cJSON_IsString(elem) || !elem->valuestring) {
                    return false;
                }
            }
        }
        return true;
    case bridge_contract::FieldKind::Object:
        return item && cJSON_IsObject(item);
    }
    return false;
}

static std::string contract_path(const char* section, const char* field) {
    if (!section || std::strcmp(section, field) == 0) {
        return field ? field : "";
    }
    return std::string(section) + "." + field;
}

static void validate_section_contract(
    const cJSON* root,
    const bridge_contract::SectionContract& section_contract
) {
    const cJSON* section = section_contract.name
        ? cJSON_GetObjectItemCaseSensitive(root, section_contract.name)
        : root;
    if (!section) {
        throw std::runtime_error(
            std::string("bridge-config.json missing section: ") + section_contract.name);
    }

    for (std::size_t i = 0; i < section_contract.field_count; ++i) {
        const auto& field = section_contract.fields[i];
        const cJSON* item = (section_contract.name && std::strcmp(section_contract.name, field.name) != 0)
            ? cJSON_GetObjectItemCaseSensitive(section, field.name)
            : section;
        if (!matches_field_kind(item, field.kind)) {
            throw std::runtime_error(
                "bridge-config.json field has invalid type: " +
                contract_path(section_contract.name, field.name));
        }
    }
}

static void validate_bridge_contract(const cJSON* root) {
    for (std::size_t i = 0; i < bridge_contract::kBridgeConfigContractCount; ++i) {
        validate_section_contract(root, bridge_contract::kBridgeConfigContract[i]);
    }
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
    cJSON* d3dx = cJSON_GetObjectItemCaseSensitive(root, bridge_contract::sections::kD3dxIni);
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
    validate_bridge_contract(root);

    // Top-level
    cfg.schema_version = require_json_int(root, bridge_contract::root::kSchemaVersion);
    if (cfg.schema_version != 1) {
        cJSON_Delete(root);
        throw std::runtime_error(
            "Unsupported bridge-config.json schemaVersion: " +
            std::to_string(cfg.schema_version));
    }
    cfg.importer = require_json_str(root, bridge_contract::root::kImporter);

    // Paths — all provided by frontend, per-game isolation
    cJSON* paths = require_object(root, bridge_contract::sections::kPaths);
    cfg.paths.app_root        = require_json_wstr(paths, bridge_contract::paths::kAppRoot);
    cfg.paths.importer_folder = require_json_wstr(paths, bridge_contract::paths::kImporterFolder);
    cfg.paths.packages_folder = require_json_wstr(paths, bridge_contract::paths::kPackagesFolder);
    cfg.paths.game_folder     = require_json_wstr(paths, bridge_contract::paths::kGameFolder);
    cfg.paths.game_exe        = require_json_wstr(paths, bridge_contract::paths::kGameExe);
    cfg.paths.cache_folder    = require_json_wstr(paths, bridge_contract::paths::kCacheFolder);

    // Game launch settings — all from frontend
    cJSON* game = require_object(root, bridge_contract::sections::kGame);
    cfg.game.start_exe            = require_json_wstr(game, bridge_contract::game::kStartExe);
    cfg.game.start_args           = require_json_wstr_array(game, bridge_contract::game::kStartArgs);
    cfg.game.work_dir             = require_json_wstr(game, bridge_contract::game::kWorkDir);
    cfg.game.process_name         = require_json_wstr(game, bridge_contract::game::kProcessName);
    cfg.game.process_start_method = require_json_str(game, bridge_contract::game::kProcessStartMethod);
    cfg.game.process_priority     = require_json_str(game, bridge_contract::game::kProcessPriority);
    cfg.game.process_timeout      = require_json_int(game, bridge_contract::game::kProcessTimeout);

    // Migoto injection settings
    cJSON* migoto = require_object(root, bridge_contract::sections::kMigoto);
    cfg.migoto.use_hook            = require_json_bool(migoto, bridge_contract::migoto::kUseHook);
    cfg.migoto.use_dll_drop        = require_json_bool(migoto, bridge_contract::migoto::kUseDllDrop);
    cfg.migoto.enforce_rendering   = require_json_bool(migoto, bridge_contract::migoto::kEnforceRendering);
    cfg.migoto.enable_hunting      = require_json_bool(migoto, bridge_contract::migoto::kEnableHunting);
    cfg.migoto.dump_shaders        = require_json_bool(migoto, bridge_contract::migoto::kDumpShaders);
    cfg.migoto.mute_warnings       = require_json_bool(migoto, bridge_contract::migoto::kMuteWarnings);
    cfg.migoto.calls_logging       = require_json_bool(migoto, bridge_contract::migoto::kCallsLogging);
    cfg.migoto.debug_logging       = require_json_bool(migoto, bridge_contract::migoto::kDebugLogging);
    cfg.migoto.unsafe_mode         = require_json_bool(migoto, bridge_contract::migoto::kUnsafeMode);
    cfg.migoto.xxmi_dll_init_delay = require_json_int(migoto, bridge_contract::migoto::kXxmiDllInitDelay);

    // Game-specific config (generic key-value, not hardcoded per game)
    cfg.game_specific = parse_game_specific(root, cfg.importer);

    // d3dx.ini config mappings
    cfg.d3dx_ini = parse_d3dx_ini(root);

    // Signatures
    cJSON* sigs = require_object(root, bridge_contract::sections::kSignatures);
    cfg.signatures.xxmi_public_key = require_json_str(sigs, bridge_contract::signatures::kXxmiPublicKey);
    cJSON* deployed = require_object(sigs, bridge_contract::signatures::kDeployedMigotoSignatures);
    cJSON* sig = nullptr;
    cJSON_ArrayForEach(sig, deployed) {
        if (cJSON_IsString(sig) && sig->string) {
            cfg.signatures.deployed_migoto_signatures[sig->string] =
                sig->valuestring ? sig->valuestring : "";
        }
    }

    // Extra libraries
    cJSON* extra = require_object(root, bridge_contract::sections::kExtraLibraries);
    cfg.extra_libraries.enabled = require_json_bool(extra, bridge_contract::extra_libraries::kEnabled);
    cfg.extra_libraries.paths   = require_json_wstr_array(extra, bridge_contract::extra_libraries::kPaths);

    // Custom launch
    cJSON* custom = require_object(root, bridge_contract::sections::kCustomLaunch);
    cfg.custom_launch.enabled     = require_json_bool(custom, bridge_contract::custom_launch::kEnabled);
    cfg.custom_launch.cmd         = require_json_str(custom, bridge_contract::custom_launch::kCmd);
    cfg.custom_launch.inject_mode = require_json_str(custom, bridge_contract::custom_launch::kInjectMode);

    // Pre-launch
    cJSON* pre = require_object(root, bridge_contract::sections::kPreLaunch);
    cfg.pre_launch.enabled = require_json_bool(pre, bridge_contract::pre_launch::kEnabled);
    cfg.pre_launch.cmd     = require_json_str(pre, bridge_contract::pre_launch::kCmd);
    cfg.pre_launch.wait    = require_json_bool(pre, bridge_contract::pre_launch::kWait);

    // Post-load
    cJSON* post = require_object(root, bridge_contract::sections::kPostLoad);
    cfg.post_load.enabled = require_json_bool(post, bridge_contract::post_load::kEnabled);
    cfg.post_load.cmd     = require_json_str(post, bridge_contract::post_load::kCmd);
    cfg.post_load.wait    = require_json_bool(post, bridge_contract::post_load::kWait);

    // Jadeite anti-cheat bypass (for miHoYo games on Linux/Proton)
    cJSON* jadeite = require_object(root, bridge_contract::sections::kJadeite);
    cfg.jadeite.enabled  = require_json_bool(jadeite, bridge_contract::jadeite::kEnabled);
    cfg.jadeite.exe_path = require_json_wstr(jadeite, bridge_contract::jadeite::kExePath);

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
