use std::sync::{Arc, Mutex};

use crate::interfaces::hardware_interface::{self, UwbAddressImpl};

enum Message {
    ProbeRequest {},
    ProbeResponse {},
    PartnerRequest {},
}

impl Message {
    fn to_data(&self) -> Vec<u8> {
        match self {
            Message::ProbeRequest {} => vec![0x01],
            Message::ProbeResponse {} => vec![0x02],
            Message::PartnerRequest {} => vec![0x03],
        }
    }

    fn from_data(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        match data.first().copied() {
            Some(0x01) => Some(Message::ProbeRequest {}),
            Some(0x02) => Some(Message::ProbeResponse {}),
            Some(0x03) => Some(Message::PartnerRequest {}),
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

struct OutgoingMessage {
    destination: hardware_interface::UwbAddress,
    data: Vec<u8>,
    time_to_send_in_ms: i32,
}

struct OutgoingMessageHandler {
    messages: Vec<OutgoingMessage>,
}

impl OutgoingMessageHandler {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    fn add_message(&mut self, message: OutgoingMessage) {
        self.messages.push(message);
    }

    /// Processes the messages and returns a vector of messages that are ready
    /// to be sent.
    fn process_messages(&mut self, time_passed_in_ms: i32) -> Vec<OutgoingMessage> {
        let mut ready_messages = Vec::new();

        for message in &mut self.messages {
            message.time_to_send_in_ms -= time_passed_in_ms;

            if message.time_to_send_in_ms <= 0 {
                ready_messages.push(OutgoingMessage {
                    destination: message.destination.clone(),
                    data: message.data.clone(),
                    time_to_send_in_ms: 0,
                });
            }
        }
        self.messages
            .retain(|message| message.time_to_send_in_ms > 0);

        ready_messages
    }
}

pub struct Mesh {
    hardware_interface: Option<Arc<Mutex<dyn hardware_interface::HardwareInterface>>>,
    time_of_last_step: u32,
    time_in_steps: u32,
    stage: MeshStage,
    outgoing_message_handler: OutgoingMessageHandler,

    /// List of devices that have been found during the InitConnections stage
    /// and have been dedicated as partners.
    /// A partner is a device for which a network edge has been established and
    /// is maintained.
    partners: Vec<hardware_interface::UwbAddress>,

    /// List of devices for which this device is a partner.
    partners_of: Vec<hardware_interface::UwbAddress>,
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
            outgoing_message_handler: OutgoingMessageHandler::new(),
            partners: Vec::new(),
            partners_of: Vec::new(),
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

        self.outgoing_message_handler
            .process_messages(10)
            .into_iter()
            .for_each(|message| {
                let mut hi = self.hardware_interface.as_mut().unwrap().lock().unwrap();
                hi.send_message(message.destination, &message.data);
                drop(hi);
            });

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
                let random_number = hi.generate_random_integer() % 100;
                let number_of_partners = if random_number < 10 { 2 } else { 1 };
                for i in 0..number_of_partners {
                    if let Some(partner) = found_devices.get(i) {
                        self.partners.push(partner.clone());
                    }
                }

                hi.printf(format_args!(
                    "InitConnections stage done. Found devices: {:?}; Partners: {:?}\n",
                    found_devices, self.partners
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

        if let MeshStage::Running = &self.stage {
            if self.time_in_steps % 500 == 0 {
                let partners = self.partners.clone();
                for partner in partners {
                    self.send_partner_request(&partner);
                }
            }
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
                        let time_until_send_in_ms = hi.generate_random_integer() % 50 * 10 + 10;

                        let outgoing_message = OutgoingMessage {
                            destination: incoming_message.source,
                            data,
                            time_to_send_in_ms: time_until_send_in_ms as i32,
                        };
                        self.outgoing_message_handler.add_message(outgoing_message);
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
                    Message::PartnerRequest {} => {
                        hi.printf(format_args!(
                            "Received partner request from {}\n",
                            incoming_message.source
                        ));

                        if !self.partners_of.contains(&incoming_message.source) {
                            self.partners_of.push(incoming_message.source);
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
        let message = Message::ProbeRequest {};
        let data = message.to_data();
        let broadcast_address =
            hardware_interface::UwbAddress::new(0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff);
        let time_until_send_in_ms = 0;

        let outgoing_message = OutgoingMessage {
            destination: broadcast_address,
            data,
            time_to_send_in_ms: time_until_send_in_ms,
        };
        self.outgoing_message_handler.add_message(outgoing_message);
    }

    fn send_partner_request(&mut self, partner: &hardware_interface::UwbAddress) {
        let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();

        let message = Message::PartnerRequest {};
        let data = message.to_data();
        let time_until_send_in_ms = hi.generate_random_integer() % 10 * 10;

        let outgoing_message = OutgoingMessage {
            destination: partner.clone(),
            data,
            time_to_send_in_ms: time_until_send_in_ms as i32,
        };
        self.outgoing_message_handler.add_message(outgoing_message);
    }
}
