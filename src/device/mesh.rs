use std::sync::{Arc, Mutex};

use crate::interfaces::hardware_interface::{self, UwbAddressImpl};

enum Message {
    ProbeRequest {},
    ProbeResponse {},
}

impl Message {
    fn to_data(&self) -> Vec<u8> {
        match self {
            Message::ProbeRequest {} => vec![0x01],
            Message::ProbeResponse {} => vec![0x02],
        }
    }

    fn from_data(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        match data.first().copied() {
            Some(0x01) => Some(Message::ProbeRequest {}),
            Some(0x02) => Some(Message::ProbeResponse {}),
            _ => None,
        }
    }
}

enum MeshStage {
    InitConnections {
        time_until_done_in_ms: i32,
        time_until_broadcast_in_ms: i32,
        found_devices: Vec<hardware_interface::UwbAddress>,
    },
    Running,
}

pub struct Mesh {
    hardware_interface: Option<Arc<Mutex<dyn hardware_interface::HardwareInterface>>>,
    time_of_last_step: u32,
    time_in_steps: u32,
    stage: MeshStage,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            hardware_interface: None,
            time_of_last_step: 0,
            time_in_steps: 0,
            stage: MeshStage::InitConnections {
                time_until_done_in_ms: 1000,
                time_until_broadcast_in_ms: 0,
                found_devices: Vec::new(),
            },
        }
    }

    fn randomize_time_until_broadcast(&mut self) {
        let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
        let random_value = (hi.generate_random_integer() % 50) * 10 + 10;

        if let MeshStage::InitConnections {
            time_until_done_in_ms: _,
            time_until_broadcast_in_ms,
            found_devices: _,
        } = &mut self.stage
        {
            *time_until_broadcast_in_ms = random_value as i32;
        }
    }

    pub fn init(
        &mut self,
        hardware_interface: Arc<Mutex<dyn hardware_interface::HardwareInterface>>,
    ) {
        self.hardware_interface = Some(hardware_interface);

        let hi = self.hardware_interface.as_ref().unwrap();

        hi.lock().unwrap().init_messages();

        self.randomize_time_until_broadcast();
    }

    pub fn heartbeat(&mut self) {
        assert!(self.hardware_interface.is_some(), "Mesh not initialized");

        self.receive_messages();

        loop {
            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
            let elapsed = hi.millis() - self.time_of_last_step;
            drop(hi);

            if elapsed >= 10 {
                self.time_of_last_step += 10;
                self.time_in_steps += 1;
                self.on_10ms_passed();
            } else {
                break;
            }
        }
    }

    fn on_10ms_passed(&mut self) {
        let mut should_broadcast_probe = false;

        if let MeshStage::InitConnections {
            time_until_done_in_ms,
            time_until_broadcast_in_ms,
            found_devices,
        } = &mut self.stage
        {
            if *time_until_done_in_ms > 0 {
                *time_until_done_in_ms -= 10;
            }

            if *time_until_broadcast_in_ms > 0 {
                *time_until_broadcast_in_ms -= 10;

                if *time_until_broadcast_in_ms <= 0 {
                    should_broadcast_probe = true;
                }
            }

            if *time_until_done_in_ms <= 0 {
                let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                hi.printf(format_args!(
                    "InitConnections stage done. Found devices: {:?}\n",
                    found_devices
                ));
                drop(hi);

                self.stage = MeshStage::Running;
            }
        }

        if should_broadcast_probe {
            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
            hi.printf(format_args!("Broadcasting probe message\n"));
            drop(hi);

            self.broadcast_probe_request();
        }
    }

    fn receive_messages(&mut self) {
        let mut hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();

        while let Some(incoming_message) = hi.get_incoming_message() {
            if let Some(message) = Message::from_data(&incoming_message.data) {
                match message {
                    Message::ProbeRequest {} => {
                        hi.printf(format_args!(
                            "Received probe message from {}\n",
                            incoming_message.source
                        ));

                        let message = Message::ProbeResponse {};
                        let data = message.to_data();
                        hi.send_message(incoming_message.source, &data);
                    }
                    Message::ProbeResponse {} => {
                        hi.printf(format_args!(
                            "Received probe response from {}\n",
                            incoming_message.source
                        ));

                        if let MeshStage::InitConnections {
                            time_until_done_in_ms: _,
                            time_until_broadcast_in_ms: _,
                            found_devices,
                        } = &mut self.stage
                        {
                            if !found_devices.contains(&incoming_message.source) {
                                found_devices.push(incoming_message.source);
                            }
                        }
                    }
                }
            } else {
                hi.printf(format_args!(
                    "Received unknown message from {}: {:?}\n",
                    incoming_message.source, incoming_message.data
                ));
            }
        }
    }

    fn broadcast_probe_request(&mut self) {
        let mut hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();

        let message = Message::ProbeRequest {};
        let data = message.to_data();
        let broadcast_address =
            hardware_interface::UwbAddress::new(0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff);

        hi.send_message(broadcast_address, &data);

        drop(hi);
    }
}
