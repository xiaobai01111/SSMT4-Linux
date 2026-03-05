#include "process_tracker.h"
#include "string_utils.h"

#ifdef _WIN32
#include <tlhelp32.h>
#endif

namespace bridge {

unsigned long ProcessTracker::find_process(const std::wstring& process_name) {
#ifdef _WIN32
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return 0;

    PROCESSENTRY32W pe;
    pe.dwSize = sizeof(pe);

    if (Process32FirstW(snap, &pe)) {
        do {
            if (iequals(wide_to_utf8(pe.szExeFile), wide_to_utf8(process_name))) {
                DWORD pid = pe.th32ProcessID;
                CloseHandle(snap);
                return pid;
            }
        } while (Process32NextW(snap, &pe));
    }
    CloseHandle(snap);
#endif
    return 0;
}

WaitResult ProcessTracker::wait_for_process(const std::wstring& process_name,
                                             unsigned long& out_pid,
                                             int timeout_sec) {
    int elapsed_ms = 0;
    int interval_ms = 100;
    int timeout_ms = timeout_sec * 1000;

    while (elapsed_ms < timeout_ms) {
        unsigned long pid = find_process(process_name);
        if (pid != 0) {
            out_pid = pid;
            return WaitResult::Found;
        }
#ifdef _WIN32
        Sleep(interval_ms);
#endif
        elapsed_ms += interval_ms;
    }
    return WaitResult::Timeout;
}

WaitResult ProcessTracker::wait_for_process_exit(unsigned long pid, int timeout_sec) {
#ifdef _WIN32
    HANDLE hProcess = OpenProcess(SYNCHRONIZE, FALSE, pid);
    if (!hProcess) return WaitResult::NotFound;

    DWORD result = WaitForSingleObject(hProcess, timeout_sec * 1000);
    CloseHandle(hProcess);

    if (result == WAIT_OBJECT_0) return WaitResult::Found;
    if (result == WAIT_TIMEOUT) return WaitResult::Timeout;
#endif
    return WaitResult::NotFound;
}

#ifdef _WIN32
struct EnumWindowsData {
    unsigned long target_pid;
    bool check_visibility;
    std::vector<HWND> hwnds;
};

static BOOL CALLBACK enum_windows_proc(HWND hwnd, LPARAM lparam) {
    EnumWindowsData* data = reinterpret_cast<EnumWindowsData*>(lparam);
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    if (pid == data->target_pid) {
        if (data->check_visibility) {
            if (!IsWindowVisible(hwnd) || IsIconic(hwnd)) {
                return TRUE; // skip non-visible/minimized
            }
        }
        data->hwnds.push_back(hwnd);
    }
    return TRUE;
}

std::vector<HWND> ProcessTracker::get_hwnds_for_pid(unsigned long pid,
                                                      bool check_visibility) {
    EnumWindowsData data;
    data.target_pid = pid;
    data.check_visibility = check_visibility;
    EnumWindows(enum_windows_proc, reinterpret_cast<LPARAM>(&data));
    return data.hwnds;
}
#endif

WaitResult ProcessTracker::wait_for_window(const std::wstring& process_name,
                                            unsigned long pid,
                                            int timeout_sec,
                                            bool check_visibility) {
#ifdef _WIN32
    int elapsed_ms = 0;
    int interval_ms = 100;
    int timeout_ms = timeout_sec * 1000;

    while (elapsed_ms < timeout_ms) {
        // If pid is 0, try to find the process first
        unsigned long target_pid = pid;
        if (target_pid == 0) {
            target_pid = find_process(process_name);
        }

        if (target_pid != 0) {
            auto hwnds = get_hwnds_for_pid(target_pid, check_visibility);
            if (!hwnds.empty()) {
                return WaitResult::Found;
            }
        }

        Sleep(interval_ms);
        elapsed_ms += interval_ms;
    }
#endif
    return WaitResult::Timeout;
}

} // namespace bridge
