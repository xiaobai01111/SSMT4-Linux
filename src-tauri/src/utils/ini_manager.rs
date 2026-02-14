use encoding_rs::SHIFT_JIS;
use std::collections::BTreeMap;
use std::path::Path;

pub type IniData = BTreeMap<String, BTreeMap<String, String>>;

pub fn load_ini(path: &Path) -> Result<IniData, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read INI file {}: {}", path.display(), e))?;

    let content = match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            let (decoded, _, _) = SHIFT_JIS.decode(&bytes);
            decoded.into_owned()
        }
    };

    let mut data: IniData = BTreeMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].trim().to_string();
            data.entry(current_section.clone())
                .or_default();
            continue;
        }

        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let value = trimmed[eq_pos + 1..].trim().to_string();
            data.entry(current_section.clone())
                .or_default()
                .insert(key, value);
        }
    }

    Ok(data)
}

#[allow(dead_code)]
pub fn get_value(data: &IniData, section: &str, key: &str) -> Option<String> {
    data.get(section).and_then(|s| s.get(key).cloned())
}

pub fn set_value(data: &mut IniData, section: &str, key: &str, value: &str) {
    data.entry(section.to_string())
        .or_default()
        .insert(key.to_string(), value.to_string());
}

pub fn remove_value(data: &mut IniData, section: &str, key: &str) {
    if let Some(section_data) = data.get_mut(section) {
        section_data.remove(key);
    }
}

pub fn save_ini(data: &IniData, path: &Path) -> Result<(), String> {
    let mut content = String::new();
    for (section, entries) in data {
        if !section.is_empty() {
            content.push_str(&format!("[{}]\n", section));
        }
        for (key, value) in entries {
            content.push_str(&format!("{}={}\n", key, value));
        }
        content.push('\n');
    }

    std::fs::write(path, content)
        .map_err(|e| format!("Failed to write INI file {}: {}", path.display(), e))
}
