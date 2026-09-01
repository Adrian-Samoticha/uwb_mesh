extern crate alloc;
use alloc::vec::Vec;
use std::sync::{Arc, Mutex};

use crate::device::array_map::ArrayMap;
use crate::interfaces::hardware_interface::{self, UwbAddress, UwbAddressImpl};

enum Message {
    ProbeRequest {},
    ProbeResponse {},
    PartnerRequest {},
    Ogm {
        source: UwbAddress,
        sequence_number: u32,
    },
    Unicast {
        source: UwbAddress,
        destination: UwbAddress,
        data: Vec<u8>,
    },
}

impl Message {
    fn to_data(&self) -> Vec<u8> {
        match self {
            Message::ProbeRequest {} => vec![0x01],
            Message::ProbeResponse {} => vec![0x02],
            Message::PartnerRequest {} => vec![0x03],
            Message::Ogm {
                source,
                sequence_number,
            } => {
                let mut data = vec![0x04];
                data.extend_from_slice(&source.bytes());
                data.extend_from_slice(&sequence_number.to_be_bytes());
                data
            }
            Message::Unicast {
                source,
                destination,
                data,
            } => {
                let mut result = vec![0x05];
                result.extend_from_slice(&source.bytes());
                result.extend_from_slice(&destination.bytes());
                result.extend_from_slice(data);
                result
            }
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
            Some(0x04) => {
                if data.len() < 1 + 8 + 4 {
                    return None;
                }
                let source_bytes = &data[1..9];
                let sequence_number_bytes = &data[9..13];

                let source = UwbAddress::Extended(source_bytes.try_into().ok()?);
                let sequence_number = u32::from_be_bytes(sequence_number_bytes.try_into().ok()?);

                Some(Message::Ogm {
                    source,
                    sequence_number,
                })
            }
            Some(0x05) => {
                if data.len() < 1 + 8 + 8 {
                    return None;
                }
                let source_bytes = &data[1..9];
                let destination_bytes = &data[9..17];
                let source = UwbAddress::Extended(source_bytes.try_into().ok()?);
                let destination = UwbAddress::Extended(destination_bytes.try_into().ok()?);
                let payload = data[17..].to_vec();

                Some(Message::Unicast {
                    source,
                    destination,
                    data: payload,
                })
            }
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
                    destination: message.destination,
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

#[derive(Eq, Hash, PartialEq, Clone, Copy)]
struct UwbAddressAndSequenceNumber {
    address: hardware_interface::UwbAddress,
    sequence_number: u32,
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

    next_ogm_sequence_number: u32,

    /// Maps received OGM messages to their age in milliseconds. This is used to determine when to discard old OGMs.
    received_ogm_to_age_in_ms: [(UwbAddressAndSequenceNumber, u32); 4],

    routing_table: ArrayMap<hardware_interface::UwbAddress, hardware_interface::UwbAddress, 4>,
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
            next_ogm_sequence_number: 0,
            received_ogm_to_age_in_ms: [(
                UwbAddressAndSequenceNumber {
                    address: hardware_interface::UwbAddress::Short(0),
                    sequence_number: 0,
                },
                0,
            ); 4],
            routing_table: ArrayMap::new([None; 4], [hardware_interface::UwbAddress::Short(0); 4]),
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
                        self.partners.push(*partner);
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

            self.age_received_ogms(10);
        }
    }

    fn receive_messages(&mut self) {
        // Drain the queue first so the hardware interface lock is not held while
        // calling other &mut self methods (get_adjacent_devices, forward_ogm, etc.).
        let incoming_messages: Vec<_> = {
            let mut hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
            let mut messages = Vec::new();
            while let Some(msg) = hi.get_incoming_message() {
                messages.push(msg);
            }
            messages
        };

        for incoming_message in incoming_messages {
            if let Some(message) = Message::from_data(&incoming_message.data) {
                match message {
                    Message::ProbeRequest {} => {
                        let time_until_send_in_ms = {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "Received probe message from {}\n",
                                incoming_message.source
                            ));
                            hi.generate_random_integer() % 50 * 10 + 10
                        };

                        let outgoing_message = OutgoingMessage {
                            destination: incoming_message.source,
                            data: Message::ProbeResponse {}.to_data(),
                            time_to_send_in_ms: time_until_send_in_ms as i32,
                        };
                        self.outgoing_message_handler.add_message(outgoing_message);
                    }
                    Message::ProbeResponse {} => {
                        {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "Received probe response from {}\n",
                                incoming_message.source
                            ));
                        }

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
                        if !self.partners_of.contains(&incoming_message.source) {
                            self.partners_of.push(incoming_message.source);
                        }
                    }
                    Message::Ogm {
                        source,
                        sequence_number,
                    } => {
                        {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "Received OGM from {} with sequence number {}\n",
                                source, sequence_number
                            ));
                        }

                        let key = UwbAddressAndSequenceNumber {
                            address: source,
                            sequence_number,
                        };
                        if self
                            .received_ogm_to_age_in_ms
                            .iter()
                            .any(|(k, _)| k == &key)
                        {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "Already received OGM from {} with sequence number {}, ignoring\n",
                                source, sequence_number
                            ));
                            continue;
                        }
                        if let Some(slot) =
                            self.received_ogm_to_age_in_ms.iter_mut().find(|(k, age)| {
                                (k.address == hardware_interface::UwbAddress::Short(0)
                                    && k.sequence_number == 0)
                                    || *age > 10000
                            })
                        {
                            *slot = (key, 0);
                        } else {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "No available slot for OGM from {} with sequence number {}, ignoring\n",
                                source, sequence_number
                            ));
                            continue;
                        }

                        if !self.routing_table.contains_key(&source)
                            || self.routing_table[&source] != incoming_message.source
                        {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "Updating routing table: {} -> {}\n",
                                source, incoming_message.source
                            ));

                            self.routing_table.insert(source, incoming_message.source);
                        }

                        let adjacent_devices = self.get_adjacent_devices();
                        for device in adjacent_devices {
                            if device == source {
                                continue;
                            }
                            self.forward_ogm(&device, source, sequence_number);
                        }
                    }
                    Message::Unicast {
                        source,
                        destination,
                        data,
                    } => {
                        let is_addressed_to_me = {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            destination == hi.get_uwb_address()
                        };

                        if is_addressed_to_me {
                            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                            hi.printf(format_args!(
                                "Received unicast message from {}: {:?}\n",
                                source, data
                            ));
                        } else {
                            let next_hop = self.routing_table.get(&destination);
                            if let Some(next_hop) = next_hop {
                                let outgoing_message = OutgoingMessage {
                                    destination: *next_hop,
                                    data: Message::Unicast {
                                        source,
                                        destination,
                                        data,
                                    }
                                    .to_data(),
                                    time_to_send_in_ms: 0,
                                };
                                self.outgoing_message_handler.add_message(outgoing_message);

                                let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                                hi.printf(format_args!(
                                    "Forwarding unicast message from {} to {} via next hop {}\n",
                                    source, destination, next_hop
                                ));
                            } else {
                                let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
                                hi.printf(format_args!(
                                    "No route to {} for unicast message from {}, dropping\n",
                                    destination, source
                                ));
                            }
                        }
                    }
                }
            } else {
                let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
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
            destination: *partner,
            data,
            time_to_send_in_ms: time_until_send_in_ms as i32,
        };
        self.outgoing_message_handler.add_message(outgoing_message);
    }

    fn get_adjacent_devices(&self) -> Vec<hardware_interface::UwbAddress> {
        let mut adjacent_devices = self.partners.clone();
        adjacent_devices.extend(self.partners_of.clone());
        adjacent_devices
    }

    pub fn send_ogm(&mut self) {
        let adjacent_devices = self.get_adjacent_devices();
        for device in adjacent_devices {
            let message = Message::Ogm {
                source: self
                    .hardware_interface
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .get_uwb_address(),
                sequence_number: self.next_ogm_sequence_number,
            };
            let data = message.to_data();
            let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
            let time_until_send_in_ms = hi.generate_random_integer() % 2 * 10;

            let outgoing_message = OutgoingMessage {
                destination: device,
                data,
                time_to_send_in_ms: time_until_send_in_ms as i32,
            };
            self.outgoing_message_handler.add_message(outgoing_message);
        }

        self.next_ogm_sequence_number = self.next_ogm_sequence_number.wrapping_add(1);
    }

    fn forward_ogm(
        &mut self,
        destination: &hardware_interface::UwbAddress,
        source: hardware_interface::UwbAddress,
        sequence_number: u32,
    ) {
        let message = Message::Ogm {
            source,
            sequence_number,
        };
        let data = message.to_data();
        let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
        let time_until_send_in_ms = hi.generate_random_integer() % 2 * 10;

        let outgoing_message = OutgoingMessage {
            destination: *destination,
            data,
            time_to_send_in_ms: time_until_send_in_ms as i32,
        };
        self.outgoing_message_handler.add_message(outgoing_message);
    }

    fn age_received_ogms(&mut self, time_passed_in_ms: u32) {
        for (_, age) in self.received_ogm_to_age_in_ms.iter_mut() {
            *age = age.saturating_add(time_passed_in_ms);
        }
    }

    pub fn get_routing_table(
        &self,
    ) -> ArrayMap<hardware_interface::UwbAddress, hardware_interface::UwbAddress, 4> {
        self.routing_table
    }

    pub fn send_unicast(&mut self, destination: hardware_interface::UwbAddress, data: Vec<u8>) {
        let own_address = self
            .hardware_interface
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .get_uwb_address();

        let message = Message::Unicast {
            source: own_address,
            destination,
            data,
        };
        let data = message.to_data();
        let hi = self.hardware_interface.as_ref().unwrap().lock().unwrap();
        let time_until_send_in_ms = hi.generate_random_integer() % 2 * 10;

        let immediate_destination = if let Some(next_hop) = self.routing_table.get(&destination) {
            *next_hop
        } else {
            destination
        };

        let outgoing_message = OutgoingMessage {
            destination: immediate_destination,
            data,
            time_to_send_in_ms: time_until_send_in_ms as i32,
        };
        self.outgoing_message_handler.add_message(outgoing_message);
    }
}
