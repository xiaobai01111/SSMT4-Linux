#pragma once

#include <string>
#include <vector>
#include <cstdint>

#ifdef _WIN32
#include <windows.h>
#endif

namespace bridge {

bool file_exists(const std::wstring& path);
bool directory_exists(const std::wstring& path);
bool ensure_directory(const std::wstring& path);
bool copy_file_overwrite(const std::wstring& src, const std::wstring& dst);
bool delete_file(const std::wstring& path);
bool delete_directory_recursive(const std::wstring& path);
bool rename_path(const std::wstring& old_path, const std::wstring& new_path);
std::wstring get_parent_path(const std::wstring& path);
std::wstring get_filename(const std::wstring& path);
std::wstring get_stem(const std::wstring& path);
std::wstring get_extension(const std::wstring& path);
std::wstring path_join(const std::wstring& base, const std::wstring& child);
std::wstring normalize_path(const std::wstring& path);
uint64_t get_file_size(const std::wstring& path);
uint64_t get_file_mtime(const std::wstring& path);
std::string read_file_text(const std::wstring& path);
std::vector<uint8_t> read_file_binary(const std::wstring& path);
bool write_file_text(const std::wstring& path, const std::string& content);
std::vector<std::wstring> list_directory(const std::wstring& dir_path);
std::vector<std::wstring> find_files_recursive(const std::wstring& dir_path,
                                                const std::wstring& extension);
bool matches_glob(const std::wstring& name, const std::wstring& pattern);

} // namespace bridge
