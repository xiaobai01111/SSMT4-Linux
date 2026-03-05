#pragma once

#include <string>
#include <vector>
#include <map>
#include <memory>

namespace bridge {

struct IniHandlerSettings {
    bool ignore_comments = true;
    bool option_value_spacing = true;   // "key = value" vs "key=value"
    bool inline_comments = false;       // allow "key = value ; comment"
    bool add_section_spacing = false;   // blank line between sections
    bool right_split = false;           // split key=value from right side
};

struct IniOption {
    std::string name;           // option name (lowercase for matching)
    std::string original_name;  // original case
    std::string value;          // current value
    bool modified = false;
    std::vector<std::string> comments;  // leading comment lines
    std::string inline_comment;         // inline comment
    std::string raw_line;               // original line text (for unmodified output)
};

struct IniSection {
    std::string name;                   // section name (e.g. "Loader")
    std::string original_name;          // original case
    std::vector<IniOption> options;
    std::vector<std::string> leading_comments;
    std::string raw_header;             // original "[SectionName]" line

    IniOption* find_option(const std::string& name);
    const IniOption* find_option(const std::string& name) const;
    void set_option(const std::string& name, const std::string& value,
                    bool spacing);
    bool remove_option(const std::string& name,
                       const std::string* match_value = nullptr,
                       bool not_equal = false);
    std::vector<std::pair<std::string, std::string>> get_option_values(
        const std::string& name) const;
};

class IniHandler {
public:
    IniHandler(IniHandlerSettings settings, const std::string& content);

    void set_option(const std::string& section, const std::string& option,
                    const std::string& value);
    void set_option(const std::string& section, const std::string& option,
                    int value);
    void set_option(const std::string& section, const std::string& option,
                    double value);
    std::string get_option(const std::string& section,
                           const std::string& option) const;
    void remove_option(const std::string& option,
                       const std::string& section_name = "",
                       const std::string& option_value = "",
                       bool not_equal = false);
    void remove_section(const std::string& section);
    IniSection* get_section(const std::string& name);
    const IniSection* get_section(const std::string& name) const;
    std::map<std::string, std::string> get_option_values(
        const std::string& option, const std::string& section_name = "") const;

    bool is_modified() const;
    std::string to_string() const;

private:
    IniHandlerSettings settings_;
    std::vector<IniSection> sections_;
    std::vector<std::string> header_lines_;  // lines before first section
    bool modified_ = false;

    void parse(const std::string& content);
};

} // namespace bridge
