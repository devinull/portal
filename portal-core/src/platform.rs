use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
#[error(
    "{program} executable was not found; searched: {searched}",
    searched = format_paths(.searched)
)]
pub struct ExecutableNotFound {
    pub program: String,
    pub searched: Vec<PathBuf>,
}

pub(crate) fn resolve_executable(
    program: &str,
    override_path: Option<&Path>,
) -> Result<PathBuf, ExecutableNotFound> {
    resolve_executable_with(
        program,
        override_path,
        std::env::var_os("PATH").as_deref(),
        &platform_candidates(program),
    )
}

fn resolve_executable_with(
    program: &str,
    override_path: Option<&Path>,
    path_value: Option<&OsStr>,
    candidates: &[PathBuf],
) -> Result<PathBuf, ExecutableNotFound> {
    if let Some(path) = override_path {
        return path
            .is_file()
            .then(|| path.to_owned())
            .ok_or_else(|| ExecutableNotFound {
                program: program.to_owned(),
                searched: vec![path.to_owned()],
            });
    }

    let names = executable_names(program);
    let mut searched = Vec::new();
    if let Some(path_value) = path_value {
        for directory in std::env::split_paths(path_value) {
            for name in &names {
                let candidate = directory.join(name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
                searched.push(candidate);
            }
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
        if !searched.contains(candidate) {
            searched.push(candidate.clone());
        }
    }

    Err(ExecutableNotFound {
        program: program.to_owned(),
        searched,
    })
}

#[must_use]
pub fn platform_candidates(program: &str) -> Vec<PathBuf> {
    match program {
        "tailscale" => tailscale_candidates(),
        "rustdesk" => rustdesk_candidates(),
        _ => Vec::new(),
    }
}

fn tailscale_candidates() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from(r"C:\Program Files\Tailscale\tailscale.exe")]
    } else {
        vec![
            PathBuf::from("/usr/bin/tailscale"),
            PathBuf::from("/usr/local/bin/tailscale"),
        ]
    }
}

fn rustdesk_candidates() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from(r"C:\Program Files\RustDesk\rustdesk.exe")]
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/RustDesk.app/Contents/MacOS/RustDesk"),
            PathBuf::from("/usr/local/bin/rustdesk"),
            PathBuf::from("/usr/bin/rustdesk"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/bin/rustdesk"),
            PathBuf::from("/usr/local/bin/rustdesk"),
        ]
    }
}

fn executable_names(program: &str) -> Vec<OsString> {
    let name = OsString::from(program);
    if cfg!(windows) && Path::new(program).extension().is_none() {
        vec![OsString::from(format!("{program}.exe")), name]
    } else {
        vec![name]
    }
}

fn format_paths(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return "(no PATH entries or platform candidates)".to_owned();
    }
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use tempfile::tempdir;

    use super::{executable_names, resolve_executable_with};

    #[test]
    fn override_wins_over_path_and_candidates() {
        let temp = tempdir().unwrap();
        let override_path = temp.path().join("override");
        let path_dir = temp.path().join("path");
        fs::create_dir(&path_dir).unwrap();
        fs::write(&override_path, "").unwrap();
        fs::write(path_dir.join("tool"), "").unwrap();

        let found = resolve_executable_with(
            "tool",
            Some(&override_path),
            Some(path_dir.as_os_str()),
            &[],
        )
        .unwrap();

        assert_eq!(found, override_path);
    }

    #[test]
    fn path_wins_over_platform_candidate() {
        let temp = tempdir().unwrap();
        let path_dir = temp.path().join("path");
        let fallback = temp.path().join("fallback");
        fs::create_dir(&path_dir).unwrap();
        let path_tool = path_dir.join(if cfg!(windows) { "tool.exe" } else { "tool" });
        fs::write(&path_tool, "").unwrap();
        fs::write(&fallback, "").unwrap();

        let found =
            resolve_executable_with("tool", None, Some(path_dir.as_os_str()), &[fallback]).unwrap();

        assert_eq!(found, path_tool);
    }

    #[test]
    fn platform_candidate_is_used_when_path_has_no_match() {
        let temp = tempdir().unwrap();
        let empty_path = temp.path().join("empty");
        let fallback = temp.path().join("folder with spaces").join("tool");
        fs::create_dir_all(fallback.parent().unwrap()).unwrap();
        fs::create_dir(&empty_path).unwrap();
        fs::write(&fallback, "").unwrap();

        let found = resolve_executable_with(
            "tool",
            None,
            Some(empty_path.as_os_str()),
            std::slice::from_ref(&fallback),
        )
        .unwrap();

        assert_eq!(found, fallback);
    }

    #[test]
    fn missing_executable_reports_searched_locations() {
        let temp = tempdir().unwrap();
        let fallback = temp.path().join("missing fallback");

        let error = resolve_executable_with("tool", None, None, std::slice::from_ref(&fallback))
            .unwrap_err();

        assert_eq!(error.program, "tool");
        assert_eq!(error.searched, vec![fallback]);
    }

    #[test]
    fn windows_search_includes_exe_extension() {
        let names = executable_names("tool");

        if cfg!(windows) {
            assert_eq!(names[0], OsString::from("tool.exe"));
        } else {
            assert_eq!(names, vec![OsString::from("tool")]);
        }
    }
}
