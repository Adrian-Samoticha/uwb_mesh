use raylib::prelude::*;
use std::sync::Arc;

use crate::message::Message;

/// Properties of a device used for simulation.
///
/// This struct contains properties about a device that are used for simulation.
/// These properties are used for rendering and other tasks.
#[derive(Clone)]
pub struct DeviceProperties {
    /// The position of the device in the 2D plane.
    ///
    /// This is the position of the device that is used for rendering.
    pub position: Vector2,

    /// The number of milliseconds the device has been active.
    ///
    /// This is the number of milliseconds the device has been active.
    pub millis: u32,

    /// Called when an message is received.
    ///
    /// This is a callback that is called when an message is received. The
    /// callback will be called with the message that was received.
    pub on_message_received: Option<Arc<dyn Fn(Arc<Message>) + Send + Sync>>, // TODO: implement this

    /// The virtual console associated with this device.
    ///
    /// This is the virtual console associated with this device. The virtual
    /// console is a simulated console that can be used to input commands and
    /// view the output of the device.
    nothing: (),
    // pub virtual_console: VirtualConsole, // TODO: implement this
}

impl DeviceProperties {
    /// Creates a new DeviceProperties with the given position.
    pub fn new(position: Vector2) -> Self {
        Self {
            position,
            millis: 0,
            nothing: (),
            on_message_received: None,
            // virtual_console: VirtualConsole::new(), // TODO: implement this
        }
    }
}

impl Default for DeviceProperties {
    fn default() -> Self {
        Self {
            position: Vector2::new(0.0, 0.0),
            millis: 0,
            nothing: (),
            on_message_received: None,
            // on_message_received: None,
            // virtual_console: VirtualConsole::new(), // TODO: implement this
        }
    }
}
