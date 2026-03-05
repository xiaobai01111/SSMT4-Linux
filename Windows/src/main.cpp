#include "bridge_config.h"
#include "status_reporter.h"
#include "path_utils.h"
#include "string_utils.h"
#include "d3dx_ini_configurator.h"
#include "mod_manager.h"
#include "xcmd_handler.h"
#include "game_initializer.h"
#include "package_deployer.h"
#include "dll_injector.h"
#include "process_tracker.h"

#include <cstdio>
#include <cstdlib>
#include <string>
#include <stdexcept>

#ifdef _WIN32
#include <windows.h>
#endif

using namespace bridge;

static void run_shell_command(const std::string& cmd, bool wait) {
#ifdef _WIN32
    STARTUPINFOA si = {};
    si.cb = sizeof(si);
    PROCESS_INFORMATION pi = {};

    std::string full_cmd = "cmd.exe /C " + cmd;
    if (CreateProcessA(nullptr, const_cast<char*>(full_cmd.c_str()),
                       nullptr, nullptr, FALSE, 0, nullptr, nullptr, &si, &pi)) {
        if (wait) {
            WaitForSingleObject(pi.hProcess, INFINITE);
        }
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
    }
#endif
}

static std::wstring parse_config_path(int argc, char* argv[]) {
    for (int i = 1; i < argc - 1; ++i) {
        if (std::string(argv[i]) == "--config") {
            return utf8_to_wide(argv[i + 1]);
        }
    }
    // If no --config flag, check if first arg is the path
    if (argc >= 2 && std::string(argv[1]) != "--config") {
        return utf8_to_wide(argv[1]);
    }
    return L"";
}

int main(int argc, char* argv[]) {
    StatusReporter reporter;

    try {
        // 1. Parse command line to get bridge-config.json path
        std::wstring config_path = parse_config_path(argc, argv);
        if (config_path.empty()) {
            reporter.error("NO_CONFIG", "Usage: ssmt4-bridge.exe --config <path-to-bridge-config.json>");
            return 1;
        }

        // 2. Load configuration — everything comes from this file, nothing hardcoded
        reporter.status("Loading configuration...");
        BridgeConfig config = BridgeConfig::load(config_path);

        // Enable file logging: write all JSON output to bridge-output.log
        // next to bridge-config.json. This is the fallback when Proton
        // swallows stdout from Windows console applications.
        {
            std::wstring config_dir = config.paths.cache_folder;
            if (!config_dir.empty()) {
                std::wstring log_dir = path_join(config_dir, L"bridge");
                ensure_directory(log_dir);
                std::string log_path = wide_to_utf8(path_join(log_dir, L"bridge-output.log"));
                reporter.set_log_file(log_path);
            }
        }

        reporter.log("info", "Loaded config for importer: " + config.importer);

        // 3. Execute pre-launch script if configured
        if (config.pre_launch.enabled && !config.pre_launch.cmd.empty()) {
            reporter.status("Running pre-launch command...");
            run_shell_command(config.pre_launch.cmd, config.pre_launch.wait);
        }

        // 4. Validate package files (d3dx.ini must exist in the per-game importer folder)
        std::wstring ini_path = path_join(config.paths.importer_folder, L"d3dx.ini");
        if (!file_exists(ini_path)) {
            reporter.error("MISSING_D3DX_INI",
                "d3dx.ini not found in importer folder: " +
                wide_to_utf8(config.paths.importer_folder));
            return 1;
        }
        ensure_directory(path_join(config.paths.importer_folder, L"Mods"));

        // 4b. Remove conflicting d3dx.ini from game directory if it exists
        // 3DMigoto warns about this and it can cause config confusion
        {
            std::wstring game_d3dx = path_join(config.game.work_dir, L"d3dx.ini");
            if (file_exists(game_d3dx) && config.game.work_dir != config.paths.importer_folder) {
                std::wstring backup = path_join(config.game.work_dir, L"d3dx.ini.bak");
                if (file_exists(backup)) delete_file(backup);
                rename_path(game_d3dx, backup);
                reporter.log("info", "Renamed conflicting d3dx.ini in game directory to d3dx.ini.bak");
            }
        }

        // 5. Execute auto_update.xcmd PreLaunch commands
        std::wstring xcmd_path = path_join(
            path_join(config.paths.importer_folder, L"Core"), L"auto_update.xcmd");
        if (file_exists(xcmd_path)) {
            reporter.status("Executing pre-launch commands...");
            XcmdHandler xcmd(xcmd_path);
            xcmd.execute_section("PreLaunch", config.paths.importer_folder);
        }

        // 6. Update d3dx.ini with configured settings
        reporter.status("Updating d3dx.ini...");
        D3dxIniConfigurator d3dx_conf(config);
        d3dx_conf.update();

        // 7. Optimize Mods and ShaderFixes folders
        {
            reporter.status("Optimizing INI files in Mods folder...");
            ModManager mod_mgr;

            // Get exclude patterns from d3dx.ini [Include] section
            std::vector<std::string> exclude_patterns;
            IniHandler* d3dx_ini = d3dx_conf.get_ini();
            if (d3dx_ini) {
                auto values = d3dx_ini->get_option_values("exclude_recursive", "Include");
                for (auto& kv : values) {
                    exclude_patterns.push_back(kv.second);
                }
            }
            if (exclude_patterns.empty()) {
                exclude_patterns.push_back("DISABLED*");
            }

            std::wstring mods_path = path_join(config.paths.importer_folder, L"Mods");
            std::wstring cache_path = path_join(
                path_join(config.paths.cache_folder, L"Ini Optimizer"),
                utf8_to_wide(config.importer + ".json"));
            mod_mgr.optimize_mods_folder(mods_path, cache_path, exclude_patterns);

            reporter.status("Optimizing INI files in ShaderFixes folder...");
            std::wstring sf_path = path_join(config.paths.importer_folder, L"ShaderFixes");
            mod_mgr.optimize_shaderfixes_folder(sf_path, exclude_patterns);
        }

        // 8. Game-specific initialization (driven by config.importer name)
        auto initializer = GameInitializer::create(config.importer);
        if (initializer) {
            reporter.status("Initializing game settings...");
            initializer->initialize(config, reporter);
        }

        // 9. Deploy XXMI DLL files (per-game importer folder, isolated)
        reporter.status("Deploying XXMI libraries...");
        PackageDeployer deployer(config, reporter);
        deployer.deploy();

        // 10. Inject 3DMigoto into game process
        // Same flow as original XXMI-Launcher: hook injection with direct
        // injection fallback. Jadeite wrapping (for miHoYo games) is handled
        // inside open_process() — it's just how the game exe is launched.
        reporter.status("Loading injector...");
        DllInjector injector(config);

        if (config.migoto.use_hook) {
            try {
                injector.run_hook_injection(reporter);
            } catch (const std::exception& hook_err) {
                reporter.warning(std::string("Hook injection failed: ") +
                                 hook_err.what() + " — falling back to direct injection");
                reporter.log("info", "Retrying with direct injection (CreateRemoteThread)...");
                DllInjector injector2(config);
                injector2.run_direct_injection(reporter);
            }
        } else {
            injector.run_direct_injection(reporter);
        }

        // 11. Execute post-load script if configured
        if (config.post_load.enabled && !config.post_load.cmd.empty()) {
            reporter.status("Running post-load command...");
            run_shell_command(config.post_load.cmd, config.post_load.wait);
        }

        reporter.done(true);
        return 0;

    } catch (const std::exception& e) {
        reporter.error("BRIDGE_ERROR", e.what());
        reporter.done(false);
        return 1;
    }
}
