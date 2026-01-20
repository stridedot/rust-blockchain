use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;
use std::{
    error::Error,
    io::{BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
    time::Duration,
};

use crate::{
    block::Block,
    blockchain::Blockchain,
    config::GLOBAL_CONFIG,
    memory_pool::{BlockInTransit, MemoryPool},
    node::Nodes,
    transaction::Transaction,
    utxo_set::UTXOSet,
};

const NODE_VERSION: usize = 1;
const CENTERAL_NODE: &str = "127.0.0.1:2001";

static GLOBAL_NODES: Lazy<Nodes> = Lazy::new(|| {
    let nodes = Nodes::new();
    nodes.add_node(String::from(CENTERAL_NODE));

    return nodes;
});

static GLOBAL_MEMORY_POOL: Lazy<MemoryPool> = Lazy::new(|| MemoryPool::new());

static GLOBAL_BLOCK_IN_TRANSIT: Lazy<BlockInTransit> = Lazy::new(|| BlockInTransit::new());

const TCP_WRITE_TIMEOUT: u64 = 1000;

pub struct Server {
    blockchain: Blockchain,
}

impl Server {
    pub fn new(blockchain: Blockchain) -> Server {
        Server { blockchain }
    }

    pub fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr)?;

        if !addr.eq(CENTERAL_NODE) {
            let best_height = self.blockchain.get_best_height()?;
            send_version(CENTERAL_NODE, best_height)?;
        }

        for stream in listener.incoming() {
            let blockchain = self.blockchain.clone();

            thread::spawn(move || match stream {
                Ok(stream) => {
                    if let Err(e) = serve(&blockchain, stream) {
                        eprintln!("Error handling connection: {}", e);
                    }
                }
                Err(e) => {
                    println!("error: {}", e)
                }
            });
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum OpType {
    Block,
    Tx,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Package {
    Block {
        addr_from: String,
        block: Vec<u8>,
    },
    GetBlocks {
        addr_from: String,
    },
    GetData {
        addr_from: String,
        op_type: OpType,
        id: Vec<u8>,
    },
    Inv {
        addr_from: String,
        op_type: OpType,
        items: Vec<Vec<u8>>,
    },
    Tx {
        addr_from: String,
        transaction: Vec<u8>,
    },
    Version {
        addr_from: String,
        version: usize,
        best_height: usize,
    },
}

fn send_version(addr: &str, height: usize) -> Result<()> {
    let socket_addr: SocketAddr = addr.parse()?;
    let node_addr = match GLOBAL_CONFIG.get_node_addr()? {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("get node addr none")),
    };

    send_data(
        socket_addr,
        Package::Version {
            addr_from: node_addr,
            version: NODE_VERSION,
            best_height: height,
        },
    )
}

fn send_get_data(addr: &str, op_type: OpType, id: &[u8]) -> Result<()> {
    let socket_addr: SocketAddr = addr.parse()?;
    let node_addr = match GLOBAL_CONFIG.get_node_addr()? {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("get node addr none")),
    };

    send_data(
        socket_addr,
        Package::GetData {
            addr_from: node_addr,
            op_type,
            id: id.to_vec(),
        },
    )
}

fn send_data(socket_addr: SocketAddr, pkg: Package) -> Result<()> {
    println!("send data to {:?}, package: {:?}", socket_addr, pkg);
    let mut stream = match TcpStream::connect(socket_addr) {
        Ok(stream) => stream,
        Err(e) => {
            println!("The {} is not valid, error: {}", socket_addr, e);
            GLOBAL_NODES.evict_node(socket_addr.to_string().as_str());
            return Ok(());
        }
    };

    stream.set_write_timeout(Some(Duration::from_millis(TCP_WRITE_TIMEOUT)))?;
    serde_json::to_writer(&stream, &pkg)?;
    stream.flush()?;

    Ok(())
}

fn send_block(addr: &str, block: &Block) -> Result<()> {
    let socket_addr: SocketAddr = addr.parse()?;
    let node_addr = match GLOBAL_CONFIG.get_node_addr()? {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("get node addr none")),
    };

    send_data(
        socket_addr,
        Package::Block {
            addr_from: node_addr,
            block: block.serialize()?,
        },
    )
}

pub fn send_tx(addr: &str, tx: &Transaction) -> Result<()> {
    let socket_addr: SocketAddr = addr.parse()?;
    let node_addr = match GLOBAL_CONFIG.get_node_addr()? {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("get node addr none")),
    };

    send_data(
        socket_addr,
        Package::Tx {
            addr_from: node_addr,
            transaction: tx.serialize()?,
        },
    )
}

fn send_get_blocks(addr: &str) -> Result<()> {
    let socket_addr: SocketAddr = addr.parse()?;
    let node_addr = match GLOBAL_CONFIG.get_node_addr()? {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("get node addr none")),
    };

    send_data(
        socket_addr,
        Package::GetBlocks {
            addr_from: node_addr,
        },
    )
}

fn send_inv(addr: &str, op_type: OpType, blocks: &[Vec<u8>]) -> Result<()> {
    let socket_addr: SocketAddr = addr.parse()?;
    let node_addr = match GLOBAL_CONFIG.get_node_addr()? {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("get node addr none")),
    };

    send_data(
        socket_addr,
        Package::Inv {
            addr_from: node_addr,
            op_type,
            items: blocks.to_vec(),
        },
    )
}

#[allow(dead_code)]
fn serve(blockchain: &Blockchain, stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let peer_addr = stream.peer_addr()?;
    let reader = BufReader::new(&stream);
    let mut pkg_reader = Deserializer::from_reader(reader).into_iter::<Package>();

    while let Some(Ok(pkg)) = pkg_reader.next() {
        println!("receive request from {:?}, package: {:?}", peer_addr, pkg);

        match pkg {
            Package::Version {
                addr_from,
                version,
                best_height,
            } => {
                println!(
                    "version: {}, best_height: {}, addr_from: {}",
                    version, best_height, addr_from
                );

                let local_best_height = blockchain.get_best_height()?;
                if local_best_height < best_height {
                    send_get_blocks(addr_from.as_str())?;
                }
                if local_best_height > best_height {
                    send_version(addr_from.as_str(), local_best_height)?;
                }

                if GLOBAL_NODES.node_is_known(peer_addr.to_string().as_str()) == false {
                    GLOBAL_NODES.add_node(addr_from);
                }
            }
            Package::GetBlocks { addr_from } => {
                let blocks = blockchain.get_block_hashes();
                send_inv(addr_from.as_str(), OpType::Block, &blocks)?;
            }
            Package::Inv {
                addr_from,
                op_type,
                items,
            } => match op_type {
                OpType::Block => {
                    GLOBAL_BLOCK_IN_TRANSIT.add_blocks(items.as_slice())?;

                    if let Some(block_hash) = items.get(0) {
                        send_get_data(addr_from.as_str(), OpType::Block, block_hash)?;
                        GLOBAL_BLOCK_IN_TRANSIT.remove(block_hash)?;
                    }
                }
                OpType::Tx => {
                    let txid = items.first().ok_or(anyhow::anyhow!("get txid none"))?;
                    let txid_hex = data_encoding::HEXLOWER.encode(txid);

                    if !GLOBAL_MEMORY_POOL.contains(txid_hex.as_str())? {
                        send_get_data(addr_from.as_str(), OpType::Tx, txid)?;
                    }
                }
            },
            Package::GetData {
                addr_from,
                op_type,
                id,
            } => match op_type {
                OpType::Block => {
                    if let Some(block) = blockchain.get_block(id.as_slice())? {
                        send_block(addr_from.as_str(), &block)?;
                    }
                }
                OpType::Tx => {
                    let txid_hex = data_encoding::HEXLOWER.encode(id.as_slice());
                    if let Some(tx) = GLOBAL_MEMORY_POOL.get(txid_hex.as_str())? {
                        send_tx(addr_from.as_str(), &tx)?;
                    }
                }
            },
            Package::Block { addr_from, block } => {
                let block = Block::deserialize(&block)?;
                blockchain.add_block(&block)?;
                println!("add block: {:?}", block.get_hash());

                if GLOBAL_BLOCK_IN_TRANSIT.len()? > 0 {
                    let block_hash = GLOBAL_BLOCK_IN_TRANSIT.first()?;
                    if let Some(block_hash) = block_hash {
                        send_get_data(addr_from.as_str(), OpType::Block, block_hash.as_slice())?;

                        GLOBAL_BLOCK_IN_TRANSIT.remove(block_hash.as_slice())?;
                    }
                } else {
                    let utxo_set = UTXOSet::new(blockchain.clone());
                    utxo_set.reindex()?;
                }
            }
            Package::Tx {
                addr_from,
                transaction,
            } => {
                let tx = Transaction::deserialize(&transaction)?;
                let txid = tx.get_id_bytes();
                GLOBAL_MEMORY_POOL.add(tx)?;

                let node_addr = GLOBAL_CONFIG
                    .get_node_addr()?
                    .ok_or(anyhow::anyhow!("get node addr none"))?;

                if node_addr.eq(CENTERAL_NODE) {
                    let nodes = GLOBAL_NODES.get_nodes();
                    for node in &nodes {
                        // 不发送给自己
                        if node_addr.eq(node.get_addr().as_str()) {
                            continue;
                        }
                        // 不发送给发送者
                        if addr_from.eq(node.get_addr().as_str()) {
                            continue;
                        }
                        send_inv(&node.get_addr(), OpType::Tx, &[txid.clone()])?;
                    }
                }
            }
        }
    }

    Ok(())
}
