use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Write},
};

use crate::utils;

const VERSION: u8 = 0x00;
pub const ADDRESS_CHECK_SUM_LEN: usize = 4;

#[derive(Debug, Deserialize, Serialize)]
pub struct Wallet {
    pkcs8: Vec<u8>,
    public_key: Vec<u8>,
}

impl Wallet {
    pub fn try_new() -> Result<Self> {
        let (pkcs8, public_key) = utils::new_key_pair().map_err(|e| anyhow::anyhow!(e))?;

        Ok(Wallet { pkcs8, public_key })
    }

    pub fn get_address(&self) -> String {
        let mut address = vec![VERSION];

        let pub_key_hash = hash_pub_key(self.public_key.as_slice());
        address.extend_from_slice(pub_key_hash.as_slice());

        let checksum = checksum(pub_key_hash.as_slice());
        address.extend_from_slice(checksum.as_slice());

        // version + pub_key_hash + checksum
        utils::base58_encode(address.as_slice())
    }

    pub fn get_public_key(&self) -> &[u8] {
        self.public_key.as_slice()
    }

    pub fn get_pkcs8(&self) -> &[u8] {
        &self.pkcs8.as_slice()
    }
}

pub fn hash_pub_key(pub_key: &[u8]) -> Vec<u8> {
    let pub_key_sha256: Vec<u8> = utils::sha256_digest(pub_key);
    utils::ripemd160_digest(pub_key_sha256.as_slice())
}

fn checksum(payload: &[u8]) -> Vec<u8> {
    let first_sha = utils::sha256_digest(payload);
    let second_sha = utils::sha256_digest(first_sha.as_slice());
    second_sha[0..ADDRESS_CHECK_SUM_LEN].to_vec()
}

pub fn validate_address(address: &str) -> bool {
    let decoded = utils::base58_decode(address);
    if decoded.is_empty() {
        return false;
    }

    let address_len = decoded.len();

    let version = decoded[0];
    if version != VERSION {
        return false;
    }

    let pub_key_hash = &decoded[1..address_len - ADDRESS_CHECK_SUM_LEN];
    let actual_checksum = &decoded[address_len - ADDRESS_CHECK_SUM_LEN..];

    let mut target = vec![version];
    target.extend_from_slice(pub_key_hash);

    let expected_checksum = checksum(target.as_slice());

    expected_checksum == actual_checksum
}

pub fn convert_address(pub_key_hash: &[u8]) -> String {
    let mut address = vec![VERSION];
    address.extend_from_slice(pub_key_hash);

    let checksum = checksum(pub_key_hash);
    address.extend_from_slice(checksum.as_slice());

    utils::base58_encode(address.as_slice())
}

pub const WALLET_FILE: &str = "wallet.dat";

#[derive(Debug, Deserialize, Serialize)]
pub struct Wallets {
    wallets: HashMap<String, Wallet>,
}

impl Wallets {
    pub fn try_new() -> Result<Self> {
        let mut wallets = Wallets {
            wallets: HashMap::new(),
        };
        wallets.load_from_file()?;

        Ok(wallets)
    }

    fn load_from_file(&mut self) -> Result<()> {
        let path = env::current_dir()?.join(WALLET_FILE);
        if !path.exists() {
            return Ok(());
        }

        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let mut buf = vec![0u8; metadata.len() as usize];
        file.read(&mut buf)?;

        let wallets = bincode::deserialize(&buf)?;
        self.wallets = wallets;

        Ok(())
    }

    pub fn create_wallet(&mut self) -> Result<String> {
        let wallet = Wallet::try_new()?;
        let address = wallet.get_address();
        self.wallets.insert(address.clone(), wallet);

        self.save_to_file()?;

        Ok(address)
    }

    fn save_to_file(&self) -> Result<()> {
        let path = env::current_dir()?.join(WALLET_FILE);
        let file = OpenOptions::new().create(true).write(true).open(&path)?;
        let mut writer = BufWriter::new(file);

        let buf = bincode::serialize(&self.wallets)?;
        writer.write(buf.as_slice())?;
        writer.flush()?;

        Ok(())
    }

    pub fn get_wallet(&self, address: &str) -> Option<&Wallet> {
        self.wallets.get(address)
    }

    pub fn get_addresses(&self) -> Vec<String> {
        self.wallets.keys().cloned().collect()
    }
}
