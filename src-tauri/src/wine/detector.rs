#[path = "detector/install.rs"]
mod install;
#[path = "detector/remote.rs"]
mod remote;
#[path = "detector/scan.rs"]
mod scan;

pub use install::{delete_local_proton, download_and_install_proton};
pub use remote::{fetch_remote_proton_versions, RemoteWineVersion};
pub use scan::{find_steam_linux_runtime, get_steam_root_path, scan_all_versions};
