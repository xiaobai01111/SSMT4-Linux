#include "wwmi_initializer.h"
#include "local_storage.h"
#include "ini_handler.h"
#include "path_utils.h"
#include "string_utils.h"
#include <algorithm>
#include <vector>

namespace bridge {

void WWMIInitializer::initialize(const BridgeConfig& config, StatusReporter& reporter) {
    bool configure_game = config.game_specific.get_bool("configure_game", false);

    if (configure_game) {
        reporter.status("Configuring WWMI game settings (SQLite)...");
        configure_settings(config, reporter);

        reporter.status("Updating Engine.ini...");
        update_engine_ini(config, reporter);

        reporter.status("Updating GameUserSettings.ini...");
        update_game_user_settings_ini(config, reporter);

        reporter.status("Updating DeviceProfiles.ini...");
        update_device_profiles_ini(config, reporter);
    }
}

void WWMIInitializer::configure_settings(const BridgeConfig& config, StatusReporter& reporter) {
    // Path: game_folder/Client/Saved/LocalStorage/
    std::wstring ls_dir = path_join(
        path_join(path_join(config.paths.game_folder, L"Client"), L"Saved"),
        L"LocalStorage");
    ensure_directory(ls_dir);

    // Find the most recent LocalStorage*.db file
    auto entries = list_directory(ls_dir);
    std::wstring active_db;
    uint64_t newest_mtime = 0;

    for (auto& entry : entries) {
        std::wstring name = get_filename(entry);
        std::wstring lower_name;
        for (auto c : name) lower_name += towlower(c);

        if (starts_with(wide_to_utf8(lower_name), "localstorage") &&
            ends_with(wide_to_utf8(lower_name), ".db")) {
            uint64_t mtime = get_file_mtime(entry);
            if (mtime > newest_mtime) {
                newest_mtime = mtime;
                active_db = entry;
            }
        }
    }

    // Clean up extra db files, keep only the active one
    for (auto& entry : entries) {
        std::wstring name = get_filename(entry);
        std::wstring lower_name;
        for (auto c : name) lower_name += towlower(c);

        if (starts_with(wide_to_utf8(lower_name), "localstorage") &&
            ends_with(wide_to_utf8(lower_name), ".db") &&
            entry != active_db) {
            delete_file(entry);
        }
    }

    // Rename to LocalStorage.db if needed
    std::wstring default_db = path_join(ls_dir, L"LocalStorage.db");
    if (!active_db.empty() && active_db != default_db) {
        if (file_exists(default_db)) {
            delete_file(default_db);
        }
        rename_path(active_db, default_db);
    }

    // Open SQLite database
    LocalStorage db(default_db);
    db.connect();

    bool unlock_fps = config.game_specific.get_bool("unlock_fps", false);
    if (unlock_fps) {
        reporter.log("info", "Unlocking FPS to 120");
        db.set_fps(120);
    } else {
        db.reset_fps_triggers();
    }

    bool force_max_lod_bias = config.game_specific.get_bool("force_max_lod_bias", false);
    if (force_max_lod_bias) {
        db.set_value("ImageDetail", "3");
    }

    // Ray Tracing Off (required for 3DMigoto compatibility)
    db.set_value("RayTracing", "0");
    db.set_value("RayTracedReflection", "0");
    db.set_value("RayTracedGI", "0");

    // Wounded FX setting
    bool disable_wounded_fx = config.game_specific.get_bool("disable_wounded_fx", false);
    db.set_value("SkinDamageMode", disable_wounded_fx ? "0" : "1");

    db.save();
}

void WWMIInitializer::update_engine_ini(const BridgeConfig& config, StatusReporter& reporter) {
    // Path: game_folder/Client/Saved/Config/WindowsNoEditor/Engine.ini
    std::wstring ini_path = path_join(
        path_join(path_join(path_join(path_join(
            config.paths.game_folder, L"Client"), L"Saved"), L"Config"),
            L"WindowsNoEditor"),
        L"Engine.ini");

    if (!file_exists(ini_path)) {
        // Create the file with empty content
        ensure_directory(get_parent_path(ini_path));
        write_file_text(ini_path, "");
    }

    std::string content = read_file_text(ini_path);
    IniHandlerSettings settings;
    settings.option_value_spacing = false; // UE4 INI uses key=value without spaces
    IniHandler ini(settings, content);

    bool apply_perf_tweaks = config.game_specific.get_bool("apply_perf_tweaks", false);

    if (apply_perf_tweaks) {
        // Apply performance tweaks from game_specific.ini_updates
        auto it = config.game_specific.ini_updates.find("perf_tweaks");
        if (it != config.game_specific.ini_updates.end()) {
            for (auto& [section, options] : it->second) {
                for (auto& [key, value] : options) {
                    ini.set_option(section, key, value);
                }
            }
        }
    }

    if (ini.is_modified()) {
        write_file_text(ini_path, ini.to_string());
    }
}

void WWMIInitializer::update_game_user_settings_ini(const BridgeConfig& config,
                                                      StatusReporter& reporter) {
    // Path: game_folder/Client/Saved/Config/WindowsNoEditor/GameUserSettings.ini
    std::wstring ini_path = path_join(
        path_join(path_join(path_join(path_join(
            config.paths.game_folder, L"Client"), L"Saved"), L"Config"),
            L"WindowsNoEditor"),
        L"GameUserSettings.ini");

    if (!file_exists(ini_path)) return;

    std::string content = read_file_text(ini_path);
    IniHandlerSettings settings;
    settings.option_value_spacing = false;
    IniHandler ini(settings, content);

    bool unlock_fps = config.game_specific.get_bool("unlock_fps", false);
    if (unlock_fps) {
        ini.set_option("/Script/Engine.GameUserSettings", "FrameRateLimit", "120.000000");
    }

    if (ini.is_modified()) {
        write_file_text(ini_path, ini.to_string());
    }
}

void WWMIInitializer::update_device_profiles_ini(const BridgeConfig& config,
                                                   StatusReporter& reporter) {
    // Path: game_folder/Client/Saved/Config/WindowsNoEditor/DeviceProfiles.ini
    std::wstring ini_path = path_join(
        path_join(path_join(path_join(path_join(
            config.paths.game_folder, L"Client"), L"Saved"), L"Config"),
            L"WindowsNoEditor"),
        L"DeviceProfiles.ini");

    if (!file_exists(ini_path)) {
        ensure_directory(get_parent_path(ini_path));
        write_file_text(ini_path, "");
    }

    std::string content = read_file_text(ini_path);
    IniHandlerSettings settings;
    settings.option_value_spacing = false;
    settings.right_split = true; // UE4 CVars use key=value=more format
    IniHandler ini(settings, content);

    // Device profile quality levels to configure
    // These correspond to the 6 quality levels in UE4
    std::vector<std::string> quality_sections = {
        "Windows DeviceProfile 0",
        "Windows DeviceProfile 1",
        "Windows DeviceProfile 2",
        "Windows DeviceProfile 3",
        "Windows DeviceProfile 4",
        "Windows DeviceProfile 5",
    };

    int mesh_lod_offset = config.game_specific.get_int("mesh_lod_distance_offset", -10);
    double streaming_boost = config.game_specific.get_double("texture_streaming_boost", 20.0);
    double streaming_min_boost = config.game_specific.get_double("texture_streaming_min_boost", 0.0);
    bool use_all_mips = config.game_specific.get_bool("texture_streaming_use_all_mips", true);
    int pool_size = config.game_specific.get_int("texture_streaming_pool_size", 0);
    bool limit_to_vram = config.game_specific.get_bool("texture_streaming_limit_to_vram", true);
    bool fixed_pool = config.game_specific.get_bool("texture_streaming_fixed_pool_size", true);

    for (auto& section : quality_sections) {
        // First remove existing CVars for these keys to avoid duplicates
        ini.remove_option("CVars", section,
                          "r.Kuro.SkeletalMesh.LODDistanceScaleDeviceOffset", false);
        ini.remove_option("CVars", section, "r.Streaming.Boost", false);
        ini.remove_option("CVars", section, "r.Streaming.MinBoost", false);
        ini.remove_option("CVars", section, "r.Streaming.UseAllMips", false);
        ini.remove_option("CVars", section, "r.Streaming.PoolSize", false);
        ini.remove_option("CVars", section, "r.Streaming.LimitPoolSizeToVRAM", false);
        ini.remove_option("CVars", section, "r.Streaming.UseFixedPoolSize", false);

        // Set new values as CVars=key=value format
        char buf[128];
        std::snprintf(buf, sizeof(buf), "r.Kuro.SkeletalMesh.LODDistanceScaleDeviceOffset=%d",
                       mesh_lod_offset);
        ini.set_option(section, "CVars", buf);

        std::snprintf(buf, sizeof(buf), "r.Streaming.Boost=%g", streaming_boost);
        ini.set_option(section, "+CVars", buf);

        std::snprintf(buf, sizeof(buf), "r.Streaming.MinBoost=%g", streaming_min_boost);
        ini.set_option(section, "+CVars", buf);

        std::snprintf(buf, sizeof(buf), "r.Streaming.UseAllMips=%d", use_all_mips ? 1 : 0);
        ini.set_option(section, "+CVars", buf);

        std::snprintf(buf, sizeof(buf), "r.Streaming.PoolSize=%d", pool_size);
        ini.set_option(section, "+CVars", buf);

        std::snprintf(buf, sizeof(buf), "r.Streaming.LimitPoolSizeToVRAM=%d",
                       limit_to_vram ? 1 : 0);
        ini.set_option(section, "+CVars", buf);

        std::snprintf(buf, sizeof(buf), "r.Streaming.UseFixedPoolSize=%d", fixed_pool ? 1 : 0);
        ini.set_option(section, "+CVars", buf);
    }

    if (ini.is_modified()) {
        write_file_text(ini_path, ini.to_string());
    }
}

} // namespace bridge
