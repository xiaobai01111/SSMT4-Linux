use serde::Serialize;

/// Canonical game identity mapping loaded from SQLite.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameIdentity {
    pub canonical_key: String,
    pub legacy_aliases: Vec<String>,
    pub display_name_en: String,
}

pub fn all_identities() -> Vec<GameIdentity> {
    crate::configs::database::list_identity_records()
        .into_iter()
        .map(|item| GameIdentity {
            canonical_key: item.canonical_key,
            legacy_aliases: item.legacy_aliases,
            display_name_en: item.display_name_en,
        })
        .collect()
}

pub fn normalize_game_key_or_alias(input: &str) -> Option<String> {
    crate::configs::database::resolve_game_key_or_alias(input)
}

pub fn to_canonical_or_keep(input: &str) -> String {
    normalize_game_key_or_alias(input)
        .unwrap_or_else(|| input.trim().to_string())
}

#[allow(dead_code)]
pub fn canonical_to_legacy_primary(canonical: &str) -> Option<String> {
    let aliases = legacy_aliases_for_canonical(canonical);
    aliases
        .into_iter()
        .find(|alias| !alias.eq_ignore_ascii_case("WuWa"))
        .or_else(|| legacy_aliases_for_canonical(canonical).into_iter().next())
}

pub fn legacy_aliases_for_canonical(canonical: &str) -> Vec<String> {
    crate::configs::database::list_aliases_for_canonical(canonical)
}

pub fn display_name_en_for_key(input: &str) -> Option<String> {
    crate::configs::database::display_name_en_for_key(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_legacy_alias_to_canonical() {
        assert_eq!(
            normalize_game_key_or_alias("WWMI"),
            Some("WutheringWaves".to_string())
        );
        assert_eq!(
            normalize_game_key_or_alias("WuWa"),
            Some("WutheringWaves".to_string())
        );
        assert_eq!(
            normalize_game_key_or_alias("SRMI"),
            Some("HonkaiStarRail".to_string())
        );
    }

    #[test]
    fn keeps_unknown_keys() {
        assert_eq!(to_canonical_or_keep("AEMI"), "AEMI".to_string());
    }
}
