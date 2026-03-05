#include "path_utils.h"
#include "string_utils.h"
#include <fstream>
#include <sstream>

#ifdef _WIN32
#include <shlwapi.h>
#endif

namespace bridge {

bool file_exists(const std::wstring& path) {
#ifdef _WIN32
    DWORD attrs = GetFileAttributesW(path.c_str());
    return (attrs != INVALID_FILE_ATTRIBUTES) && !(attrs & FILE_ATTRIBUTE_DIRECTORY);
#else
    return false;
#endif
}

bool directory_exists(const std::wstring& path) {
#ifdef _WIN32
    DWORD attrs = GetFileAttributesW(path.c_str());
    return (attrs != INVALID_FILE_ATTRIBUTES) && (attrs & FILE_ATTRIBUTE_DIRECTORY);
#else
    return false;
#endif
}

bool ensure_directory(const std::wstring& path) {
#ifdef _WIN32
    if (directory_exists(path)) return true;
    // Recursively create parent directories
    std::wstring parent = get_parent_path(path);
    if (!parent.empty() && parent != path) {
        ensure_directory(parent);
    }
    return CreateDirectoryW(path.c_str(), nullptr) != 0 ||
           GetLastError() == ERROR_ALREADY_EXISTS;
#else
    return false;
#endif
}

bool copy_file_overwrite(const std::wstring& src, const std::wstring& dst) {
#ifdef _WIN32
    return CopyFileW(src.c_str(), dst.c_str(), FALSE) != 0;
#else
    return false;
#endif
}

bool delete_file(const std::wstring& path) {
#ifdef _WIN32
    // Remove read-only attribute if present
    DWORD attrs = GetFileAttributesW(path.c_str());
    if (attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_READONLY)) {
        SetFileAttributesW(path.c_str(), attrs & ~FILE_ATTRIBUTE_READONLY);
    }
    return DeleteFileW(path.c_str()) != 0;
#else
    return false;
#endif
}

bool delete_directory_recursive(const std::wstring& path) {
#ifdef _WIN32
    std::wstring search = path + L"\\*";
    WIN32_FIND_DATAW fd;
    HANDLE hFind = FindFirstFileW(search.c_str(), &fd);
    if (hFind == INVALID_HANDLE_VALUE) return false;

    do {
        std::wstring name(fd.cFileName);
        if (name == L"." || name == L"..") continue;
        std::wstring full = path + L"\\" + name;
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            delete_directory_recursive(full);
        } else {
            delete_file(full);
        }
    } while (FindNextFileW(hFind, &fd));
    FindClose(hFind);
    return RemoveDirectoryW(path.c_str()) != 0;
#else
    return false;
#endif
}

bool rename_path(const std::wstring& old_path, const std::wstring& new_path) {
#ifdef _WIN32
    return MoveFileW(old_path.c_str(), new_path.c_str()) != 0;
#else
    return false;
#endif
}

std::wstring get_parent_path(const std::wstring& path) {
    size_t pos = path.find_last_of(L"\\/");
    if (pos == std::wstring::npos) return L"";
    return path.substr(0, pos);
}

std::wstring get_filename(const std::wstring& path) {
    size_t pos = path.find_last_of(L"\\/");
    if (pos == std::wstring::npos) return path;
    return path.substr(pos + 1);
}

std::wstring get_stem(const std::wstring& path) {
    std::wstring name = get_filename(path);
    size_t pos = name.find_last_of(L'.');
    if (pos == std::wstring::npos) return name;
    return name.substr(0, pos);
}

std::wstring get_extension(const std::wstring& path) {
    std::wstring name = get_filename(path);
    size_t pos = name.find_last_of(L'.');
    if (pos == std::wstring::npos) return L"";
    return name.substr(pos);
}

std::wstring path_join(const std::wstring& base, const std::wstring& child) {
    if (base.empty()) return child;
    if (child.empty()) return base;
    wchar_t last = base.back();
    if (last == L'\\' || last == L'/') {
        return base + child;
    }
    return base + L"\\" + child;
}

std::wstring normalize_path(const std::wstring& path) {
    std::wstring result = path;
    for (auto& c : result) {
        if (c == L'/') c = L'\\';
    }
    return result;
}

uint64_t get_file_size(const std::wstring& path) {
#ifdef _WIN32
    WIN32_FILE_ATTRIBUTE_DATA fad;
    if (!GetFileAttributesExW(path.c_str(), GetFileExInfoStandard, &fad))
        return 0;
    LARGE_INTEGER size;
    size.HighPart = fad.nFileSizeHigh;
    size.LowPart = fad.nFileSizeLow;
    return static_cast<uint64_t>(size.QuadPart);
#else
    return 0;
#endif
}

uint64_t get_file_mtime(const std::wstring& path) {
#ifdef _WIN32
    WIN32_FILE_ATTRIBUTE_DATA fad;
    if (!GetFileAttributesExW(path.c_str(), GetFileExInfoStandard, &fad))
        return 0;
    LARGE_INTEGER li;
    li.HighPart = fad.ftLastWriteTime.dwHighDateTime;
    li.LowPart = fad.ftLastWriteTime.dwLowDateTime;
    return static_cast<uint64_t>(li.QuadPart);
#else
    return 0;
#endif
}

std::string read_file_text(const std::wstring& path) {
#ifdef _WIN32
    HANDLE hFile = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ,
                               nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE) return "";
    DWORD size = GetFileSize(hFile, nullptr);
    if (size == INVALID_FILE_SIZE || size == 0) {
        CloseHandle(hFile);
        return "";
    }
    std::string content(size, '\0');
    DWORD bytesRead = 0;
    ReadFile(hFile, &content[0], size, &bytesRead, nullptr);
    CloseHandle(hFile);
    content.resize(bytesRead);
    return content;
#else
    return "";
#endif
}

std::vector<uint8_t> read_file_binary(const std::wstring& path) {
#ifdef _WIN32
    HANDLE hFile = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ,
                               nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE) return {};
    DWORD size = GetFileSize(hFile, nullptr);
    if (size == INVALID_FILE_SIZE || size == 0) {
        CloseHandle(hFile);
        return {};
    }
    std::vector<uint8_t> data(size);
    DWORD bytesRead = 0;
    ReadFile(hFile, data.data(), size, &bytesRead, nullptr);
    CloseHandle(hFile);
    data.resize(bytesRead);
    return data;
#else
    return {};
#endif
}

bool write_file_text(const std::wstring& path, const std::string& content) {
#ifdef _WIN32
    // Ensure parent directory exists
    std::wstring parent = get_parent_path(path);
    if (!parent.empty()) ensure_directory(parent);

    HANDLE hFile = CreateFileW(path.c_str(), GENERIC_WRITE, 0,
                               nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE) return false;
    DWORD written = 0;
    BOOL ok = WriteFile(hFile, content.c_str(), (DWORD)content.size(), &written, nullptr);
    CloseHandle(hFile);
    return ok && written == content.size();
#else
    return false;
#endif
}

std::vector<std::wstring> list_directory(const std::wstring& dir_path) {
    std::vector<std::wstring> entries;
#ifdef _WIN32
    std::wstring search = dir_path + L"\\*";
    WIN32_FIND_DATAW fd;
    HANDLE hFind = FindFirstFileW(search.c_str(), &fd);
    if (hFind == INVALID_HANDLE_VALUE) return entries;
    do {
        std::wstring name(fd.cFileName);
        if (name == L"." || name == L"..") continue;
        entries.push_back(dir_path + L"\\" + name);
    } while (FindNextFileW(hFind, &fd));
    FindClose(hFind);
#endif
    return entries;
}

std::vector<std::wstring> find_files_recursive(const std::wstring& dir_path,
                                                const std::wstring& extension) {
    std::vector<std::wstring> results;
#ifdef _WIN32
    std::wstring search = dir_path + L"\\*";
    WIN32_FIND_DATAW fd;
    HANDLE hFind = FindFirstFileW(search.c_str(), &fd);
    if (hFind == INVALID_HANDLE_VALUE) return results;
    do {
        std::wstring name(fd.cFileName);
        if (name == L"." || name == L"..") continue;
        std::wstring full = dir_path + L"\\" + name;
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            auto sub = find_files_recursive(full, extension);
            results.insert(results.end(), sub.begin(), sub.end());
        } else {
            if (extension.empty() || get_extension(full) == extension) {
                results.push_back(full);
            }
        }
    } while (FindNextFileW(hFind, &fd));
    FindClose(hFind);
#endif
    return results;
}

bool matches_glob(const std::wstring& name, const std::wstring& pattern) {
#ifdef _WIN32
    return PathMatchSpecW(name.c_str(), pattern.c_str()) != 0;
#else
    return false;
#endif
}

} // namespace bridge
