use raylib::prelude::*;

mod device;
mod device_properties;
mod id_provider;
mod message;
mod raymath;
mod simulator;
mod simulator_properties;
mod templates;
mod virtual_console;
mod interfaces {
    pub mod hardware_interface;
    pub mod hardware_interface_simulator_impl;
    mod message_queue;
}

fn main() {
    let simulator = simulator::Simulator::new();

    let (mut rl, thread) = raylib::init()
        .size(800, 600)
        .title("UWB Mesh Simulator")
        .resizable()
        .vsync()
        .build();

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        if let Ok(mut sim) = simulator.lock() {
            sim.draw(&mut d);
        }
    }
}
