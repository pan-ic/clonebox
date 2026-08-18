use anyhow::Context;
use serde::Deserialize;
use std::fs::read_to_string;

#[derive(Deserialize, Clone)]
pub(crate) struct Config {
    #[serde(rename = "ociVersion")]
    oci_version: String,
    process: Option<Process>,
    root: Option<Root>,
    mounts: Option<Vec<Mount>>,
    hostname: Option<String>,
}

#[allow(unused)]
#[derive(Deserialize, Clone)]
pub(crate) struct Root {
    path: String,
    readonly: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Process {
    env: Option<Vec<String>>,
    args: Option<Vec<String>>,
    cwd: String,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Mount {
    pub(crate) destination: String,
    #[serde(rename = "type")]
    pub(crate) mount_type: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) options: Option<Vec<String>>,
}

impl Config {
    pub(crate) fn load(config_path: &str) -> anyhow::Result<Self> {
        let config_file = format!("{}/config.json", config_path);
        let raw_conf = read_to_string(&config_file)
            .with_context(|| format!("failed to read {}", config_file))?;
        let config: Config =
            serde_json::from_str(&raw_conf).context("failed to desirialize config")?;

        Ok(config)
    }

    pub(crate) fn get_oci_version(&self) -> &str {
        &self.oci_version
    }

    pub(crate) fn get_process_env(&self) -> Option<Vec<String>> {
        self.process.as_ref()?.env.clone()
    }

    pub(crate) fn get_process_args(&self) -> Option<Vec<String>> {
        self.process.as_ref()?.args.clone()
    }

    pub(crate) fn get_process_cwd(&self) -> &str {
        self.process.as_ref().map(|p| p.cwd.as_str()).unwrap_or("/")
    }

    pub(crate) fn get_root_path(&self) -> &str {
        self.root.as_ref().map(|r| r.path.as_str()).unwrap_or("")
    }

    #[allow(unused)]
    pub(crate) fn get_root_readonly(&self) -> Option<bool> {
        self.root.as_ref()?.readonly
    }

    pub(crate) fn get_hostname(&self) -> Option<String> {
        self.hostname.clone()
    }

    pub(crate) fn get_mounts(&self) -> Option<&Vec<Mount>> {
        self.mounts.as_ref()
    }
}
