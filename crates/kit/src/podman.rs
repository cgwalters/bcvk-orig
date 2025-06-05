use bootc_utils::CommandRunExt;
use color_eyre::{eyre::eyre, Result};
use serde::Deserialize;

use crate::hostexec;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    #[allow(dead_code)]
    pub graph_driver_name: String,
    pub graph_root: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodmanSystemInfo {
    pub store: Store,
}

pub fn get_system_info() -> Result<PodmanSystemInfo> {
    hostexec::podman()?
        .arg("system")
        .arg("info")
        .arg("--format=json")
        .run_and_parse_json()
        .map_err(|e| eyre!("podman system info failed: {}", e))
}
