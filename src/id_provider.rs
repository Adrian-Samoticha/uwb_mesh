use std::sync::{LazyLock, Mutex};

pub struct IdProvider {
    next_id: u64,
}

impl IdProvider {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    pub fn get_next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

static ID_PROVIDER: LazyLock<Mutex<IdProvider>> = LazyLock::new(|| Mutex::new(IdProvider::new()));

pub fn get_next_id() -> u64 {
    let mut provider = ID_PROVIDER.lock().unwrap();
    provider.get_next_id()
}
