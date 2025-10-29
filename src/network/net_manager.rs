use crate::{
    error::{ContainerError, ContainerResult, Context},
    network::{ContainerNetwork, NetworkMode, PortMapping, bridge::Bridge, iptables},
};

use std::{
    cmp,
    collections::{self, HashMap, HashSet},
    fmt::format,
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};

pub struct NetworkManager {
    networks: Arc<Mutex<HashMap<String, NetworkConfig>>>,
    container_networks: Arc<Mutex<HashMap<String, ContainerNetwork>>>,
}

#[derive(Clone)]
struct NetworkConfig {
    name: String,
    bridge: Bridge,
    subnet: ipnetwork::Ipv4Network,
    gateway: Ipv4Addr,
    allocator: IpAllocator,
}
impl NetworkManager {
    pub fn new() -> ContainerResult<Self> {
        let manager = Self {
            networks: Arc::new(Mutex::new(HashMap::new())),
            container_networks: Arc::new(Mutex::new(HashMap::new())),
        };
        manager.create_network("bridge", "172.17.0.0/16")?;
        Ok(manager)
    }
    pub fn create_network(&self, name: &str, subnet: &str) -> ContainerResult<()> {
        let subnet: ipnetwork::Ipv4Network =
            subnet.parse().map_err(|_| ContainerError::Network {
                message: format!("Invalid subnet"),
            })?;
        let bridge_name = if name == "bridge" {
            "corerun0".to_string()
        } else {
            format!("br-{}", &name[..std::cmp::min(8, name.len())])
        };
        let bridge = Bridge::new(&bridge_name)?;
        bridge.create()?;
        let gateway = subnet.iter().nth(1).unwrap();
        bridge.set_ip(gateway, subnet.prefix())?;
        bridge.up()?;
        iptables::setup_nat(&bridge_name, &subnet.to_string());
        let config = NetworkConfig {
            name: name.to_string(),
            bridge,
            subnet,
            gateway,
            allocator: IpAllocator::new(subnet)?,
        };
        self.networks
            .lock()
            .unwrap()
            .insert(name.to_string(), config);

        log::info!("Created network '{}' with subnet {}", name, subnet);
        Ok(())
    }
    pub fn setup_container_network(
        &self,
        container_id: &str,
        pid: i32,
        mode: NetworkMode,
        ports: Vec<PortMapping>,
    ) -> ContainerResult<ContainerNetwork> {
        match mode {
            NetworkMode::Bridge { network_name } => {
                self.setup_bridge_network(container_id, pid, &network_name, ports)
            }
            NetworkMode::Host => self.setup_host_network(container_id),
            NetworkMode::None => self.setup_none_network(container_id, pid),
            NetworkMode::Container {
                container_id: ref target_id,
            } => self.setup_container_network_shared(container_id, target_id),
        }
    }
    fn setup_bridge_network(
        &self,
        container_id: &str,
        pid: i32,
        network_name: &str,
        ports: Vec<PortMapping>,
    ) -> ContainerResult<ContainerNetwork> {
        todo!()
    }
    fn setup_host_network(&self, container_id: &str) -> ContainerResult<ContainerNetwork> {
        todo!()
    }
    fn setup_none_network(
        &self,
        container_id: &str,
        pid: i32,
    ) -> ContainerResult<ContainerNetwork> {
        todo!()
    }
    fn setup_container_network_shared(
        &self,
        container_id: &str,
        target_container_id: &str,
    ) -> ContainerResult<ContainerNetwork> {
        todo!()
    }
    fn cleanup_container_network(&self, container_id: &str) -> ContainerResult<ContainerNetwork> {
        todo!()
    }
    fn delete_network(&self, name: &str) -> ContainerResult<ContainerNetwork> {
        todo!()
    }
}
#[derive(Clone)]
struct IpAllocator {
    subnet: ipnetwork::Ipv4Network,
    allocated: HashSet<Ipv4Addr>,
}
impl IpAllocator {
    fn new(subnet: ipnetwork::Ipv4Network) -> ContainerResult<Self> {
        todo!()
    }
    fn allocator(&mut self) -> ContainerResult<Ipv4Addr> {
        todo!()
    }
    fn release(&mut self, ip: Ipv4Addr) {
        todo!()
    }
}
