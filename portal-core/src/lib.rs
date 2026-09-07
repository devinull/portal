//! Shared discovery and action logic for Portal frontends.

mod config;
pub mod model;
pub mod platform;
mod probe;
mod rustdesk;
mod state;
pub mod tailscale;
mod theme;

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::{collections::HashSet, net::IpAddr};

use directories::ProjectDirs;
use thiserror::Error;

use config::{
    Config, ConfigError, DeviceMetadata, load_config, merge_metadata, save_config, validate_config,
};
pub use model::{Action, Device, Launch, LocalDevice};
pub use probe::{ProbeError, ProbeMeasurement};
pub use rustdesk::{
    LaunchOutcome, ProvisionError, ProvisionOutcome, ProvisionPlan, ProvisionStep, RustDeskError,
    ServiceState,
};
use state::{StateError, load_state, merge_state};
pub use tailscale::{
    DiscoveryError, TailscaleParseError, discover, discover_snapshot, parse_snapshot, parse_status,
};
pub use theme::{Theme, ThemeColor, ThemeColorParseError};

#[derive(Debug, Error)]
pub enum PortalError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    RustDesk(#[from] RustDeskError),
    #[error(transparent)]
    Provision(#[from] ProvisionError),
    #[error("could not determine platform config and local-data directories")]
    ProjectDirectoriesUnavailable,
    #[error("Portal configuration lock is unavailable")]
    ConfigLock,
}

#[derive(Debug)]
pub struct Snapshot {
    pub local_device: Option<LocalDevice>,
    pub devices: Vec<Device>,
    pub warnings: Vec<String>,
}

pub struct Portal {
    config: RwLock<Config>,
    config_path: PathBuf,
    state_path: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeviceConfig {
    pub nickname: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub notes: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyStatus {
    pub tailscale: bool,
    pub rustdesk: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveCandidate {
    pub id: String,
    pub display_name: String,
    pub hostname: String,
    pub preferred_ip: IpAddr,
}

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("no device matches {query:?}")]
    NotFound { query: String },
    #[error(
        "device identifier {query:?} is ambiguous: {candidates}",
        candidates = format_candidates(.candidates)
    )]
    Ambiguous {
        query: String,
        candidates: Vec<ResolveCandidate>,
    },
}

pub fn resolve<'a>(devices: &'a [Device], query: &str) -> Result<&'a Device, ResolveError> {
    let trimmed = query.trim();
    let normalized_name = trimmed.trim_end_matches('.');
    let query_ip = trimmed.parse::<IpAddr>().ok();
    let mut seen_ids = HashSet::new();
    let matches: Vec<&Device> = devices
        .iter()
        .filter(|device| {
            device
                .nickname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(normalized_name))
                || device.hostname.eq_ignore_ascii_case(normalized_name)
                || device.machine_name.eq_ignore_ascii_case(normalized_name)
                || device
                    .dns_name
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(normalized_name))
                || query_ip.is_some_and(|ip| device.ips.contains(&ip))
        })
        .filter(|device| seen_ids.insert(device.id.as_str()))
        .collect();

    match matches.as_slice() {
        [] => Err(ResolveError::NotFound {
            query: trimmed.to_owned(),
        }),
        [device] => Ok(device),
        _ => Err(ResolveError::Ambiguous {
            query: trimmed.to_owned(),
            candidates: matches
                .into_iter()
                .map(|device| ResolveCandidate {
                    id: device.id.clone(),
                    display_name: device.display_name().to_owned(),
                    hostname: device.hostname.clone(),
                    preferred_ip: device.preferred_ip,
                })
                .collect(),
        }),
    }
}

fn format_candidates(candidates: &[ResolveCandidate]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            format!(
                "{} ({}, {})",
                candidate.display_name, candidate.hostname, candidate.preferred_ip
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl Portal {
    pub fn load(config_override: Option<&Path>) -> Result<Self, PortalError> {
        let project_dirs = ProjectDirs::from("", "", "portal")
            .ok_or(PortalError::ProjectDirectoriesUnavailable)?;
        let config_path = config_override
            .map(Path::to_owned)
            .unwrap_or_else(|| project_dirs.config_dir().join("config.toml"));
        let state_path = project_dirs.data_local_dir().join("state.json");
        let config = load_config(&config_path)?;
        Ok(Self {
            config: RwLock::new(config),
            config_path,
            state_path,
        })
    }

    pub fn snapshot(&self) -> Result<Vec<Device>, PortalError> {
        Ok(self.snapshot_with_warnings()?.devices)
    }

    #[must_use]
    pub fn theme(&self) -> Theme {
        self.config
            .read()
            .map_or_else(|_| Theme::default(), |config| config.theme)
    }

    pub fn set_theme(&self, theme: Theme) -> Result<(), PortalError> {
        self.update_config(|config| config.theme = theme)
    }

    pub fn dependency_status(&self) -> Result<DependencyStatus, PortalError> {
        let config = self.config.read().map_err(|_| PortalError::ConfigLock)?;
        Ok(DependencyStatus {
            tailscale: platform::resolve_executable(
                "tailscale",
                config.executables.tailscale.as_deref(),
            )
            .is_ok(),
            rustdesk: platform::resolve_executable(
                "rustdesk",
                config.executables.rustdesk.as_deref(),
            )
            .is_ok(),
        })
    }

    pub fn update_device_config(
        &self,
        machine_name: &str,
        hostname: &str,
        metadata: DeviceConfig,
    ) -> Result<(), PortalError> {
        let machine_name = machine_name.to_owned();
        let hostname = hostname.to_owned();
        self.update_config(move |config| {
            config.devices.retain(|key, _| {
                !key.eq_ignore_ascii_case(&machine_name) && !key.eq_ignore_ascii_case(&hostname)
            });
            let metadata = DeviceMetadata {
                nickname: metadata.nickname,
                tags: metadata.tags,
                favorite: metadata.favorite,
                notes: metadata.notes,
            };
            if metadata.nickname.is_some()
                || !metadata.tags.is_empty()
                || metadata.favorite
                || metadata.notes.is_some()
            {
                config.devices.insert(machine_name, metadata);
            }
        })
    }

    pub fn snapshot_with_warnings(&self) -> Result<Snapshot, PortalError> {
        let config = self.config.read().map_err(|_| PortalError::ConfigLock)?;
        let discovered = discover_snapshot(config.executables.tailscale.as_deref())?;
        let mut devices = discovered.devices;
        merge_metadata(&mut devices, &config);
        drop(config);
        let (state, warning) = load_state(&self.state_path);
        merge_state(&mut devices, &state);
        Ok(Snapshot {
            local_device: discovered.local_device,
            devices,
            warnings: warning.into_iter().collect(),
        })
    }

    pub fn probe(&self, device: &Device) -> Result<ProbeMeasurement, ProbeError> {
        let executable = self
            .config
            .read()
            .ok()
            .and_then(|config| config.executables.tailscale.clone());
        probe::probe(executable.as_deref(), device.preferred_ip)
    }

    pub fn launch_spec(&self, action: Action, device: &Device) -> Result<Launch, PortalError> {
        let executable = self
            .config
            .read()
            .map_err(|_| PortalError::ConfigLock)?
            .executables
            .rustdesk
            .clone();
        rustdesk::launch_spec(executable.as_deref(), action, device).map_err(PortalError::from)
    }

    pub fn launch_with_warnings(
        &self,
        action: Action,
        device: &Device,
    ) -> Result<LaunchOutcome, PortalError> {
        let launch = self.launch_spec(action, device)?;
        rustdesk::execute_launch(&launch, &self.state_path, device).map_err(PortalError::from)
    }

    pub fn launch(&self, action: Action, device: &Device) -> Result<(), PortalError> {
        self.launch_with_warnings(action, device).map(|_| ())
    }

    pub fn local_provision_plan(&self) -> Result<ProvisionPlan, PortalError> {
        let executable = self
            .config
            .read()
            .map_err(|_| PortalError::ConfigLock)?
            .executables
            .rustdesk
            .clone();
        rustdesk::local_provision_plan(executable.as_deref()).map_err(PortalError::from)
    }

    pub fn apply_local_provision_plan(
        &self,
        plan: &ProvisionPlan,
    ) -> Result<ProvisionOutcome, PortalError> {
        rustdesk::apply_local_provision_plan(plan).map_err(PortalError::from)
    }

    fn update_config(&self, update: impl FnOnce(&mut Config)) -> Result<(), PortalError> {
        let mut current = self.config.write().map_err(|_| PortalError::ConfigLock)?;
        let mut candidate = current.clone();
        update(&mut candidate);
        validate_config(&self.config_path, &mut candidate)?;
        save_config(&self.config_path, &candidate)?;
        *current = candidate;
        Ok(())
    }
}

/// Returns the current Portal core release version.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod config_update_tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::{DeviceConfig, Portal, Theme};

    #[test]
    fn device_and_theme_updates_persist_to_toml() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let portal = Portal::load(Some(&path)).unwrap();

        portal
            .update_device_config(
                "workstation",
                "workstation.example.test",
                DeviceConfig {
                    nickname: Some("rig".to_owned()),
                    tags: vec!["primary".to_owned()],
                    favorite: true,
                    notes: Some("Main".to_owned()),
                },
            )
            .unwrap();
        portal.set_theme(Theme::terminator()).unwrap();

        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("[devices.workstation]"));
        assert!(source.contains("nickname = \"rig\""));
        assert!(source.contains("accent = \"#FF1717\""));
        let reloaded = Portal::load(Some(&path)).unwrap();
        assert_eq!(reloaded.theme(), Theme::terminator());
    }

    #[test]
    fn empty_device_update_removes_stale_table() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let portal = Portal::load(Some(&path)).unwrap();
        portal
            .update_device_config(
                "workstation",
                "workstation.example.test",
                DeviceConfig {
                    nickname: Some("rig".to_owned()),
                    ..DeviceConfig::default()
                },
            )
            .unwrap();
        portal
            .update_device_config(
                "workstation",
                "workstation.example.test",
                DeviceConfig::default(),
            )
            .unwrap();

        assert!(
            !fs::read_to_string(path)
                .unwrap()
                .contains("devices.workstation")
        );
    }
}

#[cfg(test)]
mod resolution_tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use crate::{Device, ResolveError, resolve};

    #[test]
    fn resolves_exact_nickname_case_insensitively() {
        let mut devices = devices();
        devices[0].nickname = Some("Main Rig".to_owned());

        assert_eq!(resolve(&devices, "main RIG").unwrap().id, "node-one");
    }

    #[test]
    fn resolves_hostname() {
        let devices = devices();

        assert_eq!(resolve(&devices, "alpha").unwrap().id, "node-one");
    }

    #[test]
    fn resolves_dns_with_or_without_trailing_dot() {
        let devices = devices();

        assert_eq!(
            resolve(&devices, "alpha.example.test").unwrap().id,
            "node-one"
        );
        assert_eq!(
            resolve(&devices, "ALPHA.EXAMPLE.TEST.").unwrap().id,
            "node-one"
        );
    }

    #[test]
    fn resolves_ipv4_and_ipv6() {
        let devices = devices();

        assert_eq!(resolve(&devices, "100.100.0.1").unwrap().id, "node-one");
        assert_eq!(
            resolve(&devices, "fd7a:115c:a1e0::2").unwrap().id,
            "node-two"
        );
    }

    #[test]
    fn trims_query_whitespace() {
        let devices = devices();

        assert_eq!(resolve(&devices, "  alpha  ").unwrap().id, "node-one");
    }

    #[test]
    fn returns_not_found_without_fuzzy_matching() {
        let devices = devices();

        assert!(matches!(
            resolve(&devices, "alp"),
            Err(ResolveError::NotFound { .. })
        ));
    }

    #[test]
    fn reports_cross_namespace_collision_as_ambiguous() {
        let mut devices = devices();
        devices[1].nickname = Some("alpha".to_owned());

        let error = resolve(&devices, "alpha").unwrap_err();

        match error {
            ResolveError::Ambiguous { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].id, "node-one");
                assert_eq!(candidates[1].id, "node-two");
            }
            ResolveError::NotFound { .. } => panic!("expected ambiguity"),
        }
    }

    #[test]
    fn matching_two_aliases_on_same_node_counts_once() {
        let mut devices = devices();
        devices[0].nickname = Some("alpha".to_owned());

        assert_eq!(resolve(&devices, "alpha").unwrap().id, "node-one");
    }

    fn devices() -> Vec<Device> {
        vec![
            Device {
                id: "node-one".to_owned(),
                machine_name: "alpha".to_owned(),
                hostname: "alpha".to_owned(),
                dns_name: Some("alpha.example.test".to_owned()),
                nickname: None,
                ips: vec![
                    IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1)),
                    IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 1)),
                ],
                preferred_ip: IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1)),
                os: "linux".to_owned(),
                online: true,
                tags: Vec::new(),
                favorite: false,
                notes: None,
                last_connected_at: None,
            },
            Device {
                id: "node-two".to_owned(),
                machine_name: "beta".to_owned(),
                hostname: "beta".to_owned(),
                dns_name: Some("beta.example.test".to_owned()),
                nickname: None,
                ips: vec![IpAddr::V6(Ipv6Addr::new(
                    0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 2,
                ))],
                preferred_ip: IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 2)),
                os: "windows".to_owned(),
                online: false,
                tags: Vec::new(),
                favorite: false,
                notes: None,
                last_connected_at: None,
            },
        ]
    }
}
