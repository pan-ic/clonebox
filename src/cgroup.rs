use anyhow::Context;
use std::fs::{
    create_dir_all,
    read_to_string,
    write,
};

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

pub(crate) fn create_cgroups(name: &str) -> anyhow::Result<String> {
    let root_cgroups = "/sys/fs/cgroup";
    enable_controller(root_cgroups, "memory")?;
    enable_controller(root_cgroups, "cpu")?;

    let app_cgroups_path = "/sys/fs/cgroup/clonebox";
    let child_cgroups = format!("/sys/fs/cgroup/clonebox/{}", name);
    let child_mem_max = format!("{}/memory.max", child_cgroups);
    let child_cpu_max = format!("{}/cpu.max", child_cgroups);
    let _child_procs = format!("{}/cgroup.procs", child_cgroups);

    let _ = create_dir_all(app_cgroups_path).context("failed to create clonebox cgroup dir")?;
    enable_controller(&app_cgroups_path, "memory")?;
    enable_controller(&app_cgroups_path, "cpu")?;

    let _ = create_dir_all(&child_cgroups).context("failed to create child cgroup dir")?;
    let _ = enable_controller(&child_cgroups, "memory").context("failed to enable memory cgroup")?;
    let _ = write(child_mem_max, "256M").context("failed to change memory cgroup resource")?;
    let _ = enable_controller(&child_cgroups, "cpu").context("failed to enable cpu cgroup")?;
    let _ = write(child_cpu_max, "100000 100000").context("failed to change cpu cgroup resource")?;
    
    //Here comes the troubles, writing this way inside the child cgroup.procs is not possible
    //because system.d own the child process so the resource is busy
    //let _ = write(child_procs, child_pid.as_raw().to_string()).context("failed to associate cgroups to child")?;

    Ok(child_cgroups)
}
