#pragma once

#include <string>
#include <vector>
#include <algorithm>
#include <cctype>

#ifdef _WIN32
#include <windows.h>
#endif

namespace bridge {

std::string trim(const std::string& s);
std::string to_lower(const std::string& s);
bool iequals(const std::string& a, const std::string& b);
std::vector<std::string> split(const std::string& s, char delimiter);
std::vector<std::string> split_lines(const std::string& s);
bool starts_with(const std::string& s, const std::string& prefix);
bool ends_with(const std::string& s, const std::string& suffix);
bool starts_with_icase(const std::string& s, const std::string& prefix);
std::string replace_all(const std::string& s, const std::string& from, const std::string& to);
std::string join(const std::vector<std::string>& parts, const std::string& delimiter);

#ifdef _WIN32
std::wstring utf8_to_wide(const std::string& utf8);
std::string wide_to_utf8(const std::wstring& wide);
#endif

} // namespace bridge
