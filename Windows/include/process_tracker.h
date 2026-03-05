#pragma once

#include <string>
#include <vector>

#ifdef _WIN32
#include <windows.h>
#endif

namespace bridge {

enum class WaitResult {
    Found = 0,
    NotFound = -100,
    Timeout = -200,
};

class ProcessTracker {
public:
    static WaitResult wait_for_process(const std::wstring& process_name,
                                        unsigned long& out_pid,
                                        int timeout_sec);

    static WaitResult wait_for_process_exit(unsigned long pid, int timeout_sec);

    static WaitResult wait_for_window(const std::wstring& process_name,
                                       unsigned long pid,
                                       int timeout_sec,
                                       bool check_visibility = true);

    static unsigned long find_process(const std::wstring& process_name);

#ifdef _WIN32
    static std::vector<HWND> get_hwnds_for_pid(unsigned long pid,
                                                 bool check_visibility = false);
#endif
};

} // namespace bridge
