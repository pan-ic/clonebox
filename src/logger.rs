use anyhow::Context;
use std::fs::{File, OpenOptions};
use std::io::Write;

pub(crate) fn open_log_file(bundle_path: &str) -> anyhow::Result<File> {
    let log_file_path = format!("{}/container.log", bundle_path);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .context("failed to open log file")
}

pub(crate) fn write_log_file(fd: &mut File, message: &str) -> anyhow::Result<()> {
    fd.write_all(message.as_bytes())
        .context("failed to write to log file")?;

    Ok(())
}
