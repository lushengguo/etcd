use jsonrpc_core::{Error, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

#[rpc]
pub trait EtcdRpc {
    #[rpc(name = "set")]
    fn set(&self, key: String, value: String) -> Result<KeyValue>;

    #[rpc(name = "get")]
    fn get(&self, key: String) -> Result<KeyValue>;

    #[rpc(name = "del")]
    fn del(&self, key: String) -> Result<KeyValue>;
}

pub struct EtcdRpcImpl {
    store: Arc<RwLock<HashMap<String, String>>>,
}

impl EtcdRpcImpl {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl EtcdRpc for EtcdRpcImpl {
    fn set(&self, key: String, value: String) -> Result<KeyValue> {
        let mut store = self.store.write().unwrap();
        store.insert(key.clone(), value.clone());
        
        Ok(KeyValue { key, value })
    }

    fn get(&self, key: String) -> Result<KeyValue> {
        let store = self.store.read().unwrap();
        
        match store.get(&key) {
            Some(value) => Ok(KeyValue {
                key,
                value: value.clone(),
            }),
            None => Err(Error::invalid_params("Key not found")),
        }
    }

    fn del(&self, key: String) -> Result<KeyValue> {
        let mut store = self.store.write().unwrap();
        
        match store.remove(&key) {
            Some(value) => Ok(KeyValue {
                key,
                value,
            }),
            None => Err(Error::invalid_params("Key not found")),
        }
    }
} 