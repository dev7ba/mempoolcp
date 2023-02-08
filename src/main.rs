extern crate bitcoincore_rpc;
extern crate confy;

// use bitcoincore_rpc::jsonrpc::client;
use bitcoincore_rpc::{bitcoin::Txid, Auth, Client, RpcApi};

use anyhow::Context;
use anyhow::Result;
use config::Config;

mod config;

fn main() -> Result<()> {
    let cfg = Config::load().with_context(|| "Error loading configuration".to_string())?;
    if cfg.verbose {
        println!("{}", cfg);
    }

    let source_rpc = get_client(
        &cfg.source_ip_addr,
        cfg.source_user.clone().unwrap(),
        cfg.source_passwd.clone().unwrap(),
        ClientType::Source,
    )?;
    let dest_rpc = get_client(
        &cfg.dest_ip_addr,
        cfg.dest_user.clone().unwrap(),
        cfg.dest_passwd.clone().unwrap(),
        ClientType::Destination,
    )?;

    print_mempool_sizes(&source_rpc, &dest_rpc, &cfg)?;

    let vec: Vec<TxDepth> = source_rpc
        .get_raw_mempool_verbose()?
        .iter()
        .map(|(tx_ide, mempool_entry)| TxDepth {
            ancestor_count: mempool_entry.ancestor_count as usize,
            tx_id: tx_ide.clone(),
        })
        .collect();

    //TODO Intenta que no dependa de un valor predeterminado.
    // let mut vec2: Vec<Vec<Txid>> = std::iter::repeat(vec![]).take(25).collect::<Vec<_>>();

    let mut vec2: Vec<Vec<Txid>> = vec![vec![]; 25];

    for tx_depth in vec {
        let ancestor_index = tx_depth.ancestor_count - 1;
        vec2[ancestor_index].push(tx_depth.tx_id);
    }

    if cfg.verbose {
        println!("\nTransactions dependencies:\n");
        for (i, txid_vec) in vec2.iter().enumerate() {
            println!("#Txs depending of {} parents: {}", i, txid_vec.len());
        }
        println!("");
    }

    let mut failed_query_txs: usize = 0;
    let mut failed_sent_txs: usize = 0;

    for txid_vec in &vec2 {
        for txid in txid_vec {
            let tx_hex = match source_rpc.get_raw_transaction_hex(txid, None) {
                Ok(tx_hex) => tx_hex,
                Err(err) => {
                    failed_query_txs += 1;
                    if cfg.verbose {
                        println!("Failed source TxId: {:?} Reason: {:?}", txid, err);
                    }
                    continue;
                }
            };
            match dest_rpc.send_raw_transaction(tx_hex) {
                Ok(_) => (),
                Err(err) => {
                    failed_sent_txs += 1;
                    if cfg.verbose {
                        println!("Failed destination TxId: {:?} Reason: {:?}", txid, err);
                    }
                }
            }
        }
    }

    if cfg.verbose {
        println!("\n#Failed queried txs: {}", failed_query_txs);
        println!("#Failed sent txs: {}", failed_sent_txs);

        println!("\nFailed queried transactions (if any) are because of transactions removed from mempool while executing this program. i.e. RBF txs");
        println!("\nFailed sent transactions (if any) are because of parent transaction removed from mempool while executing this program.\n");
    }
    print_mempool_sizes(&source_rpc, &dest_rpc, &cfg)?;

    println!("\nMempool sizes could not be the same at the end because of conflicting transactions between the initial transactions set or new arriving txs while executing this program.");

    Ok(())
}

fn print_mempool_sizes(
    source_rpc: &Client,
    dest_rpc: &Client,
    cfg: &Config,
) -> Result<(), anyhow::Error> {
    let source_size = source_rpc
        .get_mempool_info()
        .with_context(|| format!("Can't connect to {}", cfg.source_ip_addr))?
        .size;
    let dest_size = dest_rpc
        .get_mempool_info()
        .with_context(|| format!("Can't connect to {}", cfg.dest_ip_addr))?
        .size;
    println!(
        "# Transactions in source mempool/destination mempool: {}/{} ({} gap)",
        source_size,
        dest_size,
        source_size - dest_size
    );
    Ok(())
}

#[derive(Debug)]
struct TxDepth {
    ancestor_count: usize,
    tx_id: Txid,
}

#[derive(Debug)]
enum ClientType {
    Source,
    Destination,
}

fn get_client(
    ip: &str,
    user_name: String,
    passwd: String,
    client_type: ClientType,
) -> Result<Client> {
    Client::new(ip, Auth::UserPass(user_name, passwd))
        .with_context(|| format!("Can't connect to {:?} bitcoind node: {}", client_type, ip))
}
