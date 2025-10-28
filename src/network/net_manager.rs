use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::cli::ContainerConfig;

use super::*;

pub struct NetworkManager {
    networks: Arc<Mutex<HashMap<String, NetworkConfig>>>,
    container_networks: Arc<Mutex<HashMap<String, ContainerNetwork>>>,
}

#[derive(Clone)]
struct NetworkConfig{
	name:String,
	bridge:Bridge,
	subnet:ipnetwork::
}
