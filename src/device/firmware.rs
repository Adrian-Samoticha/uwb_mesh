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
    serial_buffer: Vec<u8>,
}

impl Firmware {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            model: String::new(),
            hardware_interface: None,
            mesh: mesh::Mesh::new(),
            serial_buffer: Vec::new(),
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

        self.handle_serial_input();
    }

    fn handle_serial_input(&mut self) {
        let chars: Vec<char> = {
            if let Some(hi) = &self.hardware_interface {
                let hi = hi.lock().unwrap();
                let mut chars = Vec::new();
                while hi.is_serial_input_available() {
                    if let Some(c) = hi.read_char_over_serial() {
                        chars.push(c);
                    }
                }
                chars
            } else {
                Vec::new()
            }
        };

        for c in chars {
            if c == ';' {
                let command = String::from_utf8_lossy(&self.serial_buffer).to_string();
                self.execute_serial_command(&command);
                self.serial_buffer.clear();
            } else {
                self.serial_buffer.push(c as u8);
            }
        }
    }

    fn execute_serial_command(&mut self, command: &str) {
        let command = command.trim();
        let parts: Vec<&str> = command.split_whitespace().collect();
        let cmd_name = parts.get(0).map(|s| *s).unwrap_or("");

        match cmd_name {
            "send_ogm" => {
                self.mesh.send_ogm();
            }
            "print_rt" => {
                let routing_table = self.mesh.get_routing_table();
                let hi = self.hardware_interface.as_mut().unwrap();
                hi.lock().unwrap().printf(format_args!("Routing Table:\n"));
                for (dest, next_hop) in routing_table {
                    hi.lock().unwrap().printf(format_args!(
                        "Destination: {:?}, Next Hop: {:?}\n",
                        dest, next_hop
                    ));
                }
            }
            _ => {
                let hi = self.hardware_interface.as_mut().unwrap();
                hi.lock()
                    .unwrap()
                    .printf(format_args!("Unknown command: {}\n", command));
            }
        }
    }
}
