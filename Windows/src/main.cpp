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

// Files dropped into the game directory by DLL Proxy Chain mode.
// Cleaned up at the start of every run and after game launch.
static const wchar_t* DLL_DROP_FILES[] = {
    L"d3d11.dll", L"d3dcompiler_47.dll", L"d3dx.ini", L"dxvk_d3d11.dll",
    nullptr
};
static const wchar_t* DLL_DROP_DIRS[] = {
    L"Mods", L"ShaderFixes", L"Core",
    nullptr
};

static void cleanup_dll_drop(const BridgeConfig& config, StatusReporter& reporter) {
    std::wstring game_dir = config.game.work_dir;
    if (game_dir.empty()) return;

    for (int i = 0; DLL_DROP_FILES[i]; ++i) {
        std::wstring p = path_join(game_dir, DLL_DROP_FILES[i]);
        if (file_exists(p)) {
            delete_file(p);
            reporter.log("debug", "DLL Drop cleanup: removed " +
                         wide_to_utf8(DLL_DROP_FILES[i]));
        }
    }
    // Remove directory symlinks (only if they are reparse points / symlinks)
    for (int i = 0; DLL_DROP_DIRS[i]; ++i) {
        std::wstring p = path_join(game_dir, DLL_DROP_DIRS[i]);
#ifdef _WIN32
        DWORD attr = GetFileAttributesW(p.c_str());
        if (attr != INVALID_FILE_ATTRIBUTES &&
            (attr & FILE_ATTRIBUTE_REPARSE_POINT)) {
            // It's a symlink we created — remove it (RemoveDirectoryW removes symlink, not target)
            RemoveDirectoryW(p.c_str());
            reporter.log("debug", "DLL Drop cleanup: removed symlink " +
                         wide_to_utf8(DLL_DROP_DIRS[i]));
        }
#endif
    }
}

static bool create_dir_symlink(const std::wstring& target, const std::wstring& link,
                                StatusReporter& reporter) {
#ifdef _WIN32
    // CreateSymbolicLinkW: SYMBOLIC_LINK_FLAG_DIRECTORY = 0x1
    // Available since Vista / Wine 1.5+
    typedef BOOLEAN (WINAPI *Fn)(LPCWSTR, LPCWSTR, DWORD);
    static Fn fn = (Fn)GetProcAddress(GetModuleHandleW(L"kernel32.dll"),
                                       "CreateSymbolicLinkW");
    if (!fn) {
        reporter.log("warn", "CreateSymbolicLinkW not available");
        return false;
    }
    BOOLEAN ok = fn(link.c_str(), target.c_str(), 0x1);
    if (!ok) {
        reporter.log("warn", "CreateSymbolicLinkW failed for " +
                     wide_to_utf8(link) + " -> " + wide_to_utf8(target) +
                     ", err=" + std::to_string(GetLastError()));
        return false;
    }
    return true;
#else
    return false;
#endif
}

static void run_dll_drop(const BridgeConfig& config, StatusReporter& reporter) {
    std::wstring game_dir = config.game.work_dir;
    std::wstring imp_dir  = config.paths.importer_folder;

    if (game_dir.empty()) {
        throw std::runtime_error("DLL Drop: game work_dir is empty");
    }

    // 0. Cleanup any leftover files from previous runs
    cleanup_dll_drop(config, reporter);

    // 1. Copy 3DMigoto DLLs from importer folder to game directory
    std::wstring src_d3d11 = path_join(imp_dir, L"d3d11.dll");
    std::wstring src_d3dc  = path_join(imp_dir, L"d3dcompiler_47.dll");
    std::wstring src_ini   = path_join(imp_dir, L"d3dx.ini");

    if (!file_exists(src_d3d11)) {
        throw std::runtime_error("DLL Drop: d3d11.dll not found in importer folder");
    }

    reporter.log("info", "DLL Drop: copying 3DMigoto files to game directory");
    copy_file_overwrite(src_d3d11, path_join(game_dir, L"d3d11.dll"));
    if (file_exists(src_d3dc)) {
        copy_file_overwrite(src_d3dc, path_join(game_dir, L"d3dcompiler_47.dll"));
    }
    copy_file_overwrite(src_ini, path_join(game_dir, L"d3dx.ini"));

    // 2. Copy DXVK's d3d11.dll as dxvk_d3d11.dll (the proxy target)
    //    DXVK's DLL lives in system32 inside the Wine prefix.
#ifdef _WIN32
    {
        wchar_t sys_dir[MAX_PATH] = {};
        GetSystemDirectoryW(sys_dir, MAX_PATH);
        std::wstring dxvk_src = std::wstring(sys_dir) + L"\\d3d11.dll";

        if (!file_exists(dxvk_src)) {
            throw std::runtime_error("DLL Drop: DXVK d3d11.dll not found at " +
                                     wide_to_utf8(dxvk_src));
        }

        reporter.log("info", "DLL Drop: copying DXVK d3d11.dll as dxvk_d3d11.dll");
        copy_file_overwrite(dxvk_src, path_join(game_dir, L"dxvk_d3d11.dll"));
    }
#endif

    // 3. Create directory symlinks so d3dx.ini relative paths resolve:
    //    GameDir/Mods -> ImporterDir/Mods
    //    GameDir/ShaderFixes -> ImporterDir/ShaderFixes
    //    GameDir/Core -> ImporterDir/Core
    for (int i = 0; DLL_DROP_DIRS[i]; ++i) {
        std::wstring target = path_join(imp_dir, DLL_DROP_DIRS[i]);
        std::wstring link   = path_join(game_dir, DLL_DROP_DIRS[i]);

        if (!directory_exists(target)) {
            reporter.log("debug", "DLL Drop: skipping symlink for " +
                         wide_to_utf8(DLL_DROP_DIRS[i]) + " (not found in importer)");
            continue;
        }

        // Skip if already a real directory in the game folder
#ifdef _WIN32
        DWORD attr = GetFileAttributesW(link.c_str());
        if (attr != INVALID_FILE_ATTRIBUTES &&
            !(attr & FILE_ATTRIBUTE_REPARSE_POINT)) {
            reporter.log("debug", "DLL Drop: " + wide_to_utf8(DLL_DROP_DIRS[i]) +
                         " already exists as real dir in game folder, skipping symlink");
            continue;
        }
#endif

        if (!create_dir_symlink(target, link, reporter)) {
            reporter.warning("DLL Drop: failed to create symlink for " +
                             wide_to_utf8(DLL_DROP_DIRS[i]) +
                             " — mods may not load correctly");
        } else {
            reporter.log("info", "DLL Drop: symlink " + wide_to_utf8(DLL_DROP_DIRS[i]) +
                         " -> " + wide_to_utf8(target));
        }
    }

    // 4. Launch game (through jadeite if configured) — NO injection
    reporter.status("Starting game process (DLL proxy chain)...");
    DllInjector launcher(config);
    launcher.launch_process_only();

    // 5. Wait for game window
    reporter.status("Waiting for game window...");
    unsigned long pid = ProcessTracker::find_process(config.game.process_name);
    auto window_result = ProcessTracker::wait_for_window(
        config.game.process_name, pid, config.game.process_timeout, true);

    if (window_result == WaitResult::Timeout) {
        reporter.warning("Timed out waiting for game window");
    }

    pid = ProcessTracker::find_process(config.game.process_name);
    reporter.inject_result("dll_drop", true, pid);

    // 6. Cleanup dropped files (game has already loaded them)
    reporter.status("Cleaning up dropped files...");
    // Only remove DLL files — keep symlinks alive while game is running
    // (3DMigoto may lazily load mods/shaders)
    for (int i = 0; DLL_DROP_FILES[i]; ++i) {
        std::wstring name(DLL_DROP_FILES[i]);
        // Keep d3d11.dll and dxvk_d3d11.dll — they are in use by the game process.
        // Keep d3dx.ini — 3DMigoto may re-read it.
        // Only clean up d3dcompiler_47 if it was copied.
        // Actually, all files are mapped into the game process — we CANNOT delete them
        // while the game is running. Cleanup will happen on next bridge start.
    }
    reporter.log("info", "DLL Drop: files will be cleaned up on next launch");
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

static BridgeConfig load_bridge_config(const std::wstring& config_path, StatusReporter& reporter) {
    reporter.status("Loading configuration...");
    BridgeConfig config = BridgeConfig::load(config_path);
    reporter.log("info", "Loaded config for importer: " + config.importer);
    return config;
}

static void configure_reporter_log_file(const BridgeConfig& config, StatusReporter& reporter) {
    std::wstring config_dir = config.paths.cache_folder;
    if (config_dir.empty()) {
        return;
    }

    std::wstring log_dir = path_join(config_dir, L"bridge");
    ensure_directory(log_dir);
    std::string log_path = wide_to_utf8(path_join(log_dir, L"bridge-output.log"));
    reporter.set_log_file(log_path);
}

static void run_configured_shell_command(
    const ShellCommandConfig& command,
    const char* status_message,
    StatusReporter& reporter
) {
    if (!command.enabled || command.cmd.empty()) {
        return;
    }

    reporter.status(status_message);
    run_shell_command(command.cmd, command.wait);
}

static void validate_bridge_runtime_files(const BridgeConfig& config, StatusReporter& reporter) {
    std::wstring ini_path = path_join(config.paths.importer_folder, L"d3dx.ini");
    if (!file_exists(ini_path)) {
        reporter.error(
            "MISSING_D3DX_INI",
            "d3dx.ini not found in importer folder: " +
                wide_to_utf8(config.paths.importer_folder)
        );
        throw std::runtime_error("Required importer d3dx.ini is missing");
    }

    ensure_directory(path_join(config.paths.importer_folder, L"Mods"));

    std::wstring game_d3dx = path_join(config.game.work_dir, L"d3dx.ini");
    if (file_exists(game_d3dx) && config.game.work_dir != config.paths.importer_folder) {
        std::wstring backup = path_join(config.game.work_dir, L"d3dx.ini.bak");
        if (file_exists(backup)) {
            delete_file(backup);
        }
        rename_path(game_d3dx, backup);
        reporter.log("info", "Renamed conflicting d3dx.ini in game directory to d3dx.ini.bak");
    }
}

static void optimize_importer_folders(
    const BridgeConfig& config,
    D3dxIniConfigurator& d3dx_conf,
    StatusReporter& reporter
) {
    if (iequals(config.importer, "EFMI")) {
        reporter.status("Skipping INI optimizer for EFMI...");
        reporter.log(
            "info",
            "EFMI optimization bypassed to preserve ALPHA-4 mod semantics"
        );
        return;
    }

    reporter.status("Optimizing INI files in Mods folder...");
    ModManager mod_mgr;

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
        utf8_to_wide(config.importer + ".json")
    );
    mod_mgr.optimize_mods_folder(mods_path, cache_path, exclude_patterns);

    reporter.status("Optimizing INI files in ShaderFixes folder...");
    std::wstring sf_path = path_join(config.paths.importer_folder, L"ShaderFixes");
    mod_mgr.optimize_shaderfixes_folder(sf_path, exclude_patterns);
}

static void initialize_game_runtime(const BridgeConfig& config, StatusReporter& reporter) {
    auto initializer = GameInitializer::create(config.importer);
    if (!initializer) {
        return;
    }

    reporter.status("Initializing game settings...");
    initializer->initialize(config, reporter);
}

static void launch_migoto_runtime(const BridgeConfig& config, StatusReporter& reporter) {
    reporter.status("Deploying XXMI libraries...");
    PackageDeployer deployer(config, reporter);
    deployer.deploy();

    if (config.migoto.use_dll_drop) {
        reporter.status("Setting up DLL proxy chain...");
        run_dll_drop(config, reporter);
        return;
    }

    reporter.status("Loading injector...");
    DllInjector injector(config);

    if (config.migoto.use_hook) {
        try {
            injector.run_hook_injection(reporter);
            return;
        } catch (const std::exception& hook_err) {
            reporter.warning(
                std::string("Hook injection failed: ") + hook_err.what() +
                " — falling back to direct injection"
            );
            reporter.log("info", "Retrying with direct injection (CreateRemoteThread)...");
        }
    }

    DllInjector injector2(config);
    injector2.run_direct_injection(reporter);
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
        BridgeConfig config = load_bridge_config(config_path, reporter);
        configure_reporter_log_file(config, reporter);

        // 3. Execute pre-launch script if configured
        run_configured_shell_command(config.pre_launch, "Running pre-launch command...", reporter);

        // 4. Validate package files (d3dx.ini must exist in the per-game importer folder)
        validate_bridge_runtime_files(config, reporter);

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
        optimize_importer_folders(config, d3dx_conf, reporter);

        // 8. Game-specific initialization (driven by config.importer name)
        initialize_game_runtime(config, reporter);

        // 9. Deploy XXMI DLL files (per-game importer folder, isolated)
        launch_migoto_runtime(config, reporter);

        // 11. Execute post-load script if configured
        run_configured_shell_command(config.post_load, "Running post-load command...", reporter);

        reporter.done(true);
        return 0;

    } catch (const std::exception& e) {
        reporter.error("BRIDGE_ERROR", e.what());
        reporter.done(false);
        return 1;
    }
}
