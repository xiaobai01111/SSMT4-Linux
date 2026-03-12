#include "dll_injector.h"
#include "process_tracker.h"
#include "path_utils.h"
#include "string_utils.h"
#include <stdexcept>
#include <thread>
#include <chrono>

#ifdef _WIN32
#include <tlhelp32.h>
#endif

namespace bridge {

DllInjector::DllInjector(const BridgeConfig& config)
    : config_(config) {
}

DllInjector::~DllInjector() {
    unload_injector_lib();
}

void DllInjector::load_injector_lib() {
#ifdef _WIN32
    // 3dmloader.dll is located in the per-game importer folder
    std::wstring loader_path = path_join(config_.paths.importer_folder, L"3dmloader.dll");
    if (!file_exists(loader_path)) {
        throw std::runtime_error("3dmloader.dll not found in: " +
                                 wide_to_utf8(config_.paths.importer_folder));
    }

    lib_ = LoadLibraryW(loader_path.c_str());
    if (!lib_) {
        DWORD err = GetLastError();
        throw std::runtime_error("Failed to load 3dmloader.dll, error code: " +
                                 std::to_string(err));
    }

    fn_hook_library_ = (HookLibraryFn)GetProcAddress(lib_, "HookLibrary");
    fn_wait_for_injection_ = (WaitForInjectionFn)GetProcAddress(lib_, "WaitForInjection");
    fn_unhook_library_ = (UnhookLibraryFn)GetProcAddress(lib_, "UnhookLibrary");
    fn_start_process_ = (StartProcessFn)GetProcAddress(lib_, "StartProcess");
    fn_inject_ = (InjectFn)GetProcAddress(lib_, "Inject");
#endif
}

void DllInjector::unload_injector_lib() {
#ifdef _WIN32
    if (lib_) {
        FreeLibrary(lib_);
        lib_ = nullptr;
    }
    fn_hook_library_ = nullptr;
    fn_wait_for_injection_ = nullptr;
    fn_unhook_library_ = nullptr;
    fn_start_process_ = nullptr;
    fn_inject_ = nullptr;
    hook_ = nullptr;
    mutex_ = nullptr;
#endif
}

unsigned long DllInjector::get_creation_flags() const {
#ifdef _WIN32
    DWORD flags = 0;
    const std::string& priority = config_.game.process_priority;
    if (iequals(priority, "Low")) flags = IDLE_PRIORITY_CLASS;
    else if (iequals(priority, "Below Normal")) flags = BELOW_NORMAL_PRIORITY_CLASS;
    else if (iequals(priority, "Normal")) flags = NORMAL_PRIORITY_CLASS;
    else if (iequals(priority, "Above Normal")) flags = ABOVE_NORMAL_PRIORITY_CLASS;
    else if (iequals(priority, "High")) flags = HIGH_PRIORITY_CLASS;
    else if (iequals(priority, "Realtime")) flags = REALTIME_PRIORITY_CLASS;
    else flags = NORMAL_PRIORITY_CLASS;
    return flags;
#else
    return 0;
#endif
}

void DllInjector::hook_library(const std::wstring& dll_path,
                                const std::wstring& process_name) {
#ifdef _WIN32
    if (!fn_hook_library_) {
        throw std::runtime_error("HookLibrary function not available in 3dmloader.dll");
    }

    int result = fn_hook_library_(dll_path.c_str(), &hook_, &mutex_);

    switch (result) {
        case 0: break; // Success
        case 100:
            throw std::runtime_error("Another instance is already running (hook mutex exists)");
        case 200:
            throw std::runtime_error("Failed to load target DLL for hooking: " +
                                     wide_to_utf8(dll_path));
        case 300:
            throw std::runtime_error("DLL entry point not found: " + wide_to_utf8(dll_path));
        case 400:
            throw std::runtime_error("SetWindowsHookEx failed");
        default:
            throw std::runtime_error("HookLibrary failed with code: " + std::to_string(result));
    }
#endif
}

bool DllInjector::wait_for_injection(int timeout) {
#ifdef _WIN32
    if (!fn_wait_for_injection_) return false;

    std::wstring dll_path = path_join(config_.paths.importer_folder, L"d3d11.dll");
    int result = fn_wait_for_injection_(dll_path.c_str(),
                                         config_.game.process_name.c_str(),
                                         timeout);
    return result == 0;
#else
    return false;
#endif
}

bool DllInjector::unhook_library() {
#ifdef _WIN32
    if (!fn_unhook_library_) return false;
    int result = fn_unhook_library_(&hook_, &mutex_);
    hook_ = nullptr;
    mutex_ = nullptr;
    return result == 0;
#else
    return false;
#endif
}

void DllInjector::start_process_native(const std::wstring& exe, const std::wstring& work_dir,
                                        const std::vector<std::wstring>& args,
                                        unsigned long flags) {
#ifdef _WIN32
    // Build command line: "exe" arg1 arg2 ...
    std::wstring cmd_line = L"\"" + exe + L"\"";
    for (auto& arg : args) {
        cmd_line += L" " + arg;
    }

    STARTUPINFOW si = {};
    si.cb = sizeof(si);
    PROCESS_INFORMATION pi = {};

    BOOL ok = CreateProcessW(
        nullptr,
        const_cast<LPWSTR>(cmd_line.c_str()),
        nullptr, nullptr, FALSE,
        flags,
        nullptr,
        work_dir.empty() ? nullptr : work_dir.c_str(),
        &si, &pi);

    if (!ok) {
        throw std::runtime_error("CreateProcessW failed, error: " +
                                 std::to_string(GetLastError()));
    }
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
#endif
}

void DllInjector::start_process_shell(const std::wstring& exe, const std::wstring& work_dir,
                                       const std::wstring& args) {
#ifdef _WIN32
    if (fn_start_process_) {
        fn_start_process_(exe.c_str(),
                          work_dir.empty() ? nullptr : work_dir.c_str(),
                          args.c_str());
    } else {
        // Fallback to ShellExecuteW
        ShellExecuteW(nullptr, L"open", exe.c_str(), args.c_str(),
                      work_dir.empty() ? nullptr : work_dir.c_str(), SW_SHOWNORMAL);
    }
#endif
}

unsigned long DllInjector::inject_libraries(const std::vector<std::wstring>& dll_paths,
                                             const std::wstring& process_name,
                                             int timeout) {
#ifdef _WIN32
    if (!fn_inject_) {
        throw std::runtime_error("Inject function not available in 3dmloader.dll");
    }

    // Wait for process to appear
    unsigned long pid = 0;
    auto wait_result = ProcessTracker::wait_for_process(process_name, pid, timeout);
    if (wait_result != WaitResult::Found || pid == 0) {
        return (unsigned long)-1;
    }

    // Inject each DLL
    for (auto& dll_path : dll_paths) {
        int result = fn_inject_(pid, dll_path.c_str(), timeout);
        switch (result) {
            case 0: break; // Success
            case 100:
                throw std::runtime_error("Target process not found for injection");
            case 200:
                throw std::runtime_error("VirtualAllocEx failed during injection");
            case 300:
                throw std::runtime_error("WriteProcessMemory failed during injection");
            case 400:
                throw std::runtime_error("CreateRemoteThread failed during injection");
            default:
                throw std::runtime_error("Injection failed with code: " +
                                         std::to_string(result));
        }
    }

    return pid;
#else
    return 0;
#endif
}

void DllInjector::open_process(const std::vector<std::wstring>* inject_dll_paths) {
    std::string method = config_.game.process_start_method;
    std::wstring exe;
    std::wstring work_dir = config_.game.work_dir;

    auto start_args = config_.game.start_args;

    // Use custom launch command if enabled
    if (config_.custom_launch.enabled && !config_.custom_launch.cmd.empty()) {
#ifdef _WIN32
        std::wstring cmd = utf8_to_wide(config_.custom_launch.cmd);
        start_process_shell(L"cmd.exe", L"", L"/C \"" + cmd + L"\"");
#endif
    } else if (iequals(method, "native")) {
        exe = path_join(config_.paths.game_folder, config_.game.start_exe);

        // Jadeite anti-cheat bypass: launch game through jadeite.exe
        // instead of directly, for miHoYo games (SRMI, ZZMI) on Linux/Proton
        // Format: jadeite.exe <game_exe_path> -- [game_args...]
        // The "--" separator is required by jadeite to delimit game path from args.
        if (config_.jadeite.enabled && !config_.jadeite.exe_path.empty()) {
            std::vector<std::wstring> jadeite_args;
            jadeite_args.push_back(exe);
            jadeite_args.push_back(L"--");
            for (auto& arg : start_args) {
                jadeite_args.push_back(arg);
            }
            start_process_native(config_.jadeite.exe_path, work_dir,
                                 jadeite_args, get_creation_flags());
        } else {
            start_process_native(exe, work_dir, start_args, get_creation_flags());
        }
    } else if (iequals(method, "shell")) {
        exe = path_join(config_.paths.game_folder, config_.game.start_exe);
        std::wstring args_str;
        for (auto& arg : start_args) {
            if (!args_str.empty()) args_str += L" ";
            args_str += arg;
        }
        start_process_shell(exe, work_dir, args_str);
    } else if (iequals(method, "manual")) {
        // Wait for user to start the game manually — do nothing
    } else {
        throw std::runtime_error("Unknown process start method: " + method);
    }

    // If direct injection DLLs are provided, inject them now
    if (inject_dll_paths && !inject_dll_paths->empty()) {
        inject_libraries(*inject_dll_paths, config_.game.process_name,
                         config_.game.process_timeout);
    }
}

void DllInjector::launch_process_only() {
    open_process(nullptr);
}

void DllInjector::run_hook_injection(StatusReporter& reporter) {
    load_injector_lib();

    try {
        // 1. Set up global Windows hook for 3DMigoto DLL
        std::wstring d3d11_path = path_join(config_.paths.importer_folder, L"d3d11.dll");
        reporter.status("Setting up injection hook...");
        hook_library(d3d11_path, config_.game.process_name);

        // 2. Start the game process (through jadeite if configured)
        reporter.status("Starting game process...");
        open_process();

        // 3. Verify injection succeeded
        reporter.status("Waiting for injection...");
        wait_for_injection(5);

        // 4. Unhook ASAP to minimize global hook footprint
        //    (anti-cheat may scan for active SetWindowsHookEx hooks)
        reporter.status("Cleaning up hook...");
        unhook_library();

        // 5. Wait for game window to become visible
        reporter.status("Waiting for game window...");
        unsigned long pid = ProcessTracker::find_process(config_.game.process_name);
        auto window_result = ProcessTracker::wait_for_window(
            config_.game.process_name, pid, config_.game.process_timeout, true);

        if (window_result == WaitResult::Timeout) {
            reporter.warning("Timed out waiting for game window");
        }

        // 6. Report success
        pid = ProcessTracker::find_process(config_.game.process_name);
        reporter.inject_result("hook", true, pid);

    } catch (...) {
        unhook_library();
        unload_injector_lib();
        throw;
    }

    unload_injector_lib();
}

void DllInjector::run_direct_injection(StatusReporter& reporter) {
    load_injector_lib();

    try {
        // Build list of DLLs to inject
        std::vector<std::wstring> dll_paths;
        dll_paths.push_back(path_join(config_.paths.importer_folder, L"d3d11.dll"));

        // Add extra libraries if enabled
        if (config_.extra_libraries.enabled) {
            for (auto& p : config_.extra_libraries.paths) {
                dll_paths.push_back(p);
            }
        }

        // 1. Start game + inject DLLs
        reporter.status("Starting game and injecting...");
        open_process(&dll_paths);

        // 2. Wait for game window
        reporter.status("Waiting for game window...");
        unsigned long pid = ProcessTracker::find_process(config_.game.process_name);
        auto window_result = ProcessTracker::wait_for_window(
            config_.game.process_name, pid, config_.game.process_timeout, true);

        if (window_result == WaitResult::Timeout) {
            reporter.warning("Timed out waiting for game window");
        }

        // 3. Report success
        pid = ProcessTracker::find_process(config_.game.process_name);
        reporter.inject_result("direct", true, pid);

    } catch (...) {
        unload_injector_lib();
        throw;
    }

    unload_injector_lib();
}

} // namespace bridge
