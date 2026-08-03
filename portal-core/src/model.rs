use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Device {
    pub id: String,
    pub hostname: String,
    pub dns_name: Option<String>,
    pub nickname: Option<String>,
    pub ips: Vec<IpAddr>,
    pub preferred_ip: IpAddr,
    pub os: String,
    pub online: bool,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub notes: Option<String>,
    pub last_connected_at: Option<u64>,
}

impl Device {
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.hostname)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Connect,
    Transfer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}
