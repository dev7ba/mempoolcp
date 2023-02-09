extern crate bitcoincore_rpc;
extern crate confy;
use anyhow::{anyhow, Context, Result};
use bitcoincore_rpc::{bitcoin::Txid, Auth, Client, RpcApi};
use config::Config;
use indicatif::ParallelProgressIterator;
use indicatif::ProgressStyle;
use std::sync::{Arc, Mutex};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::sync::atomic::{AtomicUsize, Ordering};
mod config;

fn main() -> Result<()> {
    let cfg = Config::load().with_context(|| "Error loading configuration".to_string())?;
    if cfg.verbose {
        println!("{}", cfg);
    }

    let source_client = get_client(
        &cfg.source_ip_addr,
        cfg.source_user.clone().unwrap(),
        cfg.source_passwd.clone().unwrap(),
        ClientType::Source,
    )?;
    let dest_client = get_client(
        &cfg.dest_ip_addr,
        cfg.dest_user.clone().unwrap(),
        cfg.dest_passwd.clone().unwrap(),
        ClientType::Destination,
    )?;

    println!("");
    print_mempool_sizes(&source_client, &dest_client, &cfg, "(Beginning)\t")?;

    let vec2 = get_mempool(&source_client, cfg.fast_mode)?;

    if cfg.verbose {
        println!("\nTransactions dependencies:\n");
        for (i, txid_vec) in vec2.iter().enumerate() {
            println!("#Txs depending of {} parents: {}", i, txid_vec.len());
        }
        println!("");
    }

    let failed_query_txs = AtomicUsize::new(0);
    let failed_sent_txs = AtomicUsize::new(0);
    let vec_txs_error: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

    for (i, txid_vec) in vec2.iter().enumerate() {
        let style = ProgressStyle::with_template(
            "{prefix} [{elapsed_precise}] {wide_bar} {pos:>7}/{len:7} ",
        )
        .unwrap();
        txid_vec
            .par_iter()
            .progress_with_style(style)
            .with_prefix(format!(
                "Txs depending of {} parents: {}",
                i,
                txid_vec.len()
            ))
            .for_each(|txid| {
                match source_client.get_raw_transaction_hex(txid, None) {
                    Ok(tx_hex) => match dest_client.send_raw_transaction(tx_hex) {
                        Ok(_) => (),
                        Err(err) => {
                            failed_sent_txs.fetch_add(1, Ordering::SeqCst);
                            if cfg.verbose {
                                vec_txs_error.lock().unwrap().push(format!(
                                    "Failed destination TxId: {:?} Reason: {:?}",
                                    txid, err
                                ));
                            }
                        }
                    },
                    Err(err) => {
                        failed_query_txs.fetch_add(1, Ordering::SeqCst);
                        if cfg.verbose {
                            vec_txs_error
                                .lock()
                                .unwrap()
                                .push(format!("Failed source TxId: {:?} Reason: {:?}", txid, err));
                        }
                    }
                };
            });
    }

    if cfg.verbose {
        vec_txs_error
            .lock()
            .unwrap()
            .iter()
            .for_each(|err| println!("{}", err));

        println!("\n#Failed queried txs: {:?}", failed_query_txs);
        println!("#Failed sent txs: {:?}", failed_sent_txs);

        println!("\nFailed queried transactions (if any) are because of transactions removed from mempool while executing this program. i.e. RBF txs");
        println!("\nFailed sent transactions (if any) are because of parent transaction removed from mempool while executing this program.\n");
    }
    print_mempool_sizes(&source_client, &dest_client, &cfg, "(End)\t\t")?;

    println!("\nNote: Mempool sizes could not be the same at the end because of conflicting transactions between the initial transactions set or new arriving txs while executing this program.");

    Ok(())
}

fn print_mempool_sizes(
    source_rpc: &Client,
    dest_rpc: &Client,
    cfg: &Config,
    prefix: &str,
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
        "# {} Transactions in source mempool/destination mempool: {}/{} ({} gap)",
        prefix,
        source_size,
        dest_size,
        source_size - dest_size
    );
    Ok(())
}

fn get_mempool(source_client: &Client, fast_mode: bool) -> Result<Vec<Vec<Txid>>> {
    if fast_mode {
        let vec: Vec<TxDepth> = source_client
            .get_raw_mempool_verbose()?
            .iter()
            .map(|(tx_ide, mempool_entry)| TxDepth {
                ancestor_count: mempool_entry.ancestor_count as usize,
                tx_id: tx_ide.clone(),
            })
            .collect();

        let mut vec2: Vec<Vec<Txid>> = vec![];
        for tx_depth in vec {
            let ancestor_index = tx_depth.ancestor_count - 1;
            while vec2.len() <= ancestor_index {
                vec2.push(vec![]);
            }
            vec2[ancestor_index].push(tx_depth.tx_id);
        }
        return Ok(vec2);
    }

    Err(anyhow!("Slow mode not implemented, use --fast-mode flag"))
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
