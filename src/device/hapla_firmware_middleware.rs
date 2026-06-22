use super::interface_implementations::application_interface_impl::ApplicationInterfaceImpl;
use super::interface_implementations::hardware_interface_impl::HardwareInterfaceImpl;

pub struct HaplaFirmwareMiddleware;

impl HaplaFirmwareMiddleware {
    pub fn new() -> Self {
        Self
    }

    pub fn init(
        &mut self,
        _name: &str,
        _model: &str,
        _hardware_interface: HardwareInterfaceImpl,
        _application_interface: ApplicationInterfaceImpl,
    ) {
        // TODO: Implement middleware init.
    }

    pub fn heartbeat(&mut self) {
        // TODO: Implement middleware heartbeat.
    }
}
