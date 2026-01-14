use anyhow::Result;
use std::{collections::HashMap, sync::RwLock};

use crate::transaction::Transaction;

pub struct MemoryPool {
    inner: RwLock<HashMap<String, Transaction>>,
}

impl MemoryPool {
    pub fn new() -> Self {
        MemoryPool {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn contains(&self, txid_hex: &str) -> Result<bool> {
        let inner = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read from MemoryPool: {:?}", e))?;
        Ok(inner.contains_key(txid_hex))
    }

    pub fn add(&self, tx: Transaction) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write to MemoryPool: {:?}", e))?;
        let txid_hex = data_encoding::HEXLOWER.encode(tx.get_id());
        inner.insert(txid_hex, tx);
        Ok(())
    }

    pub fn get(&self, txid_hex: &str) -> Result<Option<Transaction>> {
        let inner = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read from MemoryPool: {:?}", e))?;
        Ok(inner.get(txid_hex).cloned())
    }

    pub fn remove(&self, txid_hex: &str) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write to MemoryPool: {:?}", e))?;
        inner.remove(txid_hex);
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<Transaction>> {
        // 1. 使用 ? 处理锁中毒错误
        // 注意：RwLock 的错误处理比较特殊，需要 map_err 转换一下
        let inner = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("Poisoned lock: {}", e))?;

        // 2. 正常执行逻辑
        Ok(inner.values().cloned().collect())
    }

    pub fn len(&self) -> Result<usize> {
        let len = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read from MemoryPool: {:?}", e))?
            .len();
        Ok(len)
    }
}

pub struct BlockInTransit {
    inner: RwLock<Vec<Vec<u8>>>,
}

impl BlockInTransit {
    pub fn new() -> Self {
        BlockInTransit {
            inner: RwLock::new(vec![]),
        }
    }
    pub fn add_blocks(&self, blocks: &[Vec<u8>]) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write to BlockInTransit: {:?}", e))?;

        for hash in blocks {
            inner.push(hash.to_vec());
        }

        Ok(())
    }

    pub fn len(&self) -> Result<usize> {
        let len = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read from BlockInTransit: {:?}", e))?
            .len();
        Ok(len)
    }

    pub fn first(&self) -> Result<Option<Vec<u8>>> {
        let first = self
            .inner
            .read()
            .map_err(|e| anyhow::anyhow!("failed to read from BlockInTransit: {:?}", e))?
            .first()
            .cloned();
        Ok(first)
    }

    pub fn remove(&self, block_hash: &[u8]) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write to BlockInTransit: {:?}", e))?;
        if let Some(idx) = inner.iter().position(|x| x == block_hash) {
            inner.remove(idx);
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write to BlockInTransit: {:?}", e))?;
        inner.clear();
        Ok(())
    }
}
