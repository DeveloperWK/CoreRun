use crate::{
    cgroup::{CgroupConfig, CgroupManager},
    cli::{ContainerConfig, parse_args},
    error::{ContainerError, ContainerResult},
    filesystem::FilesystemManager,
    namespace::{NamespaceConfig, NamespaceManager},
    network,
    process::ProcessManager,
    setup::{cleanup_container_network, setup_container_network_parent},
    volume::ImplVolume,
};
use {
    log::{debug, error, info},
    nix::unistd::{ForkResult, Uid, close, fork, getpid, pipe, read, write},
    std::{
        os::fd::{IntoRawFd, RawFd},
        path::Path,
    },
};

pub fn run() -> ContainerResult<()> {
    let config = parse_args();
    info!("Starting container runtime (PID: {})", getpid());
    debug!("Configuration: {config:?}");
    if !Uid::current().is_root() {
        error!("Root privileges required for container operations");
        return Err(ContainerError::RootRequired);
    }
    let container_id = format!(
        "container-{}-{}",
        getpid(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    log::info!("Container ID: {}", container_id);
    let isolate_net = !matches!(config.network_mode, network::NetworkMode::Host);

    let ns_config = NamespaceConfig {
        isolate_pid: true,
        isolate_net,
        isolate_mount: true,
        isolate_uts: true,
        isolate_ipc: true,
        isolate_user: false,
    };
    if isolate_net {
        let (read_fd, write_fd) = pipe().expect("Failed to create pipe");
        let read_raw = read_fd.into_raw_fd();
        let write_raw = write_fd.into_raw_fd();
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                if let Err(e) = close(read_raw) {
                    error!("Failed to close read fd in parent: {}", e);
                }
                info!("Parent: Forked child process with PID {}", child);
                std::thread::sleep(std::time::Duration::from_millis(300));
                if let Err(e) =
                    setup_container_network_parent(&container_id, child.as_raw(), &config)
                {
                    error!("Failed to setup network: {}", e);

                    close(write_raw).ok();
                    let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
                    return Err(e);
                }
                info!("Network setup complete, signaling child to continue");
                let borrowed_write_fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(write_raw) };
                if let Err(e) = write(borrowed_write_fd, b"1") {
                    error!("Failed to write sync signal: {}", e);
                }

                if let Err(e) = close(write_raw) {
                    error!("Failed to close write fd in parent: {}", e);
                }

                match nix::sys::wait::waitpid(child, None) {
                    Ok(status) => {
                        info!("Container exited with status: {:?}", status);
                        if let Err(e) = cleanup_container_network(&container_id) {
                            error!("Failed to cleanup network: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to wait for child: {}", e);
                        let _ = cleanup_container_network(&container_id);
                        return Err(ContainerError::ProcessExecution {
                            message: format!("Wait failed: {}", e),
                        });
                    }
                }
            }
            Ok(ForkResult::Child) => {
                if let Err(e) = close(write_raw) {
                    error!("Failed to close write fd in child: {}", e);
                    std::process::exit(1);
                }

                if let Err(e) = run_container_with_sync(config, ns_config, container_id, read_raw) {
                    error!("Container error: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            Err(e) => {
                error!("Fork failed: {}", e);
                return Err(ContainerError::NamespaceSetup {
                    message: format!("Fork failed: {}", e),
                });
            }
        }
    } else {
        return run_container(config, ns_config, container_id);
    }

    Ok(())
}

fn run_container(
    config: ContainerConfig,
    ns_config: NamespaceConfig,
    container_id: String,
) -> ContainerResult<()> {
    let _cgroup_manager = if config.memory_limit_mb.is_some()
        || config.cpu_percent.is_some()
        || config.pids_limit.is_some()
    {
        let mut cgroup_config = CgroupConfig::new(container_id.clone());
        if let Some(mem) = config.memory_limit_mb {
            cgroup_config = cgroup_config.with_memory_mb(mem);
            info!("Setting memory limit: {} MB", mem);
        }
        if let Some(cpu) = config.cpu_percent {
            cgroup_config = cgroup_config.with_cpu_percent(cpu);
            log::info!("Setting CPU limit: {}%", cpu)
        }
        if let Some(pids) = config.pids_limit {
            cgroup_config = cgroup_config.with_pids_limit(pids);
            log::info!("Setting PIDs limit: {}", pids)
        }
        let manager = CgroupManager::new(cgroup_config)?;
        manager.setup()?;
        manager.add_process(getpid().as_raw())?;
        Some(manager)
    } else {
        info!("No resource limits specified, skipping cgroup setup");
        None
    };
    let rootfs_path = Path::new(&config.rootfs);
    let volume_manager = if !config.volumes.is_empty() {
        log::info!("Setting up {} volume(s)", config.volumes.len());
        for vol in &config.volumes {
            log::info!(" - {}", vol)
        }
        Some(ImplVolume::setup_volumes(config.volumes, rootfs_path)?)
    } else {
        log::info!("No volumes specified");
        None
    };
    NamespaceManager::unshare_namespaces(ns_config)?;
    NamespaceManager::enter_pid_namespace()?;
    info!("Running as PID 1 in container (host PID: {})", getpid());
    let hostname = config.hostname.as_deref().unwrap_or("rust-container");
    NamespaceManager::set_hostname(&hostname)?;
    let rootfs_path = std::path::Path::new(&config.rootfs);
    FilesystemManager::setup_container_filesystem(&rootfs_path)?;
    info!("Container environment setup complete, executing command...");

    ProcessManager::execute_container_command(&config.command, &config.args)?;
    if let Some(vol_mgr) = volume_manager {
        info!("Cleaning up volumes...");
        if let Err(e) = vol_mgr.cleanup_volume(rootfs_path) {
            error!("Failed to cleanup volumes: {}", e);
        }
    }
    Ok(())
}

fn run_container_with_sync(
    config: ContainerConfig,
    ns_config: NamespaceConfig,
    container_id: String,
    sync_fd: RawFd,
) -> ContainerResult<()> {
    // Setup cgroups
    let _cgroup_manager = if config.memory_limit_mb.is_some()
        || config.cpu_percent.is_some()
        || config.pids_limit.is_some()
    {
        let mut cgroup_config = CgroupConfig::new(container_id.clone());

        if let Some(mem) = config.memory_limit_mb {
            cgroup_config = cgroup_config.with_memory_mb(mem);
            info!("Setting memory limit: {} MB", mem);
        }
        if let Some(cpu) = config.cpu_percent {
            cgroup_config = cgroup_config.with_cpu_percent(cpu);
            log::info!("Setting CPU limit: {}%", cpu)
        }
        if let Some(pids) = config.pids_limit {
            cgroup_config = cgroup_config.with_pids_limit(pids);
            log::info!("Setting PIDs limit: {}", pids)
        }

        let manager = CgroupManager::new(cgroup_config)?;
        manager.setup()?;
        manager.add_process(getpid().as_raw())?;
        Some(manager)
    } else {
        info!("No resource limits specified, skipping cgroup setup");
        None
    };

    let rootfs_path = Path::new(&config.rootfs);
    let volume_manager = if !config.volumes.is_empty() {
        log::info!("Setting up {} volume(s)", config.volumes.len());
        for vol in &config.volumes {
            log::info!(" - {}", vol)
        }
        Some(ImplVolume::setup_volumes(
            config.volumes.clone(),
            rootfs_path,
        )?)
    } else {
        log::info!("No volumes specified");
        None
    };

    NamespaceManager::unshare_namespaces(ns_config)?;

    // Wait for parent to setup network
    info!("Waiting for parent to setup network...");
    let mut buf = [0u8; 1];
    let borrowed_fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(sync_fd) };
    match read(borrowed_fd, &mut buf) {
        Ok(n) if n > 0 => {
            info!("Network setup signal received from parent");
        }
        Ok(_) => {
            error!("Read 0 bytes from sync pipe");
        }
        Err(e) => {
            error!("Failed to read from sync pipe: {}", e);
        }
    }

    if let Err(e) = close(sync_fd) {
        error!("Failed to close sync fd: {}", e);
    }
    NamespaceManager::enter_pid_namespace()?;
    info!("Running as PID 1 in container (host PID: {})", getpid());

    let hostname = config.hostname.as_deref().unwrap_or("rust-container");
    NamespaceManager::set_hostname(&hostname)?;
    let rootfs_path = std::path::Path::new(&config.rootfs);
    FilesystemManager::setup_container_filesystem(&rootfs_path)?;
    info!("Container environment setup complete, executing command...");

    ProcessManager::execute_container_command(&config.command, &config.args)?;

    if let Some(vol_mgr) = volume_manager {
        info!("Cleaning up volumes...");
        if let Err(e) = vol_mgr.cleanup_volume(rootfs_path) {
            error!("Failed to cleanup volumes: {}", e);
        }
    }

    Ok(())
}
