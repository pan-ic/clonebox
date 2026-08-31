use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::error::{CoreError, LogError};

pub(crate) fn open_log_file(bundle_path: &str) -> Result<File, CoreError> {
    let log_file_path = format!("{}/container.log", bundle_path);

    Ok(OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(LogError::Io)?)
}

pub(crate) fn write_log_file(fd: &mut File, message: &str) -> Result<(), CoreError> {
    fd.write_all(message.as_bytes()).map_err(LogError::Io)?;

    Ok(())
}
