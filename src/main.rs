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

        let guard = simulator.lock();
        // SAFETY: The ReentrantMutex guarantees exclusive access at all times.
        // UnsafeCell::get() is the only legal way to obtain *mut T from a shared
        // reference; the resulting &mut Simulator is valid while the guard is live.
        let sim = unsafe { &mut *guard.get() };
        sim.draw(&mut d);
    }
}
