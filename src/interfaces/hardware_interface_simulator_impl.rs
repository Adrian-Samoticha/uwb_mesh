use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::interfaces::hardware_interface::UwbAddressImpl;
use crate::interfaces::message_queue::MessageQueue;
use crate::message::Message;

use super::super::simulator::SimulatorMutex;
use super::hardware_interface::{HardwareError, HardwareInterface, IncomingMessage, UwbAddress};

pub struct HardwareInterfaceSimulatorImpl {
    simulator: Weak<SimulatorMutex>,
    id: u32,
    mutexes: HashMap<u32, parking_lot::ReentrantMutex<()>>,
    incoming_message_queue: Arc<parking_lot::Mutex<MessageQueue>>,
}

impl HardwareInterfaceSimulatorImpl {
    pub fn new(simulator: Weak<SimulatorMutex>, id: u32) -> Self {
        HardwareInterfaceSimulatorImpl {
            simulator,
            id,
            mutexes: HashMap::new(),
            incoming_message_queue: Arc::new(parking_lot::Mutex::new(MessageQueue::with_max_size(
                1024,
            ))),
        }
    }

    fn id_to_uwb_address(&self, input_id: u32) -> UwbAddress {
        let id_byte0 = (input_id & 0xff) as u8;
        let id_byte1 = ((input_id >> 8) & 0xff) as u8;
        let id_byte2 = ((input_id >> 16) & 0xff) as u8;
        let id_byte3 = ((input_id >> 24) & 0xff) as u8;

        UwbAddress::new(id_byte0, id_byte1, id_byte2, id_byte3, 0, 0)
    }

    fn uwb_address_to_id(&self, uwb_address: &UwbAddress) -> u32 {
        let bytes = uwb_address.bytes();
        bytes[0] as u32
            + ((bytes[1] as u32) << 8)
            + ((bytes[2] as u32) << 16)
            + ((bytes[3] as u32) << 24)
    }
}

impl HardwareInterface for HardwareInterfaceSimulatorImpl {
    fn printf(&self, args: std::fmt::Arguments<'_>) {
        if let Some(sim_arc) = self.simulator.upgrade() {
            // The ReentrantMutex allows the same thread to lock again without
            // deadlocking, which is required here because printf may be called
            // from within a context that already holds this lock.
            //
            // SAFETY: The ReentrantMutex guarantees exclusive access at all times.
            // UnsafeCell::get() is the only legal way to obtain *mut T from a
            // shared reference; the resulting &mut Simulator is valid while the
            // guard is live.
            let guard = sim_arc.lock();
            let sim = unsafe { &mut *guard.get() };
            if let Some(properties) = sim.get_device_properties(self.id) {
                for c in format!("{}", args).chars() {
                    properties.virtual_console.write_char_to_output(c, false);
                }
            }
        }
    }

    fn is_serial_input_available(&self) -> bool {
        if let Some(sim_arc) = self.simulator.upgrade() {
            let guard = sim_arc.lock();
            // SAFETY: see printf.
            let sim = unsafe { &mut *guard.get() };
            if let Some(properties) = sim.get_device_properties(self.id) {
                !properties.virtual_console.is_input_buffer_empty()
            } else {
                false
            }
        } else {
            false
        }
    }

    fn read_char_over_serial(&self) -> Option<char> {
        if let Some(sim_arc) = self.simulator.upgrade() {
            let guard = sim_arc.lock();
            // SAFETY: see printf.
            let sim = unsafe { &mut *guard.get() };
            if let Some(properties) = sim.get_device_properties(self.id) {
                properties
                    .virtual_console
                    .read_first_char_from_input_buffer()
            } else {
                None
            }
        } else {
            None
        }
    }

    fn millis(&self) -> u32 {
        if let Some(sim_arc) = self.simulator.upgrade() {
            let guard = sim_arc.lock();
            // SAFETY: see printf.
            let sim = unsafe { &mut *guard.get() };
            if let Some(properties) = sim.get_device_properties(self.id) {
                properties.millis
            } else {
                0
            }
        } else {
            0
        }
    }

    fn is_alive(&self) -> bool {
        if let Some(sim_arc) = self.simulator.upgrade() {
            let guard = sim_arc.lock();
            // SAFETY: see printf.
            let sim = unsafe { &*guard.get() };
            sim.is_device_id_present(self.id)
        } else {
            false
        }
    }

    fn lock_mutex(&mut self, mutex_id: u32, timeout: u32) -> bool {
        if !self.mutexes.contains_key(&mutex_id) {
            self.mutexes
                .insert(mutex_id, parking_lot::ReentrantMutex::new(()));
        }

        let time = self.millis();
        while self.millis() - time < timeout && self.is_alive() {
            if self.mutexes[&mutex_id].try_lock().is_some() {
                return true;
            }
        }

        false
    }

    fn unlock_mutex(&mut self, mutex_id: u32) -> bool {
        self.mutexes.contains_key(&mutex_id)
    }

    fn get_uwb_address(&self) -> UwbAddress {
        self.id_to_uwb_address(self.id)
    }

    fn init_messages(&mut self) -> HardwareError {
        let simulator_arc = self.simulator.clone();
        let incoming_message_queue = self.incoming_message_queue.clone();

        let Some(sim_arc) = simulator_arc.upgrade() else {
            return HardwareError::SoftwareBug;
        };

        let guard = sim_arc.lock();
        // SAFETY: see printf.
        let sim = unsafe { &mut *guard.get() };
        sim.register_message_callback(self.id, move |message: Arc<Message>| {
            let sender_id = message.sender_id as u32;
            let sender_uwb_address = UwbAddress::new(
                (sender_id & 0xff) as u8,
                ((sender_id >> 8) & 0xff) as u8,
                ((sender_id >> 16) & 0xff) as u8,
                ((sender_id >> 24) & 0xff) as u8,
                0,
                0,
            );

            let mut queue_message = Vec::new();
            queue_message.extend_from_slice(&sender_uwb_address.bytes());
            queue_message.extend_from_slice(&message.data[..message.length]);

            incoming_message_queue.lock().push(queue_message);
        });
        HardwareError::Ok
    }

    fn get_incoming_message(&mut self) -> Option<IncomingMessage> {
        let queue = self.incoming_message_queue.lock();

        if let Some(message_vec) = queue.pop() {
            if message_vec.len() >= 6 {
                let sender_address = UwbAddress::new(
                    message_vec[0],
                    message_vec[1],
                    message_vec[2],
                    message_vec[3],
                    message_vec[4],
                    message_vec[5],
                );

                let data = message_vec[6..].to_vec();

                Some(IncomingMessage {
                    source: sender_address,
                    data,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    fn send_message(&mut self, destination: UwbAddress, data: &[u8]) -> HardwareError {
        let simulator_arc = self.simulator.clone();

        let Some(sim_arc) = simulator_arc.upgrade() else {
            return HardwareError::SoftwareBug;
        };
        let guard = sim_arc.lock();
        // SAFETY: see printf.
        let sim = unsafe { &mut *guard.get() };

        let sender_id = self.id;
        let broadcast_address = UwbAddress::new(0xff, 0xff, 0xff, 0xff, 0xff, 0xff);

        if destination == broadcast_address {
            let sender_position = match sim.get_device_properties(sender_id) {
                Some(properties) => properties.position,
                None => return HardwareError::SoftwareBug,
            };

            let devices_in_range = sim.get_devices_in_range(sender_position);

            for device_in_range in devices_in_range {
                let device_id = device_in_range.get_id();
                let device_address = self.id_to_uwb_address(device_id as u32);

                if device_address != self.id_to_uwb_address(self.id) {
                    let message = Message::create(
                        sender_id.into(),
                        device_id.into(),
                        sim.get_simulator_properties().default_message_transfer_time,
                        data.len(),
                        data,
                        false,
                    );

                    sim.send_message(Arc::new(message));
                }
            }

            return HardwareError::Ok;
        }

        let receiver_id = self.uwb_address_to_id(&destination);

        if !sim.is_device_id_present(receiver_id) {
            return HardwareError::Ok;
        }

        let sender_position = match sim.get_device_properties(sender_id) {
            Some(properties) => properties.position,
            None => return HardwareError::SoftwareBug,
        };
        let receiver_position = match sim.get_device_properties(receiver_id) {
            Some(properties) => properties.position,
            None => return HardwareError::SoftwareBug,
        };
        let do_drop = sim.do_drop_message(sender_position, receiver_position);

        let message = Message::create(
            sender_id.into(),
            receiver_id.into(),
            sim.get_simulator_properties().default_message_transfer_time,
            data.len(),
            data,
            do_drop,
        );

        sim.send_message(Arc::new(message));

        HardwareError::Ok
    }
}
