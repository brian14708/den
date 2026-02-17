use std::path::{Path, PathBuf};

use serde::Deserialize;
use xdg::BaseDirectories;

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_RUST_LOG: &str = "info";
const DEFAULT_RP_ID: &str = "localhost";
const DEFAULT_RP_ORIGIN: &str = "http://localhost:3000";

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    port: Option<u16>,
    rust_log: Option<String>,
    rp_id: Option<String>,
    rp_origin: Option<String>,
    allowed_hosts: Option<Vec<String>>,
    database_path: Option<String>,
}

#[derive(Debug)]
pub struct AppConfig {
    pub port: u16,
    pub rust_log: String,
    pub rp_id: String,
    pub rp_origin: String,
    pub allowed_hosts: Vec<String>,
    pub database_path: PathBuf,
}

#[derive(Debug)]
struct DenPaths {
    config_path: PathBuf,
    default_database_path: PathBuf,
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    let s = value?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

fn resolve_den_paths() -> DenPaths {
    let xdg = BaseDirectories::with_prefix("den");
    DenPaths {
        config_path: xdg
            .place_config_file("config.toml")
            .unwrap_or_else(|e| panic!("failed to prepare config path: {e}")),
        default_database_path: xdg
            .get_data_home()
            .expect("XDG data home is not available")
            .join("den.db"),
    }
}

fn default_config_contents() -> String {
    format!(
        "port = {DEFAULT_PORT}\n\
rust_log = \"{DEFAULT_RUST_LOG}\"\n\
rp_id = \"{DEFAULT_RP_ID}\"\n\
rp_origin = \"{DEFAULT_RP_ORIGIN}\"\n\
allowed_hosts = []\n"
    )
}

fn ensure_config_file(config_path: &Path) {
    let parent = config_path
        .parent()
        .expect("config path must have a parent");
    std::fs::create_dir_all(parent).unwrap_or_else(|e| {
        panic!(
            "failed to create config directory at {}: {e}",
            parent.display()
        )
    });
    if config_path.exists() {
        return;
    }
    std::fs::write(config_path, default_config_contents()).unwrap_or_else(|e| {
        panic!(
            "failed to write default config file at {}: {e}",
            config_path.display()
        )
    });
}

fn read_file_config(config_path: &Path) -> FileConfig {
    let contents = std::fs::read_to_string(config_path).unwrap_or_else(|e| {
        panic!(
            "failed to read config file at {}: {e}",
            config_path.display()
        )
    });
    toml::from_str(&contents).unwrap_or_else(|e| {
        panic!(
            "invalid TOML in config file at {}: {e}",
            config_path.display()
        )
    })
}

pub fn load_app_config() -> AppConfig {
    let den_paths = resolve_den_paths();
    ensure_config_file(&den_paths.config_path);
    let file = read_file_config(&den_paths.config_path);

    let allowed_hosts = file
        .allowed_hosts
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();

    AppConfig {
        port: file.port.unwrap_or(DEFAULT_PORT),
        rust_log: non_empty_string(file.rust_log).unwrap_or_else(|| DEFAULT_RUST_LOG.to_owned()),
        rp_id: non_empty_string(file.rp_id).unwrap_or_else(|| DEFAULT_RP_ID.to_owned()),
        rp_origin: non_empty_string(file.rp_origin).unwrap_or_else(|| DEFAULT_RP_ORIGIN.to_owned()),
        allowed_hosts,
        database_path: non_empty_string(file.database_path)
            .map(PathBuf::from)
            .unwrap_or(den_paths.default_database_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_does_not_hardcode_database_path() {
        let config = default_config_contents();
        assert!(!config.contains("database_path"));
    }
}
