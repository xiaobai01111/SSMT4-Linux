mod app_config_store;
mod core;
mod game_catalog_seed;
mod games;
mod hash_cache;
mod proton;
mod settings;

#[allow(unused_imports)]
pub use app_config_store::{load_app_config, save_app_config, APP_CONFIG_STORAGE_KEY};
#[allow(unused_imports)]
pub use games::{
    delete_game_config, delete_game_config_v2, display_name_en_for_key, get_game_config,
    get_game_config_exact, get_game_config_v2, get_game_config_v2_exact, has_game_config_exact,
    list_aliases_for_canonical, list_game_names, list_game_names_raw, list_game_names_v2_raw,
    list_game_preset_rows, list_identity_records, rename_game_keys, resolve_game_key_or_alias,
    set_game_config, set_game_config_exact, set_game_config_v2, set_game_config_v2_exact,
    set_game_key_alias, IdentityRecord,
};
#[allow(unused_imports)]
pub use hash_cache::{get_cached_md5, set_cached_md5};
#[allow(unused_imports)]
pub use proton::{
    list_proton_family_rows, list_proton_source_rows, replace_proton_catalog_rows,
    ProtonFamilyRecord, ProtonSourceRecord,
};
#[allow(unused_imports)]
pub use settings::{
    list_setting_records, read_migration_meta, read_setting_record, read_setting_value,
    write_migration_meta, write_setting_value, write_settings_batch, SettingRecord,
};
