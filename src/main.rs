use anyhow::Result;
use clap::Parser;

use blockchain_rust::{
    blockchain::Blockchain,
    config,
    server::{self, Server},
    transaction::Transaction,
    utils,
    utxo_set::UTXOSet,
    wallets::{self, Wallets},
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    Layer as _, fmt::Layer, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

const MINE_TRUE: usize = 1;

#[derive(Debug, Parser)]
#[command(author, about, version, long_about=None)]
struct Args {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Parser)]
enum Command {
    #[command(name = "create-wallet", about = "Create a new wallet")]
    CreateWallet,

    #[command(name = "create-blockchain", about = "Create a new blockchain")]
    CreateBlockchain {
        #[arg(long, help = "The address of the genesis block")]
        address: String,
    },

    #[command(name = "get-balance", about = "Get the balance of a wallet")]
    GetBalance {
        #[arg(long, help = "The address of the wallet")]
        address: String,
    },

    #[command(name = "list-addresses", about = "List all addresses in the wallet")]
    ListAddresses,

    #[command(name = "send", about = "Add new block to chain")]
    Send {
        #[arg(long, help = "The address of the sender")]
        from: String,
        #[arg(long, help = "The address of the recipient")]
        to: String,
        #[arg(long, help = "The amount to send")]
        amount: i32,
        #[arg(long, help = "Mine immediately on the same node")]
        mine: usize,
    },

    #[command(name = "print-chain", about = "Print blockchain all block")]
    PrintChain,

    #[command(name = "reindex-utxo", about = "rebuild UTXO index set")]
    ReindexUtxo,

    #[command(name = "start-node", about = "Start a node")]
    StartNode {
        #[arg(long, help = "Enable mining mode and send reward to ADDRESS")]
        miner: Option<String>,
    },
}

fn main() -> Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let args = Args::parse();

    match args.cmd {
        Command::CreateWallet => {
            let mut wallets = Wallets::try_new()?;
            let address = wallets.create_wallet()?;
            println!("Your new address: {}", address);

            Ok(())
        }
        Command::CreateBlockchain { address } => {
            let blockchain = Blockchain::create_blockchain(&address)?;
            let utxo_set = UTXOSet::new(blockchain);
            utxo_set.reindex()?;
            println!("Done!");

            Ok(())
        }
        Command::GetBalance { address } => {
            if !wallets::validate_address(&address) {
                return Err(anyhow::anyhow!("Invalid address"));
            }

            let payload = utils::base58_decode(address.as_str());
            let pub_key_hash = &payload[1..payload.len() - wallets::ADDRESS_CHECK_SUM_LEN];

            let blockchain = Blockchain::new_blockchain()?;
            let utxo_set = UTXOSet::new(blockchain);
            let utxos = utxo_set.find_utxo(pub_key_hash)?;

            let mut balance = 0;
            for utxo in utxos {
                balance += utxo.get_value();
            }
            println!("Balance of {}: {}", address, balance);

            Ok(())
        }
        Command::ListAddresses => {
            let wallets = Wallets::try_new()?;
            for address in wallets.get_addresses() {
                println!("address: {}", address);
            }

            Ok(())
        }
        Command::Send {
            from,
            to,
            amount,
            mine,
        } => {
            if !wallets::validate_address(&from) {
                return Err(anyhow::anyhow!("Invalid from address"));
            }
            if !wallets::validate_address(&to) {
                return Err(anyhow::anyhow!("Invalid to address"));
            }

            let blockchain = Blockchain::new_blockchain()?;
            let utxo_set = UTXOSet::new(blockchain.clone());

            let transaction =
                Transaction::new_utxo_transaction(from.as_str(), to.as_str(), amount, &utxo_set)?;

            if mine == MINE_TRUE {
                let coinbase_tx = Transaction::new_coinbase_tx(from.as_str())?;
                let block = blockchain.mine_block(&vec![transaction, coinbase_tx])?;

                utxo_set.update(&block)?;
            } else {
                server::send_tx(config::DEFAULT_NODE_ADDR, &transaction)?;
            }
            println!("Send success!");

            Ok(())
        }
        Command::PrintChain => {
            let mut block_iterator = Blockchain::new_blockchain()?.iterator();
            while let Ok(Some(block)) = block_iterator.next() {
                println!("Pre block hash: {}", block.get_pre_block_hash());
                println!("Cur block hash: {}", block.get_hash());
                println!("Cur block Timestamp: {}", block.get_timestamp());

                for tx in block.get_transactions() {
                    let cur_txid_hex = data_encoding::HEXLOWER.encode(tx.get_id());
                    println!("- Transaction txid_hex: {}", cur_txid_hex);

                    if !tx.is_coinbase() {
                        for input in tx.get_vin() {
                            let txid_hex = data_encoding::HEXLOWER.encode(input.get_txid());
                            let pub_key_hash = wallets::hash_pub_key(input.get_pub_key());
                            let address = wallets::convert_address(pub_key_hash.as_slice());
                            println!(
                                "-- Input txid = {}, vout = {}, from = {}",
                                txid_hex,
                                input.get_vout(),
                                address,
                            )
                        }
                    }

                    for output in tx.get_vout() {
                        let pub_key_hash = output.get_pub_key_hash();
                        let address = wallets::convert_address(pub_key_hash);
                        println!("-- Output value = {}, to = {}", output.get_value(), address,)
                    }
                }
            }

            Ok(())
        }
        Command::ReindexUtxo => {
            let blockchain = Blockchain::new_blockchain()?;
            let utxo_set = UTXOSet::new(blockchain.clone());
            utxo_set.reindex()?;
            let count = utxo_set.count_transactions()?;
            println!("Done! There are {} transactions in the UTXO set.", count);

            Ok(())
        }
        Command::StartNode { miner } => {
            if let Some(addr) = miner {
                if wallets::validate_address(addr.as_str()) == false {
                    return Err(anyhow::anyhow!("Wrong miner address!"));
                }
                println!("Mining is on. Address to receive rewards: {}", addr);
                config::GLOBAL_CONFIG.set_mining_addr(addr)?;
            }
            let blockchain = Blockchain::new_blockchain()?;
            let sockert_addr = config::GLOBAL_CONFIG.get_node_addr()?;
            match sockert_addr {
                Some(addr) => Server::new(blockchain).run(addr.as_str()),
                None => Err(anyhow::anyhow!("Error running server")),
            }
        }
    }
}
