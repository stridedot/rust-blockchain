use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    blockchain::Blockchain,
    utils,
    utxo_set::UTXOSet,
    wallets::{self, Wallets},
};

const SUBSIDY: i32 = 10;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TXInput {
    txid: Vec<u8>,
    vout: usize,
    signature: Vec<u8>,
    pub_key: Vec<u8>,
}

impl TXInput {
    pub fn new(txid: &[u8], vout: usize) -> Self {
        TXInput {
            txid: txid.to_vec(),
            vout,
            signature: vec![],
            pub_key: vec![],
        }
    }

    pub fn get_txid(&self) -> &[u8] {
        self.txid.as_slice()
    }

    pub fn get_vout(&self) -> usize {
        self.vout
    }

    pub fn get_pub_key(&self) -> &[u8] {
        self.pub_key.as_slice()
    }

    pub fn use_key(&self, pub_key: &[u8]) -> bool {
        let locking_hash = wallets::hash_pub_key(self.pub_key.as_slice());
        locking_hash == pub_key.to_vec()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TXOutput {
    value: i32,
    pub_key_hash: Vec<u8>,
}

impl TXOutput {
    pub fn new(value: i32, address: &str) -> Self {
        let mut output = TXOutput {
            value,
            pub_key_hash: vec![],
        };
        output.lock(address);
        output
    }

    fn lock(&mut self, address: &str) {
        let payload = utils::base58_decode(address);
        let pub_key_hash = payload[1..payload.len() - wallets::ADDRESS_CHECK_SUM_LEN].to_vec();
        self.pub_key_hash = pub_key_hash;
    }

    pub fn get_value(&self) -> i32 {
        self.value
    }

    pub fn get_pub_key_hash(&self) -> &[u8] {
        self.pub_key_hash.as_slice()
    }

    pub fn is_locked_with_key(&self, pub_key_hash: &[u8]) -> bool {
        self.pub_key_hash.eq(pub_key_hash)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Transaction {
    id: Vec<u8>,
    vin: Vec<TXInput>,
    vout: Vec<TXOutput>,
}

impl Transaction {
    pub fn new_coinbase_tx(to: &str) -> Result<Self> {
        let txout = TXOutput::new(SUBSIDY, to);
        let mut tx_input = TXInput::default();
        tx_input.signature = Uuid::new_v4().as_bytes().to_vec();

        let mut tx = Transaction {
            id: vec![],
            vin: vec![tx_input],
            vout: vec![txout],
        };
        tx.id = tx.hash()?;

        Ok(tx)
    }

    fn hash(&mut self) -> Result<Vec<u8>> {
        let tx_copy = Transaction {
            id: vec![],
            vin: self.vin.clone(),
            vout: self.vout.clone(),
        };

        Ok(utils::sha256_digest(tx_copy.serialize()?.as_slice()))
    }

    pub fn new_utxo_transaction(
        from: &str,
        to: &str,
        amount: i32,
        utxo_set: &UTXOSet,
    ) -> Result<Self> {
        let all_wallets = Wallets::try_new()?;
        let wallet = all_wallets
            .get_wallet(from)
            .ok_or(anyhow::anyhow!("ERROR: Wallet not found"))?;
        let public_key_hash = wallets::hash_pub_key(wallet.get_public_key());

        let (accumulated, valid_outputs) =
            utxo_set.find_spendable_outputs(public_key_hash.as_slice(), amount)?;

        if accumulated < amount {
            return Err(anyhow::anyhow!("ERROR: Not enough funds"));
        }

        let mut inputs = vec![];

        for (txid_hex, outs) in valid_outputs {
            let txid = data_encoding::HEXLOWER.decode(txid_hex.as_bytes())?;
            for out in outs {
                let input = TXInput {
                    txid: txid.clone(),
                    vout: out,
                    signature: vec![],
                    pub_key: wallet.get_public_key().to_vec(),
                };
                inputs.push(input);
            }
        }

        let mut outputs = vec![TXOutput::new(amount, to)];

        if accumulated > amount {
            outputs.push(TXOutput::new(accumulated - amount, from)) // to: 币收入
        }

        let mut tx = Transaction {
            id: vec![],
            vin: inputs,
            vout: outputs,
        };

        tx.id = tx.hash()?;

        tx.sign(utxo_set.get_blockchain(), wallet.get_pkcs8())?;

        Ok(tx)
    }

    fn trimmed_copy(&self) -> Self {
        let mut inputs = vec![];
        let mut outputs = vec![];

        for input in &self.vin {
            let txinput = TXInput::new(input.get_txid(), input.get_vout());
            inputs.push(txinput);
        }

        for output in &self.vout {
            outputs.push(output.clone());
        }

        Transaction {
            id: self.id.clone(),
            vin: inputs,
            vout: outputs,
        }
    }

    fn sign(&mut self, blockchain: &Blockchain, pkcs8: &[u8]) -> Result<()> {
        let mut tx_copy = self.trimmed_copy();

        for (idx, vin) in self.vin.iter_mut().enumerate() {
            let prev_tx_option = blockchain.find_transaction(vin.get_txid());
            if prev_tx_option.is_none() {
                return Err(anyhow::anyhow!(
                    "ERROR: Previous transaction is not correct"
                ));
            }
            let prev_tx = prev_tx_option.unwrap();
            tx_copy.vin[idx].signature = vec![];
            tx_copy.vin[idx].pub_key = prev_tx.vout[vin.vout].pub_key_hash.clone();
            tx_copy.id = tx_copy.hash()?;
            tx_copy.vin[idx].pub_key = vec![];

            let signature = utils::ecdsa_p256_sha256_sign_digest(pkcs8, tx_copy.get_id());
            vin.signature = signature;
        }

        Ok(())
    }

    pub fn verify(&self, blockchain: &Blockchain) -> Result<bool> {
        if self.is_coinbase() {
            return Ok(true);
        }
        let mut tx_copy = self.trimmed_copy();
        for (idx, vin) in self.vin.iter().enumerate() {
            let prev_tx_option = blockchain.find_transaction(vin.get_txid());
            if prev_tx_option.is_none() {
                panic!("ERROR: Previous transaction is not correct")
            }
            let prev_tx = prev_tx_option.unwrap();
            tx_copy.vin[idx].signature = vec![];
            tx_copy.vin[idx].pub_key = prev_tx.vout[vin.vout].pub_key_hash.clone();
            tx_copy.id = tx_copy.hash()?;
            tx_copy.vin[idx].pub_key = vec![];

            let verify = utils::ecdsa_p256_sha256_sign_verify(
                vin.pub_key.as_slice(),
                vin.signature.as_slice(),
                tx_copy.get_id(),
            );
            if !verify {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn is_coinbase(&self) -> bool {
        return self.vin.len() == 1 && self.vin[0].pub_key.len() == 0;
    }

    pub fn get_id(&self) -> &[u8] {
        self.id.as_slice()
    }

    pub fn get_id_bytes(&self) -> Vec<u8> {
        self.id.clone()
    }

    pub fn get_vin(&self) -> &[TXInput] {
        self.vin.as_slice()
    }

    pub fn get_vout(&self) -> &[TXOutput] {
        self.vout.as_slice()
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?.to_vec())
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        Ok(bincode::deserialize(data)?)
    }
}
