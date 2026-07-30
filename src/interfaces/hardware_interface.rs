use core::fmt;
use std::fmt::Arguments;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareError {
    Ok,
    NotImplemented,
    SoftwareBug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UwbAddress {
    Short(u16),
    Extended([u8; 8]),
}

impl fmt::Display for UwbAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UwbAddress::Short(short) => write!(f, "Short({})", short),
            UwbAddress::Extended(ext) => write!(
                f,
                "Extended({:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X})",
                ext[0], ext[1], ext[2], ext[3], ext[4], ext[5], ext[6], ext[7]
            ),
        }
    }
}

pub trait UwbAddressImpl {
    fn bytes(&self) -> [u8; 8];
    fn new(
        byte0: u8,
        byte1: u8,
        byte2: u8,
        byte3: u8,
        byte4: u8,
        byte5: u8,
        byte6: u8,
        byte7: u8,
    ) -> Self
    where
        Self: Sized;
}

impl UwbAddressImpl for UwbAddress {
    fn bytes(&self) -> [u8; 8] {
        match self {
            UwbAddress::Short(short) => {
                let mut bytes = [0u8; 8];
                bytes[0] = (*short & 0xff) as u8;
                bytes[1] = ((*short >> 8) & 0xff) as u8;
                bytes
            }
            UwbAddress::Extended(ext) => *ext,
        }
    }

    fn new(
        byte0: u8,
        byte1: u8,
        byte2: u8,
        byte3: u8,
        byte4: u8,
        byte5: u8,
        byte6: u8,
        byte7: u8,
    ) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0] = byte0;
        bytes[1] = byte1;
        bytes[2] = byte2;
        bytes[3] = byte3;
        bytes[4] = byte4;
        bytes[5] = byte5;
        bytes[6] = byte6;
        bytes[7] = byte7;
        UwbAddress::Extended(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingMessage {
    pub source: UwbAddress,
    pub data: Vec<u8>,
}

pub trait HardwareInterface {
    fn printf(&self, args: Arguments<'_>) {
        print!("{}", args);
    }

    fn restart(&self) {}

    fn is_serial_input_available(&self) -> bool {
        false
    }

    fn read_char_over_serial(&self) -> Option<char> {
        None
    }

    fn millis(&self) -> u32 {
        0
    }

    fn delay(&self, ms: u32) {
        let start = self.millis();
        while self.millis().wrapping_sub(start) < ms && self.is_alive() {
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn is_alive(&self) -> bool {
        true
    }

    fn lock_mutex(&mut self, _mutex_id: u32, _timeout_ms: u32) -> bool {
        false
    }

    fn unlock_mutex(&mut self, _mutex_id: u32) -> bool {
        false
    }

    fn get_uwb_address(&self) -> UwbAddress {
        UwbAddress::Short(0)
    }

    fn init_messages(&mut self) -> HardwareError {
        HardwareError::NotImplemented
    }

    fn get_incoming_message(&mut self) -> Option<IncomingMessage> {
        None
    }

    fn send_message(&mut self, _destination: UwbAddress, _data: &[u8]) -> HardwareError {
        HardwareError::NotImplemented
    }

    fn get_number_of_queued_messages(&self) -> i32 {
        0
    }

    fn generate_random_integer(&self) -> u32 {
        0
    }
}
