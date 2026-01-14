use anyhow::Result;
use std::collections::HashMap;

use crate::{block::Block, blockchain::Blockchain, transaction::TXOutput};

const UTXO_TREE: &str = "chainstate";

pub struct UTXOSet {
    blockchain: Blockchain,
}

impl UTXOSet {
    pub fn new(blockchain: Blockchain) -> Self {
        Self { blockchain }
    }

    pub fn get_blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    pub fn find_spendable_outputs(
        &self,
        pub_key_hash: &[u8],
        amount: i32,
    ) -> Result<(i32, HashMap<String, Vec<usize>>)> {
        let mut unspent_outputs: HashMap<String, Vec<usize>> = HashMap::new();
        let mut accumulated = 0;
        let db = self.blockchain.get_db();
        let utxo_tree = db.open_tree(UTXO_TREE)?;

        for item in utxo_tree.iter() {
            let (k, v) = item?;
            let txid_hex = data_encoding::HEXLOWER.encode(k.to_vec().as_slice());
            let outs: Vec<TXOutput> = bincode::deserialize(v.to_vec().as_slice())?;

            for (vout_idx, vout) in outs.iter().enumerate() {
                if vout.is_locked_with_key(pub_key_hash) && accumulated < amount {
                    accumulated += vout.get_value();
                    unspent_outputs
                        .entry(txid_hex.clone())
                        .or_default()
                        .push(vout_idx);
                }
            }
        }

        Ok((accumulated, unspent_outputs))
    }

    pub fn find_utxo(&self, pub_key_hash: &[u8]) -> Result<Vec<TXOutput>> {
        let mut utxos: Vec<TXOutput> = Vec::new();
        let db = self.blockchain.get_db();
        let utxo_tree = db.open_tree(UTXO_TREE)?;

        for item in utxo_tree.iter() {
            let (_, v) = item?;
            let outs: Vec<TXOutput> = bincode::deserialize(v.to_vec().as_slice())?;

            for vout in outs.iter() {
                if vout.is_locked_with_key(pub_key_hash) {
                    utxos.push(vout.clone());
                }
            }
        }

        Ok(utxos)
    }

    pub fn count_transactions(&self) -> Result<usize> {
        let db = self.blockchain.get_db();
        let utxo_tree = db.open_tree(UTXO_TREE)?;

        Ok(utxo_tree.len())
    }

    pub fn reindex(&self) -> Result<()> {
        let db = self.blockchain.get_db();
        let utxo_tree = db.open_tree(UTXO_TREE)?;
        utxo_tree.clear()?;
        let utxo_map = self.blockchain.find_utxo();

        for (txid_hex, outs) in &utxo_map {
            let txid = data_encoding::HEXLOWER.decode(txid_hex.as_bytes())?;
            let value = bincode::serialize(outs)?;
            utxo_tree.insert(txid.as_slice(), value.as_slice())?;
        }

        Ok(())
    }

    pub fn update(&self, block: &Block) -> Result<()> {
        let db = self.blockchain.get_db();
        let utxo_tree = db.open_tree(UTXO_TREE)?;

        for tx in block.get_transactions() {
            if !tx.is_coinbase() {
                for vin in tx.get_vin() {
                    let mut updated_outs = Vec::new();
                    let outs_bytes = utxo_tree
                        .get(vin.get_txid())?
                        .ok_or(anyhow::anyhow!("UTXO not found"))?;
                    let outs: Vec<TXOutput> = bincode::deserialize(outs_bytes.as_ref())?;

                    for (idx, out) in outs.iter().enumerate() {
                        if idx != vin.get_vout() {
                            updated_outs.push(out.clone());
                        }
                    }

                    if updated_outs.len() > 0 {
                        let outs_bytes = bincode::serialize(&updated_outs)?;
                        utxo_tree.insert(vin.get_txid(), outs_bytes.as_slice())?;
                    } else {
                        utxo_tree.remove(vin.get_txid())?;
                    }
                }
            }

            let mut new_outputs = Vec::new();
            for vout in tx.get_vout() {
                new_outputs.push(vout.clone());
            }

            let outs_bytes = bincode::serialize(&new_outputs)?;
            let txid_hex = data_encoding::HEXLOWER.encode(tx.get_id());
            utxo_tree.insert(txid_hex.as_bytes(), outs_bytes.as_slice())?;
        }

        Ok(())
    }
}
