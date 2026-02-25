use hotshot_core::config::Config;
use hotshot_core::storage::Storage;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<Config>,
    pub storage: Mutex<Storage>,
}

impl AppState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::load_or_create()?;
        let storage = Storage::new(config.clone());
        Ok(Self {
            config: Mutex::new(config),
            storage: Mutex::new(storage),
        })
    }
}
