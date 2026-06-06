use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub type Id = u64;

use crate::simulator::Simulator;

pub struct Device {
    id: Id,
    middleware: Mutex<Option<hapla_firmware_middleware::HaplaFirmwareMiddleware>>,
    heartbeats_left: AtomicI32,
}

impl Device {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            middleware: Mutex::new(None),
            heartbeats_left: AtomicI32::new(0),
        }
    }

    pub fn create() -> Arc<Self> {
        let new_id = crate::id_provider::get_next_id();
        Arc::new(Self::new(new_id))
    }

    pub fn init(self: &Arc<Self>, name: &str, model: &str, simulator: *mut Simulator) {
        let mut hapla_firmware_middleware =
            hapla_firmware_middleware::HaplaFirmwareMiddleware::new();

        let hardware_interface =
            interface_implementations::hardware_interface_impl::HardwareInterfaceImpl::new(
                simulator, self.id,
            );
        let application_interface =
            interface_implementations::application_interface_impl::ApplicationInterfaceImpl::new();

        hapla_firmware_middleware.init(name, model, hardware_interface, application_interface);

        let pending = self.heartbeats_left.swap(0, Ordering::Relaxed);
        for _ in 0..pending {
            hapla_firmware_middleware.heartbeat();
        }

        let mut guard = self.middleware.lock().expect("middleware mutex poisoned");
        *guard = Some(hapla_firmware_middleware);
    }

    pub fn heartbeat(&self) {
        let mut guard = self.middleware.lock().expect("middleware mutex poisoned");
        if let Some(middleware) = guard.as_mut() {
            middleware.heartbeat();
        } else {
            self.heartbeats_left.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn get_heartbeats_left(&self) -> i32 {
        self.heartbeats_left.load(Ordering::Relaxed)
    }

    pub fn destroy(&self) {
        let mut guard = self.middleware.lock().expect("middleware mutex poisoned");
        *guard = None;
    }

    pub fn get_id(&self) -> Id {
        self.id
    }
}

// ----------------------
// Placeholder modules
// ----------------------

mod hapla_firmware_middleware {
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
}

mod interface_implementations {
    pub mod hardware_interface_impl {
        use crate::device::Id;
        use crate::simulator::Simulator;

        pub struct HardwareInterfaceImpl {
            _simulator: *mut Simulator,
            _id: Id,
        }

        impl HardwareInterfaceImpl {
            pub fn new(simulator: *mut Simulator, id: Id) -> Self {
                Self {
                    _simulator: simulator,
                    _id: id,
                }
            }
        }
    }

    pub mod application_interface_impl {
        pub struct ApplicationInterfaceImpl;

        impl ApplicationInterfaceImpl {
            pub fn new() -> Self {
                Self
            }
        }
    }
}
