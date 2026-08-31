use std::{
    fs,
    fs::{File, create_dir_all},
    os::fd::OwnedFd,
};

use crate::error::{CgroupError, CoreError};

pub(crate) fn get_root_cgroup_path() -> &'static str {
    "/sys/fs/cgroup"
}

pub(crate) fn get_app_cgroup_path() -> &'static str {
    "/sys/fs/cgroup/clonebox"
}

pub(crate) fn get_child_cgroup_path(name: &str) -> String {
    format!("/sys/fs/cgroup/clonebox/{}", name)
}

fn enable_controller(path: &str, resource: &str) -> Result<(), CoreError> {
    let controllers_path = format!("{}{}", path, "/cgroup.controllers");
    let available = fs::read_to_string(&controllers_path).map_err(|e| {
        CgroupError::ReadToStringFailure(e, controllers_path.clone(), resource.to_string())
    })?;
    if !available.contains(resource) {
        Err(CgroupError::ResourceNotAvailable(
            controllers_path,
            resource.to_string(),
        ))?;
    }

    let subtree_path = format!("{}{}", path, "/cgroup.subtree_control");
    let enabled = fs::read_to_string(&subtree_path).map_err(|e| {
        CgroupError::ReadToStringFailure(e, subtree_path.clone(), resource.to_string())
    })?;
    if !enabled.contains(resource) {
        fs::write(subtree_path, format!("+{}", resource))
            .map_err(|e| CgroupError::WriteFailure(e, resource.to_string()))?;
    }
    Ok(())
}

pub(crate) fn set_cgroup(
    instance: &str,
    resource: &str,
    key: &str,
    value: &str,
) -> Result<(), CoreError> {
    let cgroup_file = format!("{}/{}.{}", instance, resource, key);

    fs::write(cgroup_file, value).map_err(|e| {
        let msg = format!(
            "failed to change {}.{} cgroup value:{} for {}",
            resource, key, value, instance
        );
        CgroupError::WriteFailure(e, msg)
    })?;

    Ok(())
}

pub(crate) fn init_resources(instance: &str, resources: &Vec<&str>) -> Result<(), CoreError> {
    for resource in resources {
        enable_controller(instance, resource)?;
    }

    Ok(())
}

pub(crate) fn create_cgroup(name: &str) -> Result<(String, OwnedFd), CoreError> {
    let app_cgroup_path = get_app_cgroup_path();
    let child_cgroup = get_child_cgroup_path(name);

    create_dir_all(app_cgroup_path)
        .map_err(|e| CgroupError::CreateCgroupDirFailure(e, "clonebox app".to_string()))?;
    create_dir_all(&child_cgroup)
        .map_err(|e| CgroupError::CreateCgroupDirFailure(e, "child".to_string()))?;
    let fd =
        File::open(&child_cgroup).map_err(|e| CgroupError::OpenFailure(e, "child".to_string()))?;

    Ok((child_cgroup, OwnedFd::from(fd)))
}
