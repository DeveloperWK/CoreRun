use clap::{Arg, ArgAction, Command};

use crate::network::{NetworkMode, PortMapping};

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
}

pub fn parse_args() -> ContainerConfig {
    let matches = Command::new("container-runtime")
        .version("0.1.0")
        .about("A simple container runtime in Rust")
        .arg(
            Arg::new("rootfs")
                .long("rootfs")
                .value_name("PATH")
                .required(true)
                .help("Path to root filesystem")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("memory")
                .long("memory")
                .short('m')
                .value_name("MB")
                .help("Memory limit in megabytes (e.g., 512)")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("cpu")
                .long("cpu")
                .short('c')
                .value_name("PERCENT")
                .help("CPU limit as percentage of one core (e.g., 50 = 50%)")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("pids")
                .long("pids")
                .short('p')
                .value_name("COUNT")
                .help("Maximum number of processes/threads")
                .value_parser(clap::value_parser!(i64)),
        )
        .arg(
            Arg::new("hostname")
                .long("hostname")
                .value_name("HOSTNAME")
                .help("container hostname")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("command")
                .help("Command to execute inside container")
                .required(true)
                .index(1)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("args")
                .help("Arguments for the command")
                .num_args(0..)
                .index(2)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("volume")
                .long("volume")
                .short('v')
                .help("Bind mount volume (can be used multiple times). Format: /host/path:/container/path[:ro|rw]")
                .value_name("VOLUME")
                .action(clap::ArgAction::Append)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
        	Arg::new("network")
        		.long("network")
         		.short('n')
         		.value_name("MODE")
         		.help("Network mode: bridge (default), host, none, container:<id>")
         		.default_value("bridge")
         		.value_parser(clap::value_parser!(String))
        )
        .arg(
        	Arg::new("port")
        		.long("port")
        		.short('P')
        		.help("Publish port(s). Format: HOST:CONTAINER[/PROTOCOL] (e.g., 8080:80/tcp)")
        		.value_name("PORT")
        		.action(ArgAction::Append)
        		.value_parser(clap::value_parser!(String))
        )
        .arg(
        	Arg::new("net")
        		.long("net")
        		.short('N')
        		.help("Network name for bridge mode (e.g., --net or -N mynet)")
        		.value_name("NETWORK")
        		.value_parser(clap::value_parser!(String))
        )
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
    }
}
