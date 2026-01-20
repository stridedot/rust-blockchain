use anyhow::Result;
use data_encoding;
use std::{
    collections::HashMap,
    env,
    sync::{Arc, RwLock},
};

use sled::{Db, Tree};

use crate::{
    block::Block,
    transaction::{TXOutput, Transaction},
};

const TIP_BLOCK_HASH_KEY: &str = "tip_block_hash";
const BLOCKS_TREE: &str = "blocks";

#[derive(Clone)]
pub struct Blockchain {
    tip_hash: Arc<RwLock<String>>,
    db: Db,
}

impl Blockchain {
    pub fn create_blockchain(genesis_address: &str) -> Result<Self> {
        let dir = env::current_dir()?;
        let db = sled::open(dir.join("data"))?;
        let blocks_tree = db.open_tree(BLOCKS_TREE)?;

        let tip_hash = match blocks_tree.get(TIP_BLOCK_HASH_KEY)? {
            Some(value) => String::from_utf8(value.to_vec())?,
            None => {
                let coinbase_tx = Transaction::new_coinbase_tx(genesis_address)?;
                let block = Block::generate_genesis_block(&coinbase_tx);
                Self::update_blocks_tree(&blocks_tree, &block)?;

                String::from(block.get_hash())
            }
        };

        let blockchain = Self {
            tip_hash: Arc::new(RwLock::new(tip_hash)),
            db,
        };

        Ok(blockchain)
    }

    fn update_blocks_tree(blocks_tree: &Tree, block: &Block) -> Result<()> {
        let block_hash = block.get_hash();
        let _ = blocks_tree.transaction::<_, (), ()>(|tx| {
            tx.insert(block_hash, block)?;
            tx.insert(TIP_BLOCK_HASH_KEY, block_hash)?;
            Ok(())
        });

        Ok(())
    }

    pub fn new_blockchain() -> Result<Self> {
        let dir = env::current_dir()?;
        let db = sled::open(dir.join("data"))?;
        let blocks_tree = db.open_tree(BLOCKS_TREE)?;

        let tip_bytes = blocks_tree
            .get(TIP_BLOCK_HASH_KEY)?
            .expect("No existing blockchain found. Create one first.");
        let tip_hash = String::from_utf8(tip_bytes.to_vec())?;

        let blockchain = Blockchain {
            tip_hash: Arc::new(RwLock::new(tip_hash)),
            db,
        };

        Ok(blockchain)
    }

    pub fn get_db(&self) -> &Db {
        &self.db
    }

    pub fn get_tip_hash(&self) -> String {
        let hash = self.tip_hash.read().expect("RwLock poisoned");

        hash.clone()
    }

    pub fn set_tip_hash(&self, new_tip_hash: &str) {
        let mut tip_hash = self.tip_hash.write().expect("RwLock poisoned");
        *tip_hash = String::from(new_tip_hash);
    }

    pub fn iterator(&self) -> BlockchainIterator {
        BlockchainIterator::new(self.get_tip_hash(), self.db.clone())
    }

    pub fn get_best_height(&self) -> Result<usize> {
        let blocks_tree = self.db.open_tree(BLOCKS_TREE)?;
        let tip_bytes = blocks_tree
            .get(self.get_tip_hash())?
            .expect("The tip hash is invalid.");
        let block = Block::deserialize(tip_bytes.as_ref())?;

        Ok(block.get_height())
    }

    pub fn mine_block(&self, transactions: &[Transaction]) -> Result<Block> {
        for transaction in transactions {
            if !transaction.verify(self)? {
                panic!("error: invalid transaction")
            }
        }

        let best_height = self.get_best_height()?;
        let block = Block::new_block(self.get_tip_hash(), transactions, best_height + 1);

        let blocks_tree = self.db.open_tree(BLOCKS_TREE)?;
        Self::update_blocks_tree(&blocks_tree, &block)?;

        let block_hash = block.get_hash();
        self.set_tip_hash(block_hash);

        Ok(block)
    }

    pub fn add_block(&self, block: &Block) -> Result<()> {
        let blocks_tree = self.db.open_tree(BLOCKS_TREE)?;

        // 1. 检查是否已存在
        if blocks_tree.get(block.get_hash())?.is_some() {
            return Ok(());
        }

        let block_bytes = block.serialize()?;
        let block_hash = block.get_hash().to_string();

        // 2. 事务操作
        blocks_tree
            .transaction::<_, _, anyhow::Error>(|tx| {
                // 存入块数据
                tx.insert(block.get_hash(), block_bytes.as_slice())?;

                // 获取当前 Tip 进行对比
                let tip_hash = self.get_tip_hash();
                if let Some(tip_bytes) = tx.get(&tip_hash)? {
                    let tip_block = Block::deserialize(tip_bytes.as_ref())
                        .map_err(sled::transaction::ConflictableTransactionError::Abort)?;

                    if block.get_height() > tip_block.get_height() {
                        tx.insert(TIP_BLOCK_HASH_KEY, block.get_hash())?;
                    }
                }
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!("Transaction error: {}", e))?;

        // 3. 只有事务成功了，才更新内存
        // 这里需要根据逻辑判断是否真的需要更新内存中的 tip
        self.set_tip_hash(&block_hash);

        Ok(())
    }

    pub fn get_block(&self, block_hash: &[u8]) -> Result<Option<Block>> {
        let blocks_tree = self.db.open_tree(BLOCKS_TREE)?;
        if let Some(block_bytes) = blocks_tree.get(block_hash)? {
            let block = Block::deserialize(&block_bytes)?;
            return Ok(Some(block));
        }

        Ok(None)
    }

    pub fn get_block_hashes(&self) -> Vec<Vec<u8>> {
        let mut data = Vec::new();
        let mut iterator = self.iterator();

        while let Ok(Some(block)) = iterator.next() {
            data.push(block.get_hash_bytes());
        }

        data
    }

    pub fn find_utxo(&self) -> HashMap<String, Vec<TXOutput>> {
        let mut utxo: HashMap<String, Vec<TXOutput>> = HashMap::new();
        let mut spent_utxo: HashMap<String, Vec<usize>> = HashMap::new();
        let mut iterator = self.iterator();

        while let Ok(Some(block)) = iterator.next() {
            'outer: for tx in block.get_transactions() {
                let txid_hex = data_encoding::HEXLOWER.encode(tx.get_id());

                for (idx, out) in tx.get_vout().iter().enumerate() {
                    if let Some(outs) = spent_utxo.get(&txid_hex)
                        && outs.contains(&idx)
                    {
                        continue 'outer;
                    }

                    utxo.entry(txid_hex.clone()).or_default().push(out.clone());
                }

                if tx.is_coinbase() {
                    continue;
                }

                for vin in tx.get_vin() {
                    let txid_hex = data_encoding::HEXLOWER.encode(vin.get_txid());
                    spent_utxo.entry(txid_hex).or_default().push(vin.get_vout());
                }
            }
        }

        utxo
    }

    pub fn find_transaction(&self, txid: &[u8]) -> Option<Transaction> {
        let mut iterator = self.iterator();

        while let Ok(Some(block)) = iterator.next() {
            for transaction in block.get_transactions() {
                if txid.eq(transaction.get_id()) {
                    return Some(transaction.clone());
                }
            }
        }

        None
    }
}

pub struct BlockchainIterator {
    db: Db,
    current_hash: String,
}

impl BlockchainIterator {
    fn new(tip_hash: String, db: Db) -> Self {
        Self {
            db,
            current_hash: tip_hash,
        }
    }

    pub fn next(&mut self) -> Result<Option<Block>> {
        let block_tree = self.db.open_tree(BLOCKS_TREE)?;
        let data = block_tree.get(self.current_hash.clone())?;

        match data {
            Some(value) => {
                let block = Block::deserialize(value.to_vec().as_slice())?;
                self.current_hash = block.get_pre_block_hash().clone();
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }
}
