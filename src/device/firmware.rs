use std::sync::{Arc, Mutex};

use crate::{
    device::mesh,
    interfaces::hardware_interface::{self},
};

pub struct Firmware {
    name: String,
    model: String,
    hardware_interface: Option<Arc<Mutex<dyn hardware_interface::HardwareInterface>>>,
    mesh: mesh::Mesh,
}

impl Firmware {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            model: String::new(),
            hardware_interface: None,
            mesh: mesh::Mesh::new(),
        }
    }

    pub fn init(
        &mut self,
        name: &str,
        model: &str,
        hardware_interface: Arc<Mutex<dyn hardware_interface::HardwareInterface>>,
    ) {
        self.name = name.to_string();
        self.model = model.to_string();
        self.hardware_interface = Some(hardware_interface);

        let hi = self.hardware_interface.as_mut().unwrap();

        hi.lock().unwrap().printf(format_args!("Hello World!\n"));

        self.mesh.init(self.hardware_interface.clone().unwrap());
    }

    pub fn heartbeat(&mut self) {
        self.mesh.heartbeat();
    }
}
