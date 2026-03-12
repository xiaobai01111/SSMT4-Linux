#pragma once

#include "bridge_config.h"
#include "status_reporter.h"
#include <string>
#include <vector>

#ifdef _WIN32
#include <windows.h>
#endif

namespace bridge {

class DllInjector {
public:
    explicit DllInjector(const BridgeConfig& config);
    ~DllInjector();

    DllInjector(const DllInjector&) = delete;
    DllInjector& operator=(const DllInjector&) = delete;

    void run_hook_injection(StatusReporter& reporter);
    void run_direct_injection(StatusReporter& reporter);
    void launch_process_only();

private:
    const BridgeConfig& config_;

#ifdef _WIN32
    HMODULE lib_ = nullptr;
    HHOOK hook_ = nullptr;
    HANDLE mutex_ = nullptr;

    // Function pointer types from 3dmloader.dll
    typedef int (__cdecl *HookLibraryFn)(LPCWSTR, HHOOK*, HANDLE*);
    typedef int (__cdecl *WaitForInjectionFn)(LPCWSTR, LPCWSTR, int);
    typedef int (__cdecl *UnhookLibraryFn)(HHOOK*, HANDLE*);
    typedef int (__cdecl *StartProcessFn)(LPCWSTR, LPCWSTR, LPCWSTR);
    typedef int (__cdecl *InjectFn)(DWORD, LPCWSTR, int);

    HookLibraryFn fn_hook_library_ = nullptr;
    WaitForInjectionFn fn_wait_for_injection_ = nullptr;
    UnhookLibraryFn fn_unhook_library_ = nullptr;
    StartProcessFn fn_start_process_ = nullptr;
    InjectFn fn_inject_ = nullptr;
#endif

    void load_injector_lib();
    void unload_injector_lib();
    void hook_library(const std::wstring& dll_path, const std::wstring& process_name);
    bool wait_for_injection(int timeout);
    bool unhook_library();
    void open_process(const std::vector<std::wstring>* inject_dll_paths = nullptr);
    void start_process_native(const std::wstring& exe, const std::wstring& work_dir,
                              const std::vector<std::wstring>& args, unsigned long flags);
    void start_process_shell(const std::wstring& exe, const std::wstring& work_dir,
                             const std::wstring& args);
    unsigned long inject_libraries(const std::vector<std::wstring>& dll_paths,
                                   const std::wstring& process_name, int timeout);
    unsigned long get_creation_flags() const;
};

} // namespace bridge
