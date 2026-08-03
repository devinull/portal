use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::Device;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct UsageState {
    pub last_connected: BTreeMap<String, u64>,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("could not create state directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not serialize usage state: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("could not write usage state at {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("system clock is before the Unix epoch")]
    InvalidSystemTime,
}

pub(crate) fn load_state(path: &Path) -> (UsageState, Option<String>) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (UsageState::default(), None);
        }
        Err(error) => {
            return (
                UsageState::default(),
                Some(format!(
                    "could not read usage state at {}: {error}; continuing with empty state",
                    path.display()
                )),
            );
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(state) => (state, None),
        Err(error) => (
            UsageState::default(),
            Some(format!(
                "could not parse usage state at {}: {error}; continuing with empty state",
                path.display()
            )),
        ),
    }
}

pub(crate) fn merge_state(devices: &mut [Device], state: &UsageState) {
    for device in devices {
        device.last_connected_at = state.last_connected.get(&device.id).copied();
    }
}

pub(crate) fn record_connection(path: &Path, device_id: &str) -> Result<(), StateError> {
    let (mut state, _) = load_state(path);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StateError::InvalidSystemTime)?
        .as_secs();
    state.last_connected.insert(device_id.to_owned(), timestamp);
    write_state(path, &state)
}

fn write_state(path: &Path, state: &UsageState) -> Result<(), StateError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| StateError::CreateDirectory {
        path: parent.to_owned(),
        source,
    })?;
    let bytes = serde_json::to_vec_pretty(state)?;
    let temp_path = temporary_path(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|source| StateError::Write {
                path: temp_path.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| StateError::Write {
                path: temp_path.clone(),
                source,
            })?;
        fs::rename(&temp_path, path).map_err(|source| StateError::Write {
            path: path.to_owned(),
            source,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::{IpAddr, Ipv4Addr};

    use tempfile::tempdir;

    use crate::model::Device;

    use super::{UsageState, load_state, merge_state, record_connection, write_state};

    #[test]
    fn missing_state_is_empty_without_warning() {
        let temp = tempdir().unwrap();
        let (state, warning) = load_state(&temp.path().join("missing.json"));

        assert!(state.last_connected.is_empty());
        assert!(warning.is_none());
    }

    #[test]
    fn valid_state_merges_only_by_node_id() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("state.json");
        fs::write(
            &path,
            r#"{"last_connected":{"node-one":42,"same-as-hostname":99}}"#,
        )
        .unwrap();
        let (state, warning) = load_state(&path);
        let mut devices = vec![
            device("node-one", "alpha"),
            device("node-two", "same-as-hostname"),
        ];

        merge_state(&mut devices, &state);

        assert!(warning.is_none());
        assert_eq!(devices[0].last_connected_at, Some(42));
        assert_eq!(devices[1].last_connected_at, None);
    }

    #[test]
    fn invalid_state_is_nonfatal() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("state.json");
        fs::write(&path, "{").unwrap();

        let (state, warning) = load_state(&path);

        assert!(state.last_connected.is_empty());
        assert!(warning.is_some());
    }

    #[test]
    fn atomic_replacement_produces_valid_json() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut state = UsageState::default();
        state.last_connected.insert("node-one".to_owned(), 1);
        write_state(&path, &state).unwrap();
        state.last_connected.insert("node-two".to_owned(), 2);
        write_state(&path, &state).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();

        assert_eq!(parsed["last_connected"]["node-one"], 1);
        assert_eq!(parsed["last_connected"]["node-two"], 2);
        assert_eq!(
            fs::read_dir(temp.path())
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1
        );
    }

    #[test]
    fn recording_connection_adds_unix_timestamp() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("nested").join("state.json");

        record_connection(&path, "node-one").unwrap();
        let (state, warning) = load_state(&path);

        assert!(warning.is_none());
        assert!(state.last_connected["node-one"] > 0);
    }

    fn device(id: &str, hostname: &str) -> Device {
        Device {
            id: id.to_owned(),
            machine_name: hostname.to_owned(),
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
