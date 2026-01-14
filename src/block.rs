use crate::{proof_of_work::ProofOfWork, transaction::Transaction, utils};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sled::IVec;

#[derive(Clone, Deserialize, Serialize)]
pub struct Block {
    timestamp: i64,
    pre_block_hash: String,
    hash: String,
    transactions: Vec<Transaction>,
    nonce: i64,
    height: usize,
}

impl Block {
    pub fn generate_genesis_block(transaction: &Transaction) -> Self {
        let transactions = vec![transaction.clone()];

        Self::new_block(String::from("None"), &transactions, 0)
    }

    pub fn new_block(pre_block_hash: String, transactions: &[Transaction], height: usize) -> Self {
        let mut block = Block {
            timestamp: utils::current_timestamp(),
            pre_block_hash,
            hash: String::new(),
            transactions: transactions.to_vec(),
            nonce: 0,
            height,
        };

        let pow = ProofOfWork::new_proof_of_work(block.clone());
        let (nonce, hash) = pow.run();
        block.nonce = nonce;
        block.hash = hash;

        block
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(bincode::deserialize::<Self>(bytes)?)
    }

    pub fn get_timestamp(&self) -> i64 {
        self.timestamp
    }

    pub fn get_pre_block_hash(&self) -> String {
        self.pre_block_hash.clone()
    }

    pub fn get_hash(&self) -> &str {
        self.hash.as_str()
    }

    pub fn get_hash_bytes(&self) -> Vec<u8> {
        self.hash.as_bytes().to_vec()
    }

    pub fn hash_transactions(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for transaction in &self.transactions {
            data.extend(transaction.get_id());
        }

        utils::sha256_digest(data.as_slice())
    }

    pub fn get_transactions(&self) -> &[Transaction] {
        self.transactions.as_slice()
    }

    pub fn get_height(&self) -> usize {
        self.height
    }
}

impl From<&Block> for IVec {
    fn from(value: &Block) -> Self {
        let bytes = bincode::serialize(value).expect("Block serialization failed");
        Self::from(bytes)
    }
}
