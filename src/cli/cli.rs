use crate::network::{NetworkMode, PortMapping};
use clap::{Arg, ArgAction, Command};

#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub rootfs: String,
    pub command: String,
    pub args: Vec<String>,
    pub hostname: Option<String>,
    pub memory_limit_mb: Option<u64>,
    pub pids_limit: Option<i64>,
    pub cpu_percent: Option<u64>,
    pub volumes: Vec<String>,
    pub network_mode: NetworkMode,
    pub ports: Vec<PortMapping>,
    pub logs: Option<bool>,
}

pub fn parse_args() -> ContainerConfig {
    let matches = Command::new("corerun")
        .version(env!("CARGO_PKG_VERSION"))
        .about(
            "⚙️  CoreRun — A lightweight container runtime written in Rust.\n\
                Run isolated containers with custom rootfs, resource limits, and network modes.",
        )
        .next_line_help(true)
        .help_template(
            "\
{name} {version}
{about}

{usage-heading} {usage}

{all-args}
",
        )
        // --- Core options ---
        .arg(
            Arg::new("rootfs")
                .long("rootfs")
                .value_name("PATH")
                .required(true)
                .help(
                    "🔹 Path to the container root filesystem (required).\n\
                       Example: --rootfs ./ubuntu-rootfs",
                )
                .help_heading("CORE OPTIONS")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("log")
                .long("log")
                .short('l')
                .value_name("LOGS")
                .help(
                    "Enable or disable logging output.\n\
             Example: --log true  (enable info logs)\n\
                      --log false (disable logs, minimal output)",
                )
                .help_heading("MISC OPTIONS")
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("command")
                .help(
                    "🔹 Command to execute inside the container.\n\
                       Example: /bin/bash or /usr/bin/python3",
                )
                .required(true)
                .index(1)
                .help_heading("CORE OPTIONS")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("args")
                .help(
                    "🔹 Arguments passed to the main command.\n\
                       Example: corerun --rootfs ./rootfs /bin/bash -c 'echo hello'",
                )
                .num_args(0..)
                .index(2)
                .help_heading("CORE OPTIONS")
                .value_parser(clap::value_parser!(String)),
        )
        // --- Resource Limits ---
        .arg(
            Arg::new("memory")
                .long("memory")
                .short('m')
                .value_name("MB")
                .help(
                    "💾 Memory limit in megabytes.\n\
                       Example: --memory 512 for 512MB RAM.",
                )
                .help_heading("RESOURCE LIMITS")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("cpu")
                .long("cpu")
                .short('c')
                .value_name("PERCENT")
                .help(
                    "⚙️  CPU limit as percentage of one core.\n\
                       Example: --cpu 50 = 50% of one core.",
                )
                .help_heading("RESOURCE LIMITS")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("pids")
                .long("pids")
                .short('p')
                .value_name("COUNT")
                .help("🧵 Maximum number of processes/threads allowed.")
                .help_heading("RESOURCE LIMITS")
                .value_parser(clap::value_parser!(i64)),
        )
        // --- Networking ---
        .arg(
            Arg::new("network")
                .long("network")
                .short('n')
                .value_name("MODE")
                .help(
                    "🌐 Network mode options:\n\
    - bridge: Containers communicate via isolated network (default)\n\
    - host:   Share host network stack for direct access\n\
    - none:   Disable all networking (full isolation)\n\
    - ports:  Enable port forwarding to expose container services\n\
    - multi:  Connect multiple containers to the same virtual network",
                )
                .default_value("bridge")
                .help_heading("NETWORK OPTIONS")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('P')
                .help(
                    "🔌 Publish port(s) to the host. Can be used multiple times.\n\
                       Format: HOST:CONTAINER[/PROTOCOL]\n\
                       Example: -P 8080:80/tcp",
                )
                .value_name("PORT")
                .help_heading("NETWORK OPTIONS")
                .action(ArgAction::Append)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("net")
                .long("net")
                .short('N')
                .help(
                    "🌉 Custom bridge network name.\n\
                       Example: --net my_bridge_network",
                )
                .value_name("NETWORK")
                .help_heading("NETWORK OPTIONS")
                .value_parser(clap::value_parser!(String)),
        )
        // --- Misc ---
        .arg(
            Arg::new("hostname")
                .long("hostname")
                .value_name("HOSTNAME")
                .help(
                    "🏷️  Set custom hostname inside the container.\n\
                       Example: --hostname mycontainer",
                )
                .help_heading("MISC OPTIONS")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("volume")
                .long("volume")
                .short('v')
                .help(
                    "💽 Bind mount host directory into the container.\n\
                       Can be used multiple times.\n\
                       Format: /host/path:/container/path[:ro|rw]\n\
                       Example: -v /data:/app/data:ro",
                )
                .value_name("VOLUME")
                .help_heading("MISC OPTIONS")
                .action(ArgAction::Append)
                .value_parser(clap::value_parser!(String)),
        )
        // --- Footer examples ---
        .after_help(
            "\
📘 Examples:
  ▶ Basic usage:
    corerun --rootfs ./ubuntu-rootfs /bin/bash

  ▶ Limit memory and CPU:
    corerun --rootfs ./ubuntu-rootfs -m 512 -c 50 /bin/sh

  ▶ Mount a volume and set hostname:
    corerun --rootfs ./rootfs -v /data:/app/data --hostname myapp /bin/bash

  ▶ Run with custom network:
    corerun --rootfs ./rootfs --network host /usr/bin/python3 app.py
",
        )
        .color(clap::ColorChoice::Always)
        .get_matches();

    let rootfs = matches
        .get_one::<String>("rootfs")
        .expect("rootfs is required")
        .clone();
    let command = matches
        .get_one::<String>("command")
        .expect("command is required")
        .clone();
    let args: Vec<String> = matches
        .get_many::<String>("args")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    let hostname = matches.get_one::<String>("hostname").cloned();
    let memory_limit_mb = matches.get_one::<u64>("memory").copied();
    let cpu_percent = matches.get_one::<u64>("cpu").copied();
    let pids_limit = matches.get_one::<i64>("pids").copied();
    let logs = matches.get_one::<bool>("log").cloned();
    let volumes = matches
        .get_many::<String>("volume")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    let network_str = matches
        .get_one::<String>("network")
        .map(|s| s.as_str())
        .unwrap_or("bridge");
    let network_mode = if network_str.starts_with("container:") {
        let container_id = network_str.strip_prefix("container:").unwrap().to_string();
        NetworkMode::Container { container_id }
    } else {
        match network_str {
            "bridge" => NetworkMode::Bridge {
                network_name: "bridge".to_string(),
            },
            "host" => NetworkMode::Host,
            "none" => NetworkMode::None,
            _ => {
                log::error!("Invalid network mode: {}, using bridge", network_str);
                NetworkMode::Bridge {
                    network_name: "bridge".to_string(),
                }
            }
        }
    };
    let ports: Vec<PortMapping> = matches
        .get_many::<String>("port")
        .map(|v| {
            v.filter_map(|s| match PortMapping::parse(s) {
                Ok(pm) => Some(pm),
                Err(e) => {
                    log::warn!("Warning: Invalid port mapping '{}': {}", s, e);
                    None
                }
            })
            .collect()
        })
        .unwrap_or_default();

    ContainerConfig {
        rootfs,
        command,
        args,
        hostname,
        memory_limit_mb,
        cpu_percent,
        pids_limit,
        volumes,
        network_mode,
        ports,
        logs,
    }
}
