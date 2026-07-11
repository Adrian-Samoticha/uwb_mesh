use crate::device::Device;
use crate::device_properties::DeviceProperties;
use crate::message::Message;
use crate::raymath::{vector2_add, vector2_rotate, vector2_scale, vector2_subtract};
use crate::simulator_properties;
use crate::templates;

use raylib::prelude::*;
use std::cell::UnsafeCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

lazy_static::lazy_static! {
    /// A recursive mutex used to protect the shared resources of the simulator.
    ///
    /// This mutex is used to synchronize access to the simulator's shared data
    /// structures, such as the list of devices and the device properties.
    ///
    /// All member functions that modify the shared data structures should acquire
    /// a lock on this mutex before making any modifications. This ensures that
    /// multiple threads accessing the simulator will not interfere with each
    /// other.
    static ref SIMULATOR_MUTEX: parking_lot::ReentrantMutex<()> = parking_lot::ReentrantMutex::new(());
}

/// Type alias for the shared simulator handle.
///
/// `UnsafeCell` provides interior mutability needed to obtain `&mut Simulator`
/// through the `ReentrantMutexGuard` (which only exposes `Deref`, not `DerefMut`).
/// Call `guard.get()` on the guard to obtain a `*mut Simulator`, then wrap it
/// in `&mut *ptr` inside an `unsafe` block.
pub type SimulatorMutex = parking_lot::ReentrantMutex<UnsafeCell<Simulator>>;

pub struct Simulator {
    /// A weak reference to itself, used for creating hardware interfaces for
    /// devices.
    self_ref: Weak<SimulatorMutex>,

    /// The properties of the simulator.
    properties: simulator_properties::SimulatorProperties,

    /// The position of the camera in the simulated space.
    camera_position: Vector2,

    /// The zoom level of the camera.
    camera_zoom: f64,

    /// The current simulation time.
    simulation_time: AtomicI32,

    /// The list of devices in the simulator.
    devices: Vec<std::sync::Arc<Device>>,

    /// The list of messages in the simulator.
    messages: Vec<Arc<Message>>,

    /// A map from device ID to its properties in the simulator.
    device_id_to_properties: std::collections::HashMap<u32, DeviceProperties>,

    /// The action to be performed when the left mouse button is pressed.
    left_mouse_button_pressed_action: Option<Box<dyn FnMut(&mut Simulator, &mut RaylibDrawHandle)>>,

    /// The action to be performed when the left mouse button is released.
    left_mouse_button_released_action:
        Option<Box<dyn FnMut(&mut Simulator, &mut RaylibDrawHandle)>>,

    /// The timestamp of the last left mouse button press event.
    last_left_mouse_button_pressed_time: f64,

    /// The timestamp of the last left mouse button release event.
    last_left_mouse_button_released_time: f64,

    /// The position of the last left mouse button press event in screen coordinates.
    last_left_mouse_button_pressed_position: Vector2,

    /// The position of the last left mouse button release event in screen coordinates.
    last_left_mouse_button_released_position: Vector2,

    /// The set of IDs of the selected items.
    selected_items: HashSet<u32>,

    /// Indicates whether the simulation is currently paused.
    is_paused: bool,

    /// Indicates whether the drag operation is currently active.
    do_drag: bool,

    /// The currently active dialog.
    current_dialog: Option<Box<dyn FnMut(&mut Simulator, &mut RaylibDrawHandle)>>,
}

unsafe impl Sync for Simulator {}

impl Simulator {
    /// Creates a new simulator with the default properties.
    pub fn new() -> Arc<SimulatorMutex> {
        let simulator = Arc::new(SimulatorMutex::new(UnsafeCell::new(Self {
            self_ref: Weak::new(),
            properties: simulator_properties::SimulatorProperties::default(),
            camera_position: Vector2::new(0.0, 0.0),
            camera_zoom: 1.0,
            simulation_time: AtomicI32::new(0),
            messages: Vec::new(),
            devices: Vec::new(),
            device_id_to_properties: std::collections::HashMap::new(),
            left_mouse_button_pressed_action: None,
            left_mouse_button_released_action: None,
            last_left_mouse_button_pressed_time: 0.0,
            last_left_mouse_button_released_time: 0.0,
            last_left_mouse_button_pressed_position: Vector2::new(0.0, 0.0),
            last_left_mouse_button_released_position: Vector2::new(0.0, 0.0),
            selected_items: HashSet::new(),
            is_paused: false,
            do_drag: false,
            current_dialog: None,
        })));

        {
            let guard = simulator.lock();
            // SAFETY: The ReentrantMutex guarantees exclusive access at all times.
            // UnsafeCell::get() is the only legal way to obtain *mut T from a shared
            // reference; the resulting &mut Simulator is valid for the guard's lifetime.
            let sim = unsafe { &mut *guard.get() };
            sim.self_ref = Arc::downgrade(&simulator);
        }

        simulator
    }

    fn real_position_to_screen_position(
        &self,
        position: Vector2,
        d: &mut RaylibDrawHandle,
    ) -> Vector2 {
        let window_width = d.get_screen_width();
        let window_height = d.get_screen_height();

        let screen_size = Vector2::new(window_width as f32, window_height as f32);
        let half_screen_size = screen_size * 0.5;

        let relative_to_center = (position - self.camera_position) * self.camera_zoom as f32;

        relative_to_center + half_screen_size
    }

    fn screen_position_to_real_position(
        &self,
        position: Vector2,
        d: &mut RaylibDrawHandle,
    ) -> Vector2 {
        let window_width = d.get_screen_width();
        let window_height = d.get_screen_height();

        let screen_size = Vector2::new(window_width as f32, window_height as f32);
        let half_screen_size = screen_size * 0.5;

        let relative_to_center = (position - half_screen_size) * (1.0 / self.camera_zoom as f32);

        relative_to_center + self.camera_position
    }

    fn handle_actions(&mut self, d: &mut RaylibDrawHandle) {
        if let Some(mut action) = self.left_mouse_button_pressed_action.take() {
            action(self, d);
            self.last_left_mouse_button_pressed_time = d.get_time();
            self.last_left_mouse_button_pressed_position = d.get_mouse_position();
        }

        if let Some(mut action) = self.left_mouse_button_released_action.take() {
            action(self, d);
            self.last_left_mouse_button_released_time = d.get_time();
            self.last_left_mouse_button_released_position = d.get_mouse_position();
        }
    }

    fn set_actions_to_default_state(&mut self, d: &mut RaylibDrawHandle) {
        if d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            self.left_mouse_button_pressed_action = Some(Box::new(|_, _| {}));
        }
        if d.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT) {
            self.left_mouse_button_released_action = Some(Box::new(|simulator, _| {
                simulator.selected_items.clear();
            }));
        }
    }

    fn handle_device_step(&mut self, step_size: u32) {
        for device in &self.devices {
            let device_id: u32 = device.get_id().try_into().unwrap();
            self.device_id_to_properties
                .get_mut(&device_id)
                .unwrap()
                .millis += step_size;

            device.heartbeat();
        }
    }
    fn remove_messages_whose_devices_have_been_deleted(&mut self) {
        self.messages.retain(|message| {
            self.device_id_to_properties
                .contains_key(&(message.receiver_id as u32))
                && self
                    .device_id_to_properties
                    .contains_key(&(message.sender_id as u32))
        });
    }

    fn remove_arrived_messages(&mut self) {
        self.messages
            .retain(|message| message.time_until_arrival > 0);
    }

    fn handle_message_step(&mut self, step_size: u32) {
        self.remove_messages_whose_devices_have_been_deleted();

        let mut arrived_message_indices = Vec::new();

        for (index, message) in self.messages.iter_mut().enumerate() {
            if let Some(message) = Arc::get_mut(message) {
                message.time_until_arrival -= step_size as i32;

                if message.time_until_arrival <= 0 {
                    arrived_message_indices.push(index);
                }
            }
        }

        for index in arrived_message_indices {
            let message = Arc::clone(&self.messages[index]);
            self.receive_message(message);
        }

        self.remove_arrived_messages();
    }

    fn receive_message(&mut self, message: Arc<Message>) {
        if message.will_be_dropped {
            return;
        }

        let receiver_id: u32 = match message.receiver_id.try_into() {
            Ok(id) => id,
            Err(_) => return,
        };

        assert!(self.device_id_to_properties.contains_key(&receiver_id));

        let Some(receiver_properties) = self.device_id_to_properties.get_mut(&receiver_id) else {
            return;
        };

        let on_message_received = receiver_properties
            .on_message_received
            .as_mut()
            .expect("Receiver properties should have a message handler");

        on_message_received(message);
    }

    fn get_distance_between_points(&self, point1: Vector2, point2: Vector2) -> f32 {
        ((point1.x - point2.x).powi(2) + (point1.y - point2.y).powi(2)).sqrt()
    }

    fn get_message_drop_chance_between_points(&self, point1: Vector2, point2: Vector2) -> f64 {
        let min_transmission_range = self.properties.min_transmission_range;
        let max_transmission_range = self.properties.max_transmission_range;

        let min_transmission_range_drop_chance =
            self.properties.min_transmission_range_message_drop_chance;
        let max_transmission_range_drop_chance =
            self.properties.max_transmission_range_message_drop_chance;

        let distance = self.get_distance_between_points(point1, point2) as f64;

        templates::generic_constrained_map(
            distance,
            min_transmission_range,
            max_transmission_range,
            min_transmission_range_drop_chance,
            max_transmission_range_drop_chance,
        )
    }

    fn draw_grid(
        &self,
        d: &mut RaylibDrawHandle,
        color: Color,
        space_between_lines: f32,
        thickness: f32,
    ) {
        let window_width = d.get_screen_width();
        let window_height = d.get_screen_height();

        let zoom = self.camera_zoom as f32;
        let window_size = Vector2::new(window_width as f32, window_height as f32) / zoom;
        let half_window_size = window_size * 0.5;

        let top_left_corner = self.camera_position - half_window_size;
        let bottom_right_corner = self.camera_position + half_window_size;

        let offset = Vector2::new(
            self.camera_position.x % space_between_lines,
            self.camera_position.y % space_between_lines,
        );

        let x_start_position =
            ((top_left_corner.x - offset.x) / space_between_lines).floor() * space_between_lines;

        let mut x = x_start_position;
        while x < bottom_right_corner.x {
            let mut current_line_thickness = thickness;
            if x.abs() < 0.01 {
                current_line_thickness *= 1.5;
            }

            let position_on_screen =
                self.real_position_to_screen_position(Vector2::new(x, top_left_corner.y), d);
            d.draw_line_ex(
                position_on_screen,
                Vector2::new(position_on_screen.x, d.get_screen_height() as f32),
                current_line_thickness,
                color,
            );

            x += space_between_lines;
        }

        let y_start_position =
            ((top_left_corner.y - offset.y) / space_between_lines).floor() * space_between_lines;

        let mut y = y_start_position;
        while y < bottom_right_corner.y {
            let mut current_line_thickness = thickness;
            if y.abs() < 0.01 {
                current_line_thickness *= 1.5;
            }

            let position_on_screen =
                self.real_position_to_screen_position(Vector2::new(top_left_corner.x, y), d);
            d.draw_line_ex(
                position_on_screen,
                Vector2::new(d.get_screen_width() as f32, position_on_screen.y),
                current_line_thickness,
                color,
            );

            y += space_between_lines;
        }
    }

    fn handle_camera_movement(&mut self, d: &mut RaylibDrawHandle) {
        if self.current_dialog.is_none() {
            let v = d.get_mouse_wheel_move_v();
            if d.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
                || d.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL)
            {
                self.camera_zoom += (v.y * 0.05) as f64;
                self.camera_zoom = self.camera_zoom.max(0.1);
            } else {
                let is_shift_down = d.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
                    || d.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT);
                let effective_v = if is_shift_down {
                    Vector2::new(v.y, v.x)
                } else {
                    Vector2::new(v.x, v.y)
                };
                let scaled_vector =
                    vector2_scale(effective_v.into(), 5.0 * (1.0 / self.camera_zoom) as f32);
                self.camera_position = vector2_subtract(self.camera_position, scaled_vector);
            }
        }

        if d.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT) {
            let delta = d.get_mouse_delta();
            let scaled_vector = vector2_scale(delta, 1.0 * (1.0 / self.camera_zoom) as f32);
            self.camera_position = vector2_subtract(self.camera_position, scaled_vector);
        }
    }

    fn handle_device_creation(&mut self, d: &mut RaylibDrawHandle) {
        if d.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT) {
            let current_mouse_position = d.get_mouse_position();
            let time_since_last_release = d.get_time() - self.last_left_mouse_button_released_time;

            if time_since_last_release < 0.5
                && self.get_distance_between_points(
                    current_mouse_position,
                    self.last_left_mouse_button_released_position,
                ) < 5.0
            {
                self.left_mouse_button_released_action = Some(Box::new(|simulator, d| {
                    let current_mouse_position = d.get_mouse_position();
                    let real_position =
                        simulator.screen_position_to_real_position(current_mouse_position, d);
                    simulator.add_device("name", "model", real_position);
                }));
            }
        }
    }

    fn draw_all_device_connections(&self, d: &mut RaylibDrawHandle) {
        for device1 in &self.devices {
            for device2 in &self.devices {
                self.draw_device_connections(device1, device2, d);
            }
        }
    }

    fn draw_device_connections(
        &self,
        device1: &std::sync::Arc<Device>,
        device2: &std::sync::Arc<Device>,
        d: &mut RaylibDrawHandle,
    ) {
        assert!(
            self.device_id_to_properties
                .contains_key(&device1.get_id().try_into().unwrap())
        );
        assert!(
            self.device_id_to_properties
                .contains_key(&device2.get_id().try_into().unwrap())
        );

        let position1 = self
            .device_id_to_properties
            .get(&device1.get_id().try_into().unwrap())
            .unwrap()
            .position;
        let position2 = self
            .device_id_to_properties
            .get(&device2.get_id().try_into().unwrap())
            .unwrap()
            .position;

        let position_on_screen1 = self.real_position_to_screen_position(position1, d);
        let position_on_screen2 = self.real_position_to_screen_position(position2, d);

        let drop_chance = self.get_message_drop_chance_between_points(position1, position2);
        if drop_chance >= 1.0 {
            return;
        }

        d.draw_line_ex(
            position_on_screen1,
            position_on_screen2,
            2.0 * self.camera_zoom as f32,
            Color::new(0, 100, 255, (128.0 * (1.0 - drop_chance)) as u8),
        );
    }

    fn draw_devices(&mut self, d: &mut RaylibDrawHandle) {
        let devices = self.devices.clone();
        for device in devices {
            self.draw_device(&device, d);
        }
    }

    fn draw_device(&mut self, device: &std::sync::Arc<Device>, d: &mut RaylibDrawHandle) {
        let device_id: u32 = device.get_id().try_into().unwrap();
        let Some(device_properties) = self.device_id_to_properties.get(&device_id) else {
            return;
        };

        let position = device_properties.position;
        let position_on_screen = self.real_position_to_screen_position(position, d);

        let button_width = 32.0 * self.camera_zoom as f32;
        let button_height = 24.0 * self.camera_zoom as f32;

        let button_rect = Rectangle::new(
            position_on_screen.x - button_width / 2.0,
            position_on_screen.y - button_height / 2.0,
            button_width,
            button_height,
        );

        if self.selected_items.contains(&device_id) {
            let distance_to_button = 6.0;
            let selection_rect = Rectangle::new(
                button_rect.x - distance_to_button,
                button_rect.y - distance_to_button,
                button_rect.width + distance_to_button * 2.0,
                button_rect.height + distance_to_button * 2.0,
            );
            d.draw_rectangle_lines_ex(selection_rect, 4.0, Color::new(0, 192, 255, 255));
            self.draw_message_range_for_device(device, d);
        }

        d.draw_rectangle_rec(button_rect, Color::new(245, 245, 245, 255));
        d.draw_rectangle_lines_ex(button_rect, 1.0, Color::DARKGRAY);
        d.draw_text(
            "X",
            (button_rect.x + button_rect.width * 0.35) as i32,
            (button_rect.y + button_rect.height * 0.15) as i32,
            (16.0 * self.camera_zoom) as i32,
            Color::BLACK,
        );

        let mouse = d.get_mouse_position();
        let hovered = mouse.x >= button_rect.x
            && mouse.x <= button_rect.x + button_rect.width
            && mouse.y >= button_rect.y
            && mouse.y <= button_rect.y + button_rect.height;

        if hovered && d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            self.left_mouse_button_pressed_action = Some(Box::new(|simulator, _| {
                simulator.do_drag = true;
            }));
        }

        if hovered && d.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT) {
            let is_left_ctrl_key_down = d.is_key_down(KeyboardKey::KEY_LEFT_CONTROL);
            let is_right_ctrl_key_down = d.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL);
            self.left_mouse_button_released_action = Some(Box::new(move |simulator, _| {
                if is_left_ctrl_key_down || is_right_ctrl_key_down {
                    if simulator.selected_items.contains(&device_id) {
                        simulator.selected_items.remove(&device_id);
                    } else {
                        simulator.selected_items.insert(device_id);
                    }
                    return;
                }

                simulator.selected_items.clear();
                simulator.selected_items.insert(device_id);
            }));
        }
    }

    fn draw_message_range_for_device(
        &self,
        device: &std::sync::Arc<Device>,
        d: &mut RaylibDrawHandle,
    ) {
        let device_id: u32 = device.get_id().try_into().unwrap();
        let Some(device_properties) = self.device_id_to_properties.get(&device_id) else {
            return;
        };

        let position_on_screen =
            self.real_position_to_screen_position(device_properties.position, d);

        let min_transmission_range = self.properties.min_transmission_range;
        let max_transmission_range = self.properties.max_transmission_range;

        d.draw_ring(
            position_on_screen,
            (min_transmission_range * self.camera_zoom) as f32,
            (max_transmission_range * self.camera_zoom) as f32,
            0.0,
            360.0,
            64,
            Color::new(0, 0, 0, 16),
        );
    }

    fn get_step_size(&self, d: &mut RaylibDrawHandle) -> i32 {
        let window_time = d.get_time();
        while self.is_some_device_still_working() {
            if d.get_time() > window_time + 0.1 {
                println!("Warning: Wait for still working devices timed out!");

                return 1;
            }
        }

        let _lock = SIMULATOR_MUTEX.lock();
        if !self.messages.is_empty() {
            return 1;
        }

        32
    }

    fn get_step_skip_size(&self) -> i32 {
        let _lock = SIMULATOR_MUTEX.lock();
        if !self.messages.is_empty() {
            return 1;
        }

        4
    }

    fn is_some_device_still_working(&self) -> bool {
        for device in &self.devices {
            if device.get_heartbeats_left() > 0 {
                return true;
            }
        }

        return false;
    }

    fn step(&mut self, d: &mut RaylibDrawHandle) {
        for _i in (0..self.get_step_size(d)).step_by(self.get_step_skip_size() as usize) {
            let _lock = SIMULATOR_MUTEX.lock();
            let skip_size = self.get_step_skip_size();

            self.simulation_time
                .fetch_add(skip_size, std::sync::atomic::Ordering::Relaxed);

            self.handle_device_step(skip_size as u32);

            let skip_size = self.get_step_skip_size(); // device may have sent a message, so the step size may have changed

            self.handle_message_step(skip_size as u32);
        }
    }

    fn draw_main_ui(&mut self, d: &mut RaylibDrawHandle) {
        let screen_height = d.get_screen_height() as f32;

        let mut x = 8.0;

        if d.gui_button(
            Rectangle::new(x, screen_height - 24.0 - 8.0, 64.0, 24.0),
            "Step",
        ) {
            self.left_mouse_button_released_action = Some(Box::new(|simulator, d| {
                simulator.step(d);
            }));
        }

        x += 72.0;

        if d.gui_button(
            Rectangle::new(x, screen_height - 24.0 - 8.0, 64.0, 24.0),
            if self.is_paused { "Play" } else { "Pause" },
        ) {
            self.is_paused = !self.is_paused;
        }
    }

    fn draw_selected_device_ui(&mut self, d: &mut RaylibDrawHandle) {
        if self.selected_items.is_empty() {
            return;
        }

        for selected_item_id in &self.selected_items {
            if !self.device_id_to_properties.contains_key(selected_item_id) {
                return;
            }
        }

        let mut y = 8.0;

        if d.gui_button(Rectangle::new(8.0, y, 64.0, 24.0), "Delete") {
            self.left_mouse_button_released_action = Some(Box::new(|simulator, _| {
                let selected_items_copy: Vec<u32> =
                    simulator.selected_items.iter().copied().collect();
                for selected_item_id in selected_items_copy {
                    simulator.remove_device(selected_item_id);
                }
            }));
        }

        y += 32.0;

        if self.selected_items.len() == 1
            && d.gui_button(Rectangle::new(8.0, y, 96.0, 24.0), "Open Console")
        {
            let selected_device_id = *self.selected_items.iter().next().unwrap();

            self.left_mouse_button_released_action = Some(Box::new(move |simulator, _d| {
                let mut scroll_position_y = 0.0f32;

                simulator.current_dialog = Some(Box::new(move |simulator, d| {
                    let window_width = d.get_screen_width();
                    let window_height = d.get_screen_height();
                    let screen_size = Vector2::new(window_width as f32, window_height as f32);

                    let distance_to_edge = 32.0f32;

                    let top_left = Vector2::new(distance_to_edge, distance_to_edge);
                    let size =
                        screen_size - Vector2::new(distance_to_edge * 2.0, distance_to_edge * 2.0);
                    let bottom_right = top_left + size;

                    d.draw_rectangle_v(top_left, size, Color::new(16, 16, 16, 192));
                    d.draw_rectangle_lines_ex(
                        Rectangle::new(top_left.x, top_left.y, size.x, size.y),
                        2.0,
                        Color::new(194, 194, 194, 194),
                    );

                    d.draw_scissor_mode(
                        top_left.x as i32,
                        top_left.y as i32,
                        size.x as i32,
                        size.y as i32,
                        |mut d| {
                            if let Some(properties) =
                                simulator.get_device_properties(selected_device_id)
                            {
                                let virtual_console = &mut properties.virtual_console;

                                let pressed_key = unsafe { raylib::ffi::GetCharPressed() };
                                if (32..=125).contains(&pressed_key) {
                                    let ch = char::from_u32(pressed_key as u32).unwrap_or('\0');
                                    virtual_console.write_char_to_input_buffer(ch);
                                    virtual_console.write_char_to_output(ch, true);
                                }

                                let console_output = virtual_console.get_output();
                                let mut y = 8.0f32;

                                for line in console_output {
                                    if y - scroll_position_y >= -16.0 {
                                        let color = if line.is_input {
                                            Color::new(255, 150, 0, 255)
                                        } else {
                                            Color::new(255, 255, 255, 255)
                                        };

                                        d.draw_text(
                                            &line.line,
                                            (top_left.x + 8.0) as i32,
                                            (top_left.y + y - scroll_position_y) as i32,
                                            16,
                                            color,
                                        );
                                    }

                                    y += 16.0;
                                    if y - scroll_position_y > size.y {
                                        break;
                                    }
                                }
                            }
                        },
                    );

                    scroll_position_y =
                        (scroll_position_y - d.get_mouse_wheel_move_v().y * 5.0).max(0.0);

                    if d.gui_button(
                        Rectangle::new(bottom_right.x - 24.0, top_left.y, 24.0, 24.0),
                        "x",
                    ) {
                        simulator.current_dialog = None;
                    }
                }));
            }));
        }
    }

    fn to_hex(data: u8) -> String {
        format!("{data:02x}")
    }

    fn to_ascii(data: u8) -> String {
        match data {
            0 => "\\0".to_string(),
            b'\n' => "\\n".to_string(),
            b'\r' => "\\r".to_string(),
            b'\t' => "\\t".to_string(),
            8 => "\\b".to_string(),
            12 => "\\f".to_string(),
            11 => "\\v".to_string(),
            32..=126 => char::from(data).to_string(),
            _ => "?".to_string(),
        }
    }

    fn draw_selected_message_ui(&mut self, d: &mut RaylibDrawHandle) {
        if self.selected_items.is_empty() {
            return;
        }

        for selected_item_id in &self.selected_items {
            if !self
                .messages
                .iter()
                .any(|message| message.id == *selected_item_id as u64)
            {
                return;
            }
        }

        let mut y = 8.0;

        if d.gui_button(Rectangle::new(8.0, y, 96.0, 24.0), "Drop/Undrop") {
            self.left_mouse_button_released_action = Some(Box::new(|simulator, _| {
                for message in &mut simulator.messages {
                    if simulator.selected_items.contains(&(message.id as u32)) {
                        if let Some(message) = Arc::get_mut(message) {
                            message.will_be_dropped = !message.will_be_dropped;
                        }
                    }
                }
            }));
        }

        y += 32.0;

        if self.selected_items.len() == 1 {
            let selected_message_id = *self.selected_items.iter().next().unwrap() as u64;
            if let Some(message) = self
                .messages
                .iter()
                .find(|message| message.id == selected_message_id as u64)
            {
                let message_data = message
                    .data
                    .iter()
                    .map(|byte| Self::to_hex(*byte))
                    .collect::<Vec<_>>()
                    .join(", ");

                let message_string = format!(
                    "\"{}\"",
                    message
                        .data
                        .iter()
                        .map(|byte| Self::to_ascii(*byte))
                        .collect::<String>()
                );

                d.draw_text_ex(
                    &d.get_font_default(),
                    &message_data,
                    Vector2::new(8.0, y),
                    16.0,
                    0.0,
                    Color::new(0, 0, 0, 128),
                );
                y += 20.0;
                d.draw_text_ex(
                    &d.get_font_default(),
                    &message_string,
                    Vector2::new(8.0, y),
                    16.0,
                    0.0,
                    Color::new(0, 0, 0, 128),
                );
            }
        }
    }

    fn handle_dragging(&mut self, d: &mut RaylibDrawHandle) {
        if !self.do_drag {
            return;
        }

        if !d.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            self.do_drag = false;
            return;
        }

        let delta = d.get_mouse_delta();
        let scaled_vector = vector2_scale(delta, 1.0 / self.camera_zoom as f32);

        for item_id in &self.selected_items {
            if let Some(device_properties) = self.device_id_to_properties.get_mut(item_id) {
                device_properties.position = vector2_add(device_properties.position, scaled_vector);
            }
        }
    }

    fn draw_messages(&mut self, d: &mut RaylibDrawHandle) {
        for message in self.messages.clone() {
            self.draw_message(message, d);
        }
    }

    fn draw_message(&mut self, message: Arc<Message>, d: &mut RaylibDrawHandle) {
        let sender_id: u32 = match message.sender_id.try_into() {
            Ok(id) => id,
            Err(_) => return,
        };
        let receiver_id: u32 = match message.receiver_id.try_into() {
            Ok(id) => id,
            Err(_) => return,
        };

        let Some(sender_properties) = self.device_id_to_properties.get(&sender_id) else {
            return;
        };
        let Some(receiver_properties) = self.device_id_to_properties.get(&receiver_id) else {
            return;
        };

        let transmission_percentage = message.get_transmission_percentage() as f32;
        let real_position = sender_properties.position
            + (receiver_properties.position - sender_properties.position) * transmission_percentage;
        let screen_position = self.real_position_to_screen_position(real_position, d);

        let button_size = 16.0 * self.camera_zoom as f32;
        let button_rectangle = Rectangle::new(
            screen_position.x - button_size / 2.0,
            screen_position.y - button_size / 2.0,
            button_size,
            button_size,
        );

        if self.selected_items.contains(&(message.id as u32)) {
            let distance_to_button = 6.0;
            d.draw_rectangle_lines_ex(
                Rectangle::new(
                    button_rectangle.x - distance_to_button,
                    button_rectangle.y - distance_to_button,
                    button_rectangle.width + distance_to_button * 2.0,
                    button_rectangle.height + distance_to_button * 2.0,
                ),
                4.0,
                Color::new(0, 192, 255, 255),
            );
        }

        let message_id = message.id as u32;
        if d.gui_button(button_rectangle, "") {
            self.left_mouse_button_released_action = Some(Box::new(move |simulator, d| {
                if d.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
                    || d.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL)
                {
                    if simulator.selected_items.contains(&message_id) {
                        simulator.selected_items.remove(&message_id);
                    } else {
                        simulator.selected_items.insert(message_id);
                    }
                    return;
                }

                simulator.selected_items.clear();
                simulator.selected_items.insert(message_id);
            }));
        }

        let color_hash = (message.get_type().overflowing_mul(0xde).0 as u32 ^ 0xead) as i32;
        let hue = (color_hash.rem_euclid(360)) as f32;
        let color = Color::color_from_hsv(hue, 1.0, 1.0);

        d.draw_rectangle_lines_ex(button_rectangle, 2.0 * self.camera_zoom as f32, color);

        if message.will_be_dropped {
            d.draw_line_ex(
                Vector2::new(button_rectangle.x, button_rectangle.y),
                Vector2::new(
                    button_rectangle.x + button_rectangle.width,
                    button_rectangle.y + button_rectangle.height,
                ),
                4.0 * self.camera_zoom as f32,
                Color::RED,
            );

            d.draw_line_ex(
                Vector2::new(
                    button_rectangle.x + button_rectangle.width,
                    button_rectangle.y,
                ),
                Vector2::new(
                    button_rectangle.x,
                    button_rectangle.y + button_rectangle.height,
                ),
                4.0 * self.camera_zoom as f32,
                Color::RED,
            );
        }
    }

    fn draw_clock_hand(&self, d: &mut RaylibDrawHandle, position: Vector2, size: f32, angle: f32) {
        let tip_position = vector2_add(position, vector2_rotate(Vector2::new(size, 0.0), angle));
        d.draw_line_ex(position, tip_position, 2.0, Color::new(0, 0, 0, 94));
    }

    fn draw_clock(&self, d: &mut RaylibDrawHandle) {
        let size = 32.0f32;
        let distance_to_window_edge = 8.0f32;

        let window_width = d.get_screen_width() as f32;

        let center = Vector2::new(
            window_width - distance_to_window_edge - size * 0.5,
            distance_to_window_edge + size * 0.5,
        );

        let simulation_time = self
            .simulation_time
            .load(std::sync::atomic::Ordering::Relaxed) as f32;
        self.draw_clock_hand(
            d,
            center,
            size * 0.4,
            simulation_time % 1000.0 * (2.0 * std::f32::consts::PI / 1000.0),
        );
        self.draw_clock_hand(
            d,
            center,
            size * 0.25,
            simulation_time % 60000.0 * (2.0 * std::f32::consts::PI / 60000.0),
        );

        d.draw_circle_lines_v(center, size * 0.5, Color::new(0, 0, 0, 64));
    }

    fn draw_current_dialog(&mut self, d: &mut RaylibDrawHandle) {
        if let Some(mut dialog) = self.current_dialog.take() {
            self.current_dialog = Some(Box::new(|_simulator, _d| {}));
            dialog(self, d);
            // If the dialog wasn't closed during its execution, keep it open.
            if !self.current_dialog.is_none() {
                self.current_dialog = Some(dialog);
            }
        }
    }

    /// Main loop for the simulator.  It handles user input, updates the state
    /// of the simulator and renders the current frame.
    pub fn draw(&mut self, d: &mut RaylibDrawHandle) {
        self.set_actions_to_default_state(d);

        self.handle_camera_movement(d);
        self.handle_device_creation(d);
        self.handle_dragging(d);

        d.clear_background(Color::new(240, 240, 240, 255));
        self.draw_grid(d, Color::new(210, 210, 210, 255), 50.0, 1.0);

        let text_position = self.real_position_to_screen_position(Vector2::new(190.0, 200.0), d);
        d.draw_text(
            "CONTROLS:\nDouble click: Add device\nLMB: Select elements\nCtrl + LMB: Select multiple elements\nRMB: Pan view\nCtrl + MB3: Zoom",
            text_position.x as i32, text_position.y as i32, (20.0 * self.camera_zoom) as i32, Color::LIGHTGRAY);

        {
            let _lock = SIMULATOR_MUTEX.lock();

            self.draw_all_device_connections(d);
            self.draw_messages(d);
            self.draw_devices(d);

            self.draw_selected_device_ui(d);
            self.draw_selected_message_ui(d);

            self.draw_clock(d);
            self.draw_main_ui(d);

            self.draw_current_dialog(d);
        }

        if !self.is_paused {
            self.step(d);
        }

        self.handle_actions(d);
    }

    /// Adds a device to the simulator.
    pub fn add_device(&mut self, name: &str, model: &str, position: Vector2) {
        let device = Device::create();
        let device_id = device.get_id();

        self.device_id_to_properties.insert(
            device_id.try_into().unwrap(),
            DeviceProperties::new(position),
        );
        self.devices.push(device.clone());

        device.init(name, model, self.self_ref.clone());
    }

    /// Removes a device from the simulator.
    pub fn remove_device(&mut self, id: u32) {
        self.device_id_to_properties.remove(&id);
        self.devices.retain(|device| {
            if device.get_id() == id.into() {
                device.destroy();
                false
            } else {
                true
            }
        });
        self.selected_items.remove(&id);
    }

    /// Gets the properties for a device.
    pub fn get_device_properties(&mut self, id: u32) -> Option<&mut DeviceProperties> {
        self.device_id_to_properties.get_mut(&id)
    }

    /// Checks if a device with the given id is present in the simulator.
    pub fn is_device_id_present(&self, id: u32) -> bool {
        self.device_id_to_properties.contains_key(&id)
    }

    /// Gets a vector of all device ids in the simulator.
    pub fn get_all_device_ids(&self) -> Vec<u32> {
        self.device_id_to_properties.keys().copied().collect()
    }

    /// Sends a message to the simulator.
    pub fn send_message(&mut self, message: Arc<Message>) {
        self.messages.push(message);
    }

    /// Gets the mutex for the simulator. The mutex is used to protect the
    /// simulator state from concurrent access.
    pub fn get_mutex(&self) -> &parking_lot::ReentrantMutex<()> {
        &SIMULATOR_MUTEX
    }

    /// Registers a callback for messages sent to the device with the given id.
    pub fn register_message_callback(
        &mut self,
        device_id: u32,
        callback: impl Fn(Arc<Message>) + Send + Sync + 'static,
    ) {
        let props = self
            .device_id_to_properties
            .get_mut(&device_id)
            .expect("device not found");
        props.on_message_received = Some(Arc::new(callback));
    }

    /// Gets the properties of the simulator.
    ///
    /// The properties of the simulator are used to customize the behavior of the simulator.
    pub fn get_simulator_properties(&mut self) -> &mut simulator_properties::SimulatorProperties {
        &mut self.properties
    }

    /// Calculates the transmission quality at the given position.
    ///
    /// The transmission quality is a value between 0.0 and 1.0 that represents the
    /// quality of the transmission in the simulated space. The quality is based on
    /// the position of the device in the simulated space and the current
    /// simulation time. It is used to determine the likelihood of a message being
    /// received by a device.
    pub fn get_transmission_quality(&self, position: Vector2) -> f64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let rounded_x = (position.x / 128.0).round() as i32;
        let rounded_y = (position.y / 128.0).round() as i32;
        let rounded_time =
            (self.simulation_time.load(Ordering::Relaxed) as f64 / 128.0).round() as i64;

        let mut hasher = DefaultHasher::new();
        rounded_time.hash(&mut hasher);
        rounded_x.hash(&mut hasher);
        rounded_y.hash(&mut hasher);
        let hash = hasher.finish();

        // Map hash to [0.0, 1.0)
        (hash as f64) / (u64::MAX as f64)
    }

    /// Calculates whether a message should be dropped when sent from `point1` to `point2`.
    ///
    /// Returns `true` if the message should be dropped, `false` otherwise.
    pub fn do_drop_message(&self, point1: Vector2, point2: Vector2) -> bool {
        let quality1 = self.get_transmission_quality(point1);
        let quality2 = self.get_transmission_quality(point2);
        let quality = (quality1 + quality2) / 2.0;
        let drop_chance = self.get_message_drop_chance_between_points(point1, point2);
        quality < drop_chance
    }

    /// Gets all devices within transmission range of the given point.
    pub fn get_devices_in_range(&self, point: Vector2) -> Vec<Arc<Device>> {
        self.devices
            .iter()
            .filter(|device| {
                let device_id = device.get_id() as u32;
                let device_position = self.device_id_to_properties[&device_id].position;
                let drop_chance =
                    self.get_message_drop_chance_between_points(point, device_position);
                drop_chance < 0.999_999_99
            })
            .map(|device| Arc::clone(device))
            .collect()
    }
}
