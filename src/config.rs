use anyhow::Result;
use std::{collections::HashMap, env, sync::RwLock};

use once_cell::sync::Lazy;

pub static GLOBAL_CONFIG: Lazy<Config> = Lazy::new(Config::new);

pub static DEFAULT_NODE_ADDR: &str = "127.0.0.1:2001";

const NODE_ADDRESS_KEY: &str = "NODE_ADDRESS";
const MINING_ADDRESS_KEY: &str = "MINING_ADDRESS";

pub struct Config {
    inner: RwLock<HashMap<String, String>>,
}

impl Config {
    pub fn new() -> Self {
        let mut node_addr = String::from(DEFAULT_NODE_ADDR);
        if let Ok(addr) = env::var(NODE_ADDRESS_KEY) {
            node_addr = addr;
        }
        let mut map = HashMap::new();
        map.insert(String::from(NODE_ADDRESS_KEY), node_addr);

        Config {
            inner: RwLock::new(map),
        }
    }

    pub fn get_node_addr(&self) -> Result<Option<String>> {
        let inner = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read addr: {:?}", e))?;
        Ok(inner.get(NODE_ADDRESS_KEY).cloned())
    }

    pub fn set_mining_addr(&self, addr: String) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write addr: {:?}", e))?;
        inner.insert(MINING_ADDRESS_KEY.to_string(), addr);
        Ok(())
    }

    pub fn get_mining_addr(&self) -> Result<Option<String>> {
        let inner = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read addr: {:?}", e))?;
        Ok(inner.get(MINING_ADDRESS_KEY).cloned())
    }

    pub fn is_miner(&self) -> Result<bool> {
        let inner = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read addr: {:?}", e))?;
        Ok(inner.contains_key(MINING_ADDRESS_KEY))
    }
}
