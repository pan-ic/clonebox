use anyhow::Context;
use std::{
    fs::{
        create_dir_all,
        read_to_string,
        write,
        File,
    },
    os::fd::OwnedFd,
};
use nix::unistd::Pid;

pub(crate) fn get_root_cgroup_path() -> &'static str {
    "/sys/fs/cgroup"
}

pub(crate) fn get_app_cgroup_path() -> &'static str {
    "/sys/fs/cgroup/clonebox"
}

pub(crate) fn get_child_cgroup_path(name: &str) -> String {
    format!("/sys/fs/cgroup/clonebox/{}", name)
}

fn enable_controller(path: &str, resource: &str) -> anyhow::Result<()> {
    let controllers_path = format!("{}{}", path, "/cgroup.controllers");
    let available = read_to_string(controllers_path).with_context(|| format!("failed to read cgroup.controllers: {}", &resource))?;
    if !available.contains(resource) {
        anyhow::bail!("controller {} not available on this system", resource);
    }

    let subtree_path = format!("{}{}", path, "/cgroup.subtree_control");
    let enabled = read_to_string(&subtree_path).with_context(|| format!("failed to read cgroup.subtree: {}: ", &resource))?;
    if !enabled.contains(resource) {
        write(subtree_path, format!("+{}", resource))?;
    }
    Ok(())
}

pub(crate) fn set_cgroup(instance: &str, resource: &str, key: &str, value: &str) -> anyhow::Result<()>{ 
    let cgroup_file = format!("{}/{}.{}", instance, resource, key);

    write(cgroup_file, value).with_context(|| format!("failed to change {}.{} cgroup value:{} for {}", resource, key, value, instance))?;
    
    Ok(())
}

pub(crate) fn init_resources(instance: &str, resources: &Vec<&str>) -> anyhow::Result<()>{
    for resource in resources {
        enable_controller(&instance, resource).with_context(|| { format!("failed to enable {} {} cgroup", instance, resource)})?;
    }

    Ok(())
}

pub(crate) fn create_cgroup(name: &str) -> anyhow::Result<(String, OwnedFd)> {
    //let root_cgroup = get_root_cgroup_path();
    let app_cgroup_path = get_app_cgroup_path();
    let child_cgroup = get_child_cgroup_path(&name);

    //enable_controller(root_cgroup, "memory").context("failed to enable root memory cgroup")?;
    //enable_controller(root_cgroup, "cpu").context("failed to enable root cpu cgroup")?;

    create_dir_all(app_cgroup_path).context("failed to create clonebox cgroup dir")?;
    //enable_controller(&app_cgroup_path, "memory").context("failed to enable app memory cgroup")?;
    //enable_controller(&app_cgroup_path, "cpu").context("failed to enable app cpu cgroup")?;

    create_dir_all(&child_cgroup).context("failed to create child cgroup dir")?;
    //enable_controller(&child_cgroup, "memory").context("failed to enable container memory cgroup")?;
    //enable_controller(&child_cgroup, "cpu").context("failed to enable container cpu cgroup")?;

    let fd = File::open(&child_cgroup)?;
    
    Ok((child_cgroup, OwnedFd::from(fd)))
}
