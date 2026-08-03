use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use thiserror::Error;

use crate::model::{Device, LocalDevice};
use crate::platform::{ExecutableNotFound, resolve_executable};

#[derive(Debug, Error)]
pub enum TailscaleParseError {
    #[error("Tailscale returned invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Tailscale reported {invalid_count} peer(s), but none had a usable IP address")]
    NoUsablePeers { invalid_count: usize },
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    ExecutableNotFound(#[from] ExecutableNotFound),
    #[error("could not start Tailscale at {path}: {source}")]
    Start {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("Tailscale is not ready: {message}")]
    Unhealthy { message: String },
    #[error(
        "Tailscale at {path} exited with {status}: {stderr}",
        status = status.map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
    )]
    Exit {
        path: std::path::PathBuf,
        status: Option<i32>,
        stderr: String,
    },
    #[error("Tailscale output was not valid UTF-8")]
    InvalidUtf8,
    #[error(transparent)]
    Parse(#[from] TailscaleParseError),
}

#[derive(Deserialize)]
struct Status {
    #[serde(rename = "Self")]
    local: Option<Peer>,
    #[serde(rename = "Peer", default)]
    peers: HashMap<String, Peer>,
}

#[derive(Deserialize)]
struct Peer {
    #[serde(rename = "ID", default)]
    id: String,
    #[serde(rename = "HostName", default)]
    hostname: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "OS", default)]
    os: String,
    #[serde(rename = "TailscaleIPs", default)]
    ips: Vec<String>,
    #[serde(rename = "Online", default)]
    online: bool,
}

#[derive(Debug)]
pub struct TailscaleSnapshot {
    pub local_device: Option<LocalDevice>,
    pub devices: Vec<Device>,
}

/// Runs one on-demand Tailscale discovery without a shell.
pub fn discover(tailscale_override: Option<&Path>) -> Result<Vec<Device>, DiscoveryError> {
    discover_snapshot(tailscale_override).map(|snapshot| snapshot.devices)
}

/// Runs one on-demand Tailscale discovery including the local device identity.
pub fn discover_snapshot(
    tailscale_override: Option<&Path>,
) -> Result<TailscaleSnapshot, DiscoveryError> {
    let executable = resolve_executable("tailscale", tailscale_override)?;
    let output = Command::new(&executable)
        .args(["status", "--json"])
        .output()
        .map_err(|source| DiscoveryError::Start {
            path: executable.clone(),
            source,
        })?;

    if !output.status.success() {
        let stderr = concise_stderr(&output.stderr);
        if indicates_unhealthy(&stderr) {
            return Err(DiscoveryError::Unhealthy { message: stderr });
        }
        return Err(DiscoveryError::Exit {
            path: executable,
            status: output.status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|_| DiscoveryError::InvalidUtf8)?;
    parse_snapshot(&stdout).map_err(DiscoveryError::from)
}

/// Parses a single `tailscale status --json` response without performing I/O.
pub fn parse_status(json: &str) -> Result<Vec<Device>, TailscaleParseError> {
    parse_snapshot(json).map(|snapshot| snapshot.devices)
}

/// Parses peers and the local Tailscale identity from a single status response.
pub fn parse_snapshot(json: &str) -> Result<TailscaleSnapshot, TailscaleParseError> {
    let status: Status = serde_json::from_str(json)?;
    let local_id = status.local.as_ref().map(|peer| peer.id.as_str());
    let local_device = status.local.as_ref().and_then(local_device);
    let mut invalid_count = 0;
    let mut considered_count = 0;
    let mut devices = Vec::new();

    for peer in status.peers.into_values() {
        if local_id == Some(peer.id.as_str()) {
            continue;
        }
        if is_mobile_os(&peer.os) {
            continue;
        }
        considered_count += 1;

        let ips: Vec<IpAddr> = peer
            .ips
            .iter()
            .filter_map(|value| value.parse().ok())
            .collect();
        let Some(preferred_ip) = ips
            .iter()
            .copied()
            .find(IpAddr::is_ipv4)
            .or_else(|| ips.first().copied())
        else {
            invalid_count += 1;
            continue;
        };

        let machine_name = machine_name(&peer);
        devices.push(Device {
            id: peer.id,
            machine_name,
            hostname: peer.hostname,
            dns_name: normalize_dns_name(&peer.dns_name),
            nickname: None,
            ips,
            preferred_ip,
            os: peer.os,
            online: peer.online,
            tags: Vec::new(),
            favorite: false,
            notes: None,
            last_connected_at: None,
        });
    }

    if considered_count > 0 && devices.is_empty() {
        return Err(TailscaleParseError::NoUsablePeers { invalid_count });
    }

    devices.sort_by(|left, right| {
        left.display_name()
            .to_lowercase()
            .cmp(&right.display_name().to_lowercase())
            .then_with(|| left.hostname.cmp(&right.hostname))
    });
    Ok(TailscaleSnapshot {
        local_device,
        devices,
    })
}

fn local_device(peer: &Peer) -> Option<LocalDevice> {
    let ips: Vec<IpAddr> = peer
        .ips
        .iter()
        .filter_map(|value| value.parse().ok())
        .collect();
    let preferred_ip = ips
        .iter()
        .copied()
        .find(IpAddr::is_ipv4)
        .or_else(|| ips.first().copied())?;
    Some(LocalDevice {
        machine_name: machine_name(peer),
        preferred_ip,
    })
}

fn machine_name(peer: &Peer) -> String {
    normalize_dns_name(&peer.dns_name)
        .and_then(|dns_name| dns_name.split('.').next().map(str::to_owned))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| peer.hostname.clone())
}

fn normalize_dns_name(value: &str) -> Option<String> {
    let normalized = value.trim_end_matches('.');
    (!normalized.is_empty()).then(|| normalized.to_owned())
}

fn is_mobile_os(os: &str) -> bool {
    os.eq_ignore_ascii_case("ios") || os.eq_ignore_ascii_case("android")
}

fn concise_stderr(bytes: &[u8]) -> String {
    let value = String::from_utf8_lossy(bytes);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "(no stderr)".to_owned()
    } else {
        trimmed.chars().take(500).collect()
    }
}

fn indicates_unhealthy(stderr: &str) -> bool {
    let message = stderr.to_ascii_lowercase();
    ["logged out", "not logged in", "stopped", "not running"]
        .iter()
        .any(|needle| message.contains(needle))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::{TailscaleParseError, parse_snapshot, parse_status};

    const MIXED: &str = include_str!("../tests/fixtures/tailscale-status.json");
    const EMPTY: &str = include_str!("../tests/fixtures/tailscale-empty.json");
    const IPV6_ONLY: &str = include_str!("../tests/fixtures/tailscale-ipv6-only.json");

    #[test]
    fn parses_mixed_peers_and_ignores_unknown_fields() {
        let devices = parse_status(MIXED).unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].hostname, "alpha-laptop");
        assert!(!devices[0].online);
        assert_eq!(devices[1].hostname, "zeta-workstation");
        assert!(devices[1].online);
        assert!(
            devices.iter().all(|device| !matches!(
                device.os.to_ascii_lowercase().as_str(),
                "ios" | "android"
            ))
        );
    }

    #[test]
    fn normalizes_dns_and_prefers_ipv4_while_preserving_ips() {
        let devices = parse_status(MIXED).unwrap();
        let device = &devices[1];

        assert_eq!(device.dns_name.as_deref(), Some("zeta.example.test"));
        assert_eq!(
            device.preferred_ip,
            IpAddr::V4(Ipv4Addr::new(100, 100, 0, 12))
        );
        assert_eq!(device.ips.len(), 2);
    }

    #[test]
    fn uses_tailscale_machine_name_instead_of_os_hostname() {
        let devices = parse_status(
            r#"{
                "Peer": {
                    "one": {
                        "ID":"node-one",
                        "HostName":"os-hostname",
                        "DNSName":"portal-machine.example.test.",
                        "TailscaleIPs":["100.100.0.2"]
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(devices[0].machine_name, "portal-machine");
        assert_eq!(devices[0].hostname, "os-hostname");
        assert_eq!(devices[0].display_name(), "portal-machine");
    }

    #[test]
    fn parses_local_machine_name_and_preferred_ip() {
        let snapshot = parse_snapshot(
            r#"{
                "Self": {
                    "ID":"node-self",
                    "HostName":"local-os-name",
                    "DNSName":"local-machine.example.test.",
                    "TailscaleIPs":["fd7a:115c:a1e0::1", "100.100.0.1"]
                }
            }"#,
        )
        .unwrap();
        let local = snapshot.local_device.unwrap();

        assert_eq!(local.machine_name, "local-machine");
        assert_eq!(
            local.preferred_ip,
            IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1))
        );
    }

    #[test]
    fn ignores_an_invalid_ip_when_another_is_valid() {
        let devices = parse_status(MIXED).unwrap();
        let device = &devices[0];

        assert_eq!(device.ips.len(), 1);
        assert_eq!(
            device.preferred_ip,
            IpAddr::V4(Ipv4Addr::new(100, 100, 0, 20))
        );
    }

    #[test]
    fn parses_empty_peer_set() {
        assert!(parse_status(EMPTY).unwrap().is_empty());
    }

    #[test]
    fn falls_back_to_ipv6() {
        let devices = parse_status(IPV6_ONLY).unwrap();

        assert_eq!(
            devices[0].preferred_ip,
            IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 42))
        );
    }

    #[test]
    fn excludes_self_if_repeated_in_peer_map() {
        let devices = parse_status(
            r#"{
                "Self": {"ID":"node-self"},
                "Peer": {
                    "self": {
                        "ID":"node-self",
                        "HostName":"local",
                        "TailscaleIPs":["100.100.0.1"]
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(devices.is_empty());
    }

    #[test]
    fn skips_unusable_peer_when_other_peer_is_valid() {
        let devices = parse_status(MIXED).unwrap();

        assert!(
            devices
                .iter()
                .all(|device| device.hostname != "broken-peer")
        );
    }

    #[test]
    fn errors_if_all_peers_have_no_usable_ip() {
        let error = parse_status(
            r#"{"Peer":{"one":{"ID":"node-one","HostName":"one","TailscaleIPs":["bad"]}}}"#,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            TailscaleParseError::NoUsablePeers { invalid_count: 1 }
        ));
    }

    #[test]
    fn mobile_only_peer_set_returns_empty() {
        let devices = parse_status(
            r#"{
                "Peer": {
                    "phone": {
                        "ID":"node-phone",
                        "HostName":"phone",
                        "OS":"iOS",
                        "TailscaleIPs":["100.100.0.30"]
                    },
                    "tablet": {
                        "ID":"node-tablet",
                        "HostName":"tablet",
                        "OS":"ANDROID",
                        "TailscaleIPs":["100.100.0.31"]
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(devices.is_empty());
    }

    #[test]
    fn object_order_does_not_change_output_order() {
        let first = parse_status(MIXED).unwrap();
        let reversed = parse_status(
            r#"{
                "Peer": {
                    "a": {"ID":"node-a","HostName":"zeta","TailscaleIPs":["100.100.0.2"]},
                    "z": {"ID":"node-z","HostName":"alpha","TailscaleIPs":["100.100.0.3"]}
                }
            }"#,
        )
        .unwrap();

        assert_eq!(first[0].hostname, "alpha-laptop");
        assert_eq!(reversed[0].hostname, "alpha");
        assert_eq!(reversed[1].hostname, "zeta");
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(matches!(
            parse_status("{"),
            Err(TailscaleParseError::InvalidJson(_))
        ));
    }

    #[test]
    #[ignore = "requires an installed, authenticated Tailscale client"]
    fn live_discovery_smoke_test() {
        let devices = super::discover(None).unwrap();
        assert!(!devices.is_empty());
    }
}
