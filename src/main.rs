extern crate bitcoincore_rpc;
extern crate confy;
use crate::zmqseq::ZmqThread;
use anyhow::{Context, Result};
use bitcoincore_rpc::{bitcoin::Txid, Auth, Client, RpcApi};
use config::Config;
use indicatif::ParallelProgressIterator;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;
use std::str;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
mod config;
mod zmqseq;

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

fn main() -> Result<()> {
    let cfg = Config::load().with_context(|| "Error loading configuration".to_string())?;
    if cfg.verbose {
        println!("{}", cfg);
    }

    let (source_client, dest_client) = get_clients(&cfg)?;

    //If zmq option, then spawn a thread to receive zmq transactions while working.
    let zmq_thread = match cfg.zmq_address {
        Some(ref address) => Some(ZmqThread::spawn(address)),
        None => None,
    };

    print_mempool_sizes(&source_client, &dest_client, &cfg, "(Beginning)\t")?;

    let vec = get_tx_dept_vec(&source_client, cfg.fast_mode)?;

    //vec2 is a vector of vectors containing txs with same ancestor_count:
    //(vec2[ancestor_count-1] has a vector with all tx having ancestor_count-1)
    let vec2 = get_mempool_layers(vec);

    list_mempool_layers(&cfg, &vec2);

    //Thread-safe things...
    let failed_query_txs = AtomicUsize::new(0);
    let failed_sent_txs = AtomicUsize::new(0);
    let vec_txs_error: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

    //First retransmit txs obtained vía RPC
    retransmit_rpc_txs(
        vec2,
        &source_client,
        &dest_client,
        &failed_sent_txs,
        &failed_query_txs,
        &cfg,
        &vec_txs_error,
    );

    // If zmq option, then retransmit all ZMQ transactions received during execution.
    retransmit_zmq_txs(
        zmq_thread,
        &source_client,
        &dest_client,
        &failed_sent_txs,
        &failed_query_txs,
        &cfg,
        &vec_txs_error,
    );

    //If verbose mode, then print failed txs during retranmission.
    print_failed_txs(&cfg, vec_txs_error, failed_query_txs, failed_sent_txs);

    print_mempool_sizes(&source_client, &dest_client, &cfg, "(End)\t\t")?;

    println!("\nNote: Mempool sizes could not be the same at the end because of different peers connections, conflicting transactions or transaction arrival timing issues between nodes (among other causes).");

    Ok(())
}

fn get_clients(cfg: &Config) -> Result<(Client, Client), anyhow::Error> {
    let source_client = if let Some(path) = &cfg.source_cookie_auth_path {
        get_client_cookie(&cfg.source_ip_addr, path.clone(), ClientType::Source)?
    } else {
        get_client_user_passw(
            &cfg.source_ip_addr,
            cfg.source_user.as_ref().unwrap().clone(),
            cfg.source_passwd.as_ref().unwrap().clone(),
            ClientType::Source,
        )?
    };

    let dest_client = if let Some(path) = &cfg.dest_cookie_auth_path {
        get_client_cookie(&cfg.dest_ip_addr, path.clone(), ClientType::Destination)?
    } else {
        get_client_user_passw(
            &cfg.dest_ip_addr,
            cfg.dest_user.as_ref().unwrap().clone(),
            cfg.dest_passwd.as_ref().unwrap().clone(),
            ClientType::Destination,
        )?
    };

    Ok((source_client, dest_client))
}

fn get_client_cookie(ip: &str, path: PathBuf, client_type: ClientType) -> Result<Client> {
    Client::new(ip, Auth::CookieFile(path))
        .with_context(|| format!("Can't connect to {:?} bitcoind node: {}", client_type, ip))
}
fn get_client_user_passw(
    ip: &str,
    user_name: String,
    passwd: String,
    client_type: ClientType,
) -> Result<Client> {
    Client::new(ip, Auth::UserPass(user_name, passwd))
        .with_context(|| format!("Can't connect to {:?} bitcoind node: {}", client_type, ip))
}

fn list_mempool_layers(cfg: &Config, vec2: &Vec<Vec<Txid>>) {
    if cfg.verbose {
        println!("\nTransactions dependencies:\n");
        for (i, txid_vec) in vec2.iter().enumerate() {
            println!("#Txs depending of {} parents: {}", i, txid_vec.len());
        }
        println!("");
    }
}

fn get_tx_dept_vec(source_client: &Client, fast_mode: bool) -> Result<Vec<TxDepth>> {
    if fast_mode {
        let vec: Vec<TxDepth> = source_client
            .get_raw_mempool_verbose()?
            .iter()
            .map(|(tx_ide, mempool_entry)| TxDepth {
                ancestor_count: mempool_entry.ancestor_count as usize,
                tx_id: tx_ide.clone(),
            })
            .collect();
        return Ok(vec);
    } else {
        let vec: Vec<TxDepth> = source_client
            .get_raw_mempool()?
            .par_iter()
            .filter_map(|tx_id| match source_client.get_mempool_entry(tx_id) {
                Ok(entry) => Some((tx_id, entry)),
                Err(_) => None, //If tx_id do not exist we don't care
            })
            .map(|(tx_id, entry)| TxDepth {
                ancestor_count: entry.ancestor_count as usize,
                tx_id: tx_id.clone(),
            })
            .collect();
        return Ok(vec);
    }
}

fn get_mempool_layers(vec: Vec<TxDepth>) -> Vec<Vec<Txid>> {
    let mut vec2: Vec<Vec<Txid>> = vec![];
    for tx_depth in vec {
        let ancestor_index = tx_depth.ancestor_count - 1;
        while vec2.len() <= ancestor_index {
            vec2.push(vec![]);
        }
        vec2[ancestor_index].push(tx_depth.tx_id);
    }
    vec2
}

fn retransmit_zmq_txs(
    zmq_thread: Option<ZmqThread>,
    source_client: &Client,
    dest_client: &Client,
    failed_sent_txs: &AtomicUsize,
    failed_query_txs: &AtomicUsize,
    cfg: &Config,
    vec_txs_error: &Arc<Mutex<Vec<String>>>,
) {
    if zmq_thread.is_some() {
        println!("");
        let sp = create_spinner();
        let txs = zmq_thread.unwrap().for_each(|txid| {
            retransmit(
                txid,
                source_client,
                dest_client,
                failed_sent_txs,
                failed_query_txs,
                cfg,
                vec_txs_error,
            )
        });
        sp.finish_with_message(format!(
            "Done. Sent {} additional transactions from ZMQ iterface",
            txs
        ));
        println!("\n");
    }
}

fn retransmit_rpc_txs(
    vec2: Vec<Vec<Txid>>,
    source_client: &Client,
    dest_client: &Client,
    failed_sent_txs: &AtomicUsize,
    failed_query_txs: &AtomicUsize,
    cfg: &Config,
    vec_txs_error: &Arc<Mutex<Vec<String>>>,
) {
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
            .for_each(|tx_id| {
                retransmit(
                    tx_id,
                    source_client,
                    dest_client,
                    failed_sent_txs,
                    failed_query_txs,
                    cfg,
                    vec_txs_error,
                )
            });
    }
}
fn retransmit(
    txid: &Txid,
    source_client: &Client,
    dest_client: &Client,
    failed_sent_txs: &AtomicUsize,
    failed_query_txs: &AtomicUsize,
    cfg: &Config,
    vec_txs_error: &Arc<Mutex<Vec<String>>>,
) {
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
}

fn print_failed_txs(
    cfg: &Config,
    vec_txs_error: Arc<Mutex<Vec<String>>>,
    failed_query_txs: AtomicUsize,
    failed_sent_txs: AtomicUsize,
) {
    if cfg.verbose {
        vec_txs_error
            .lock()
            .unwrap()
            .iter()
            .for_each(|err| println!("{}", err));

        println!("\n#Failed queried txs: {:?}", failed_query_txs);
        println!("#Failed sent txs: {:?}", failed_sent_txs);

        println!("\nFailed queried transactions (if any) are because of transactions removed from mempool while executing this program. i.e. RBF txs");
        println!("\nFailed sent transactions (if any) are because of parent transaction removed from mempool while executing this program.");
    }
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
        "\n# {} Transactions in source mempool/destination mempool: {}/{} ({} gap)",
        prefix,
        source_size,
        dest_size,
        (source_size as i64 - dest_size as i64).abs()
    );
    Ok(())
}

fn create_spinner() -> ProgressBar {
    let sp = ProgressBar::new_spinner();
    sp.set_message("Sending ZMQ Transactions...");
    sp.enable_steady_tick(Duration::from_millis(120));
    sp.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ]),
    );
    sp
}
