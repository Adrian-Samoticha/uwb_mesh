use crate::interfaces::hardware_interface;

pub struct HaplaFirmwareMiddleware {
    name: String,
    model: String,
    hardware_interface: Option<Box<dyn hardware_interface::HardwareInterface>>,
}

impl HaplaFirmwareMiddleware {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            model: String::new(),
            hardware_interface: None,
        }
    }

    pub fn init(
        &mut self,
        name: &str,
        model: &str,
        hardware_interface: Box<dyn hardware_interface::HardwareInterface>,
    ) {
        self.name = name.to_string();
        self.model = model.to_string();
        self.hardware_interface = Some(hardware_interface);

        println!("before");
        self.hardware_interface
            .as_ref()
            .unwrap()
            .printf(format_args!("Hello World!\n"));
        println!("after");
    }

    pub fn heartbeat(&mut self) {
        // TODO: Implement middleware heartbeat.
    }
}
