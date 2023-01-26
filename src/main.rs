extern crate bitcoincore_rpc;

use bitcoincore_rpc::{Auth, Client, RpcApi};

fn main() {
    let rpc = Client::new(
        "http://localhost:8332",
        Auth::UserPass("anon".to_string(), "anon".to_string()),
    )
    .unwrap();
    let best_block_hash = rpc.get_best_block_hash().unwrap();
    println!("best block hash: {}", best_block_hash);

    // let tx_id: Txid =
    //     Txid::from_str("72951d0e0b87cb9d8b82325523e963ed6d67c931d208e8d74d6f12131c6c5d87").unwrap();
    // let get_mempool_entry_result = rpc.get_mempool_entry(&tx_id).unwrap();
    // println!("{:?}", get_mempool_entry_result);

    let get_mempool_info_result = rpc.get_mempool_info().unwrap();
    println!("{:?}", get_mempool_info_result);

    //
    //Txid::from_str(
    // "72951d0e0b87cb9d8b82325523e963ed6d67c931d208e8d74d6f12131c6c5d87",
    // ));
}
