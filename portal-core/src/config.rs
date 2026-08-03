use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::model::Device;
use crate::theme::Theme;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub executables: Executables,
    pub devices: BTreeMap<String, DeviceMetadata>,
    pub theme: Theme,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Executables {
    pub tailscale: Option<PathBuf>,
    pub rustdesk: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DeviceMetadata {
    pub nickname: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read config at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse config at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid config at {path}: {message}")]
    Validation { path: PathBuf, message: String },
}

pub(crate) fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    let mut config: Config = toml::from_str(&source).map_err(|source| ConfigError::Parse {
        path: path.to_owned(),
        source,
    })?;
    validate_config(path, &mut config)?;
    Ok(config)
}

fn validate_config(path: &Path, config: &mut Config) -> Result<(), ConfigError> {
    let mut nicknames = HashSet::new();
    for (hostname, metadata) in &mut config.devices {
        if let Some(nickname) = &metadata.nickname {
            if nickname.trim().is_empty() {
                return validation_error(
                    path,
                    format!("device {hostname:?} has an empty nickname"),
                );
            }
            if !nicknames.insert(nickname.to_lowercase()) {
                return validation_error(
                    path,
                    format!("nickname {nickname:?} is assigned to more than one device"),
                );
            }
        }

        let mut unique_tags = HashSet::new();
        metadata.tags.retain(|tag| unique_tags.insert(tag.clone()));
        if metadata.tags.iter().any(|tag| tag.trim().is_empty()) {
            return validation_error(
                path,
                format!("device {hostname:?} has an empty or whitespace-only tag"),
            );
        }
    }
    Ok(())
}

fn validation_error<T>(path: &Path, message: String) -> Result<T, ConfigError> {
    Err(ConfigError::Validation {
        path: path.to_owned(),
        message,
    })
}

pub(crate) fn merge_metadata(devices: &mut [Device], config: &Config) {
    for device in devices.iter_mut() {
        let Some(metadata) = config
            .devices
            .iter()
            .find(|(hostname, _)| hostname.eq_ignore_ascii_case(&device.hostname))
            .map(|(_, metadata)| metadata)
        else {
            continue;
        };
        device.nickname.clone_from(&metadata.nickname);
        device.tags.clone_from(&metadata.tags);
        device.favorite = metadata.favorite;
        device.notes.clone_from(&metadata.notes);
    }

    devices.sort_by(|left, right| {
        left.display_name()
            .to_lowercase()
            .cmp(&right.display_name().to_lowercase())
            .then_with(|| left.hostname.cmp(&right.hostname))
    });
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::{IpAddr, Ipv4Addr};

    use tempfile::tempdir;

    use crate::model::Device;

    use super::{ConfigError, load_config, merge_metadata};

    #[test]
    fn missing_config_is_empty() {
        let temp = tempdir().unwrap();
        let config = load_config(&temp.path().join("missing.toml")).unwrap();

        assert!(config.devices.is_empty());
        assert!(config.executables.tailscale.is_none());
        assert!(config.executables.rustdesk.is_none());
        assert_eq!(config.theme, crate::Theme::default());
    }

    #[test]
    fn minimal_empty_config_is_valid() {
        let config = load_ok("");

        assert!(config.devices.is_empty());
        assert!(config.executables.tailscale.is_none());
        assert!(config.executables.rustdesk.is_none());
        assert_eq!(config.theme, crate::Theme::default());
    }

    #[test]
    fn loads_minimal_and_full_config() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            r#"
                [executables]
                tailscale = "/custom/tailscale"
                rustdesk = "/custom/rustdesk"

                [devices.workstation]
                nickname = "rig"
                tags = ["desktop", "primary"]
                favorite = true
                notes = "Main workstation"
            "#,
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        let metadata = &config.devices["workstation"];

        assert_eq!(
            config.executables.tailscale.unwrap().to_string_lossy(),
            "/custom/tailscale"
        );
        assert_eq!(
            config.executables.rustdesk.unwrap().to_string_lossy(),
            "/custom/rustdesk"
        );
        assert_eq!(metadata.nickname.as_deref(), Some("rig"));
        assert_eq!(metadata.tags, ["desktop", "primary"]);
        assert!(metadata.favorite);
        assert_eq!(metadata.notes.as_deref(), Some("Main workstation"));
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = load_from("[devices.host]\npassword = \"secret\"");

        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn loads_full_and_partial_themes() {
        let full = load_ok(
            r##"
                [theme]
                background = "#010203"
                foreground = "#111213"
                accent = "#212223"
                muted = "#313233"
                signal = "#414243"
            "##,
        );
        assert_eq!(full.theme.background.red, 0x01);
        assert_eq!(full.theme.foreground.green, 0x12);
        assert_eq!(full.theme.accent.blue, 0x23);
        assert_eq!(full.theme.muted.red, 0x31);
        assert_eq!(full.theme.signal.blue, 0x43);

        let partial = load_ok("[theme]\naccent = \"#010203\"");
        assert_eq!(partial.theme.accent.red, 0x01);
        assert_eq!(partial.theme.background, crate::Theme::default().background);
        assert_eq!(partial.theme.signal, crate::Theme::default().signal);
    }

    #[test]
    fn rejects_invalid_or_unknown_theme_values() {
        for source in [
            "[theme]\naccent = \"7AA2F7\"",
            "[theme]\naccent = \"#ABC\"",
            "[theme]\naccent = \"#7AA2F7FF\"",
            "[theme]\naccent = \"#ZZZZZZ\"",
            "[theme]\nselection = \"#7AA2F7\"",
        ] {
            assert!(matches!(load_from(source), ConfigError::Parse { .. }));
        }
    }

    #[test]
    fn rejects_empty_nickname_and_whitespace_tag() {
        assert!(matches!(
            load_from("[devices.host]\nnickname = \"  \""),
            ConfigError::Validation { .. }
        ));
        assert!(matches!(
            load_from("[devices.host]\ntags = [\"valid\", \" \"]"),
            ConfigError::Validation { .. }
        ));
    }

    #[test]
    fn removes_exact_duplicate_tags_in_user_order() {
        let config = load_ok("[devices.host]\ntags = [\"one\", \"two\", \"one\", \"One\"]");

        assert_eq!(config.devices["host"].tags, ["one", "two", "One"]);
    }

    #[test]
    fn rejects_duplicate_nicknames_case_insensitively() {
        let error =
            load_from("[devices.one]\nnickname = \"Rig\"\n[devices.two]\nnickname = \"rig\"");

        assert!(matches!(error, ConfigError::Validation { .. }));
    }

    #[test]
    fn merges_case_insensitively_without_creating_dormant_devices() {
        let config = load_ok(
            r#"
                [devices.WORKSTATION]
                nickname = "rig"
                tags = ["desktop"]
                favorite = true
                notes = "Primary"

                [devices.undiscovered]
                nickname = "dormant"
            "#,
        );
        let mut devices = vec![device("node-one", "workstation")];

        merge_metadata(&mut devices, &config);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].nickname.as_deref(), Some("rig"));
        assert_eq!(devices[0].tags, ["desktop"]);
        assert!(devices[0].favorite);
        assert_eq!(devices[0].notes.as_deref(), Some("Primary"));
    }

    fn load_ok(source: &str) -> super::Config {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, source).unwrap();
        load_config(&path).unwrap()
    }

    fn load_from(source: &str) -> ConfigError {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, source).unwrap();
        load_config(&path).unwrap_err()
    }

    fn device(id: &str, hostname: &str) -> Device {
        Device {
            id: id.to_owned(),
            hostname: hostname.to_owned(),
            dns_name: None,
            nickname: None,
            ips: vec![IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1))],
            preferred_ip: IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1)),
            os: "linux".to_owned(),
            online: true,
            tags: Vec::new(),
            favorite: false,
            notes: None,
            last_connected_at: None,
        }
    }
}
