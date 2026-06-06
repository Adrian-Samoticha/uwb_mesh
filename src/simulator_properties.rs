pub struct SimulatorProperties {
    /// The default time it takes for a message to be transferred,
    /// in milliseconds.
    ///
    /// This is the time that is used for the transfer of a message if no
    /// other transfer time is specified.
    pub default_message_transfer_time: i32,

    /// The range at which the drop chance for messages is
    /// `min_transmission_range_message_drop_chance`.
    pub min_transmission_range: f64,
    /// The range at which the drop chance for messages is
    /// `max_transmission_range_message_drop_chance`.
    pub max_transmission_range: f64,

    /// The drop chance for messages at `min_transmission_range`.
    pub min_transmission_range_message_drop_chance: f64,

    /// The drop chance for messages at `max_transmission_range`.
    pub max_transmission_range_message_drop_chance: f64,
}

impl Default for SimulatorProperties {
    fn default() -> Self {
        Self {
            default_message_transfer_time: 15,
            min_transmission_range: 256.0 - 32.0,
            max_transmission_range: 256.0 + 32.0,
            min_transmission_range_message_drop_chance: 0.01,
            max_transmission_range_message_drop_chance: 1.0,
        }
    }
}
