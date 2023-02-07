extern crate bitcoincore_rpc;
extern crate confy;

// use bitcoincore_rpc::jsonrpc::client;
// use bitcoincore_rpc::{bitcoin::Txid, Auth, Client, RpcApi};
use bitcoincore_rpc::{Auth, Client, RpcApi};

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
        cfg.source_user.unwrap(),
        cfg.source_passwd.unwrap(),
        ClientType::Source,
    )?;
    let dest_rpc = get_client(
        &cfg.dest_ip_addr,
        cfg.dest_user.unwrap(),
        cfg.dest_passwd.unwrap(),
        ClientType::Destination,
    )?;

    let source_get_mempool_info_result = source_rpc
        .get_mempool_info()
        .with_context(|| format!("Can't connect to {}", cfg.source_ip_addr))?;
    let dest_get_mempool_info_result = dest_rpc
        .get_mempool_info()
        .with_context(|| format!("Can't connect to {}", cfg.source_ip_addr))?;
    if cfg.verbose {
        println!(
            "# Transactions in source mempool/destination mempool: {}/{}",
            source_get_mempool_info_result.size, dest_get_mempool_info_result.size
        );
    }
    Ok(())

    //#[derive(Debug)]
    // struct TxDepth {
    //     ancestor_count: u64,
    //     tx_id: Txid,
    // }
    /*
    let rpc = Client::new(
        &cfg.source_url,
        Auth::UserPass(cfg.source_user, cfg.source_passwd),
    )
    .with_context(|| format!("Can't connect to {}", cfg.source_url))?;

    let get_mempool_info_result = rpc
        .get_mempool_info()
        .with_context(|| format!("Can't connect to {}", cfg.source_url))?;
    println!("# Transactions in mepool: {}", get_mempool_info_result.size);

    let vec: Vec<TxDepth> = rpc
        .get_raw_mempool_verbose()?
        .iter()
        .filter_map(|(tx_ide, mempool_entry)| {
            if mempool_entry.ancestor_count == 1 {
                // rpc.
            }
            // Next rpc call can fail if txIde has been removed from mempool while executing this
            // program.
            match rpc.get_raw_transaction_hex(tx_ide, None).ok() {
                None => None,
                Some(tx_raw) => Some(TxDepth {
                    // raw_tx_hex: tx_raw,
                    ancestor_count: mempool_entry.ancestor_count,
                    tx_id: tx_ide.clone(),
                }),
            }
        })
        .collect();

    // println!("{:#?}", vec);
    */
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
