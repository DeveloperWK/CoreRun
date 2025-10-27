use std::net::Ipv4Addr;

use crate::error::ContainerResult;

#[derive(Debug, Clone)]
pub enum NetworkMode {
    Bridge,
    Host,
    None,
    Container(String),
    Custom(String),
}
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub mode: NetworkMode,
    pub ports: Vec<PortMapping>,
    pub dns_server: Vec<Ipv4Addr>,
    pub hostname: Option<String>,
}
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Protocol,
}
#[derive(Debug, Clone)]
pub enum Protocol {
    UDP,
    TCP,
}
impl NetworkConfig {
    pub fn parse(network_str: &str) -> ContainerResult<Self> {
        match network_str {
            "bridge" => Ok(NetworkConfig {
                mode: NetworkMode::Bridge,
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
            "host" => Ok(NetworkConfig {
                mode: NetworkMode::Host,
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
            "none" => Ok(NetworkConfig {
                mode: NetworkMode::None,
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
            network if network.starts_with("container:") => {
                let contsiner_id = network.trim_start_matches("container:").to_string();
                Ok(NetworkConfig {
                    mode: NetworkMode::Container(contsiner_id),
                    ports: Vec::new(),
                    dns_server: Vec::new(),
                    hostname: None,
                })
            }
            custom => Ok(NetworkConfig {
                mode: NetworkMode::Custom(custom.to_string()),
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
        }
    }
}
