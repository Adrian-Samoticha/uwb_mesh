use std::sync::atomic::{AtomicU64, Ordering};

pub type ID = u64;
pub type WifiChannel = u8;

static GLOBAL_ID_PROVIDER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: ID,
    /// The ID of the device that sent this message.
    pub sender_id: ID,
    /// The ID of the device that this message is intended for.
    pub receiver_id: ID,
    /// The time it takes for the message to be transmitted.
    pub transmission_time: i32,
    /// The time period until this message will be received.
    pub time_until_arrival: i32,
    /// The channel number on which this message will be sent.
    pub channel: WifiChannel,
    /// The length of the data that is being sent.
    pub length: usize,
    /// The data that is being sent.
    pub data: [u8; 256],
    /// Whether this message will be dropped before arriving at its destination.
    pub will_be_dropped: bool,
}

impl Message {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ID,
        sender_id: ID,
        receiver_id: ID,
        transmission_time: i32,
        time_until_arrival: i32,
        channel: WifiChannel,
        length: usize,
        data: &[u8],
        will_be_dropped: bool,
    ) -> Self {
        let copy_len = length.min(data.len()).min(256);
        let mut payload = [0u8; 256];
        payload[..copy_len].copy_from_slice(&data[..copy_len]);

        Self {
            id,
            sender_id,
            receiver_id,
            transmission_time,
            time_until_arrival,
            channel,
            length: copy_len,
            data: payload,
            will_be_dropped,
        }
    }

    pub fn create(
        sender_id: ID,
        receiver_id: ID,
        transmission_time: i32,
        channel: WifiChannel,
        length: usize,
        data: &[u8],
        will_be_dropped: bool,
    ) -> Self {
        let id = GLOBAL_ID_PROVIDER.fetch_add(1, Ordering::Relaxed);
        Self::new(
            id,
            sender_id,
            receiver_id,
            transmission_time,
            transmission_time,
            channel,
            length,
            data,
            will_be_dropped,
        )
    }

    /// The type is the first byte of the message.
    pub fn get_type(&self) -> u8 {
        self.data[0]
    }

    /// The transmission percentage as a value between 0.0 and 1.0.
    pub fn get_transmission_percentage(&self) -> f64 {
        if self.transmission_time == 0 {
            return 1.0;
        }

        1.0 - (self.time_until_arrival as f64) / (self.transmission_time as f64)
    }
}
