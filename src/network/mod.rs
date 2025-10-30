pub mod bridge;
pub mod iptables;
pub mod net_manager;

pub mod network_namespace;
pub mod veth;
use std::net::Ipv4Addr;

pub use net_manager::*;

pub use network_namespace::*;
#[derive(Debug, Clone)]
pub enum NetworkMode {
    Bridge { network_name: String },
    Host,
    None,
    Container { container_id: String },
}
// #[derive(Debug, Clone)]
// pub struct NetworkConfig {
//     pub mode: NetworkMode,
//     pub ports: Vec<PortMapping>,
//     pub dns_server: Vec<Ipv4Addr>,
//     pub hostname: Option<String>,
// }
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Protocol,
}
#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    UDP,
    TCP,
}
#[derive(Debug, Clone)]
pub struct ContainerNetwork {
    pub mode: NetworkMode,
    pub ip_address: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub veth_host: Option<String>,
    pub veth_container: Option<String>,
    pub ports: Vec<PortMapping>,
}
