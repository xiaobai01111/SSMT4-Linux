#include "ini_handler.h"
#include "string_utils.h"
#include <sstream>
#include <algorithm>

namespace bridge {

// --- IniSection ---

IniOption* IniSection::find_option(const std::string& name) {
    std::string lower = to_lower(name);
    for (auto& opt : options) {
        if (opt.name == lower) return &opt;
    }
    return nullptr;
}

const IniOption* IniSection::find_option(const std::string& name) const {
    std::string lower = to_lower(name);
    for (auto& opt : options) {
        if (opt.name == lower) return &opt;
    }
    return nullptr;
}

void IniSection::set_option(const std::string& name, const std::string& value,
                            bool spacing) {
    IniOption* existing = find_option(name);
    if (existing) {
        if (existing->value != value) {
            existing->value = value;
            existing->modified = true;
        }
    } else {
        IniOption opt;
        opt.name = to_lower(name);
        opt.original_name = name;
        opt.value = value;
        opt.modified = true;
        if (spacing) {
            opt.raw_line = name + " = " + value;
        } else {
            opt.raw_line = name + "=" + value;
        }
        options.push_back(opt);
    }
}

bool IniSection::remove_option(const std::string& name,
                                const std::string* match_value,
                                bool not_equal) {
    std::string lower = to_lower(name);
    bool removed = false;
    auto it = options.begin();
    while (it != options.end()) {
        if (it->name == lower) {
            bool should_remove = true;
            if (match_value) {
                if (not_equal) {
                    should_remove = (it->value != *match_value);
                } else {
                    should_remove = (it->value == *match_value);
                }
            }
            if (should_remove) {
                it = options.erase(it);
                removed = true;
            } else {
                ++it;
            }
        } else {
            ++it;
        }
    }
    return removed;
}

std::vector<std::pair<std::string, std::string>> IniSection::get_option_values(
    const std::string& name) const {
    std::string lower = to_lower(name);
    std::vector<std::pair<std::string, std::string>> results;
    for (auto& opt : options) {
        if (opt.name == lower) {
            results.push_back({opt.original_name, opt.value});
        }
    }
    return results;
}

// --- IniHandler ---

IniHandler::IniHandler(IniHandlerSettings settings, const std::string& content)
    : settings_(settings) {
    parse(content);
}

void IniHandler::parse(const std::string& content) {
    auto lines = split_lines(content);
    IniSection* current_section = nullptr;
    std::vector<std::string> pending_comments;

    for (auto& line : lines) {
        std::string trimmed = trim(line);

        // Empty line
        if (trimmed.empty()) {
            if (current_section) {
                // Store as a comment/blank line in current section
                IniOption blank;
                blank.raw_line = line;
                blank.name = "";
                current_section->options.push_back(blank);
            } else {
                header_lines_.push_back(line);
            }
            continue;
        }

        // Comment line
        if (trimmed[0] == ';' || trimmed[0] == '#') {
            if (current_section) {
                pending_comments.push_back(line);
            } else {
                header_lines_.push_back(line);
            }
            continue;
        }

        // Section header
        if (trimmed[0] == '[') {
            size_t end = trimmed.find(']');
            if (end != std::string::npos) {
                IniSection sec;
                sec.original_name = trimmed.substr(1, end - 1);
                sec.name = to_lower(sec.original_name);
                sec.raw_header = line;
                sec.leading_comments = pending_comments;
                pending_comments.clear();
                sections_.push_back(sec);
                current_section = &sections_.back();
                continue;
            }
        }

        // Option line
        if (current_section) {
            // Find the = separator
            size_t eq_pos;
            if (settings_.right_split) {
                eq_pos = trimmed.rfind('=');
            } else {
                eq_pos = trimmed.find('=');
            }

            if (eq_pos != std::string::npos) {
                IniOption opt;
                opt.raw_line = line;
                opt.comments = pending_comments;
                pending_comments.clear();

                std::string key = trimmed.substr(0, eq_pos);
                std::string val = trimmed.substr(eq_pos + 1);
                key = trim(key);
                val = trim(val);

                // Handle inline comments
                if (settings_.inline_comments && !val.empty()) {
                    size_t comment_pos = val.find(';');
                    if (comment_pos != std::string::npos && comment_pos > 0) {
                        opt.inline_comment = val.substr(comment_pos);
                        val = trim(val.substr(0, comment_pos));
                    }
                }

                opt.original_name = key;
                opt.name = to_lower(key);
                opt.value = val;
                current_section->options.push_back(opt);
            } else {
                // Line without '=' — store as-is
                IniOption opt;
                opt.raw_line = line;
                opt.name = "";
                opt.comments = pending_comments;
                pending_comments.clear();
                current_section->options.push_back(opt);
            }
        } else {
            // Line before any section — store in header
            header_lines_.push_back(line);
        }
    }

    // Any trailing pending comments go to last section or header
    if (!pending_comments.empty()) {
        if (current_section) {
            for (auto& c : pending_comments) {
                IniOption opt;
                opt.raw_line = c;
                opt.name = "";
                current_section->options.push_back(opt);
            }
        } else {
            for (auto& c : pending_comments) {
                header_lines_.push_back(c);
            }
        }
    }
}

void IniHandler::set_option(const std::string& section, const std::string& option,
                            const std::string& value) {
    IniSection* sec = get_section(section);
    if (!sec) {
        // Create new section
        IniSection new_sec;
        new_sec.name = to_lower(section);
        new_sec.original_name = section;
        new_sec.raw_header = "[" + section + "]";
        sections_.push_back(new_sec);
        sec = &sections_.back();
    }
    sec->set_option(option, value, settings_.option_value_spacing);
    modified_ = true;
}

void IniHandler::set_option(const std::string& section, const std::string& option,
                            int value) {
    set_option(section, option, std::to_string(value));
}

void IniHandler::set_option(const std::string& section, const std::string& option,
                            double value) {
    char buf[64];
    std::snprintf(buf, sizeof(buf), "%g", value);
    set_option(section, option, std::string(buf));
}

std::string IniHandler::get_option(const std::string& section,
                                   const std::string& option) const {
    const IniSection* sec = get_section(section);
    if (!sec) return "";
    const IniOption* opt = sec->find_option(option);
    if (!opt) return "";
    return opt->value;
}

void IniHandler::remove_option(const std::string& option,
                                const std::string& section_name,
                                const std::string& option_value,
                                bool not_equal) {
    const std::string* match_val = option_value.empty() ? nullptr : &option_value;
    if (!section_name.empty()) {
        IniSection* sec = get_section(section_name);
        if (sec) {
            if (sec->remove_option(option, match_val, not_equal))
                modified_ = true;
        }
    } else {
        for (auto& sec : sections_) {
            if (sec.remove_option(option, match_val, not_equal))
                modified_ = true;
        }
    }
}

void IniHandler::remove_section(const std::string& section) {
    std::string lower = to_lower(section);
    auto it = std::remove_if(sections_.begin(), sections_.end(),
        [&lower](const IniSection& s) { return s.name == lower; });
    if (it != sections_.end()) {
        sections_.erase(it, sections_.end());
        modified_ = true;
    }
}

IniSection* IniHandler::get_section(const std::string& name) {
    std::string lower = to_lower(name);
    for (auto& sec : sections_) {
        if (sec.name == lower) return &sec;
    }
    return nullptr;
}

const IniSection* IniHandler::get_section(const std::string& name) const {
    std::string lower = to_lower(name);
    for (auto& sec : sections_) {
        if (sec.name == lower) return &sec;
    }
    return nullptr;
}

std::map<std::string, std::string> IniHandler::get_option_values(
    const std::string& option, const std::string& section_name) const {
    std::map<std::string, std::string> results;
    if (!section_name.empty()) {
        const IniSection* sec = get_section(section_name);
        if (sec) {
            auto vals = sec->get_option_values(option);
            for (auto& kv : vals) {
                results[kv.first] = kv.second;
            }
        }
    } else {
        for (auto& sec : sections_) {
            auto vals = sec.get_option_values(option);
            for (auto& kv : vals) {
                results[kv.first] = kv.second;
            }
        }
    }
    return results;
}

bool IniHandler::is_modified() const {
    if (modified_) return true;
    for (auto& sec : sections_) {
        for (auto& opt : sec.options) {
            if (opt.modified) return true;
        }
    }
    return false;
}

std::string IniHandler::to_string() const {
    std::ostringstream out;

    // Header lines (before first section)
    for (auto& line : header_lines_) {
        out << line << "\n";
    }

    bool first_section = true;
    for (auto& sec : sections_) {
        // Add spacing between sections
        if (settings_.add_section_spacing && !first_section) {
            out << "\n";
        }

        // Leading comments before section header
        for (auto& c : sec.leading_comments) {
            out << c << "\n";
        }

        // Section header
        out << sec.raw_header << "\n";

        // Options
        for (auto& opt : sec.options) {
            // Output leading comments
            for (auto& c : opt.comments) {
                out << c << "\n";
            }

            if (opt.name.empty()) {
                // Blank line or raw unparsed line
                out << opt.raw_line << "\n";
            } else if (opt.modified) {
                // Rebuilt line
                if (settings_.option_value_spacing) {
                    out << opt.original_name << " = " << opt.value;
                } else {
                    out << opt.original_name << "=" << opt.value;
                }
                if (!opt.inline_comment.empty()) {
                    out << " " << opt.inline_comment;
                }
                out << "\n";
            } else {
                // Original line preserved
                out << opt.raw_line << "\n";
            }
        }

        first_section = false;
    }

    return out.str();
}

} // namespace bridge
