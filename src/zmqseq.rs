use bitcoincore_rpc::bitcoin::hashes::sha256d::Hash;
use bitcoincore_rpc::bitcoin::Txid;
use std::str;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Barrier};
use std::thread;
use std::thread::JoinHandle;

use url::Url;

#[derive(Debug)]
enum MempoolSequence {
    BlockConnection(String),
    BlockDisconnection(String),
    TxRemoved { _txid: String, _seq_num: u64 },
    TxAdded { txid: String, _seq_num: u64 },
}

impl TryFrom<&[u8]> for MempoolSequence {
    type Error = &'static str;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match *value.get(32).unwrap() {
            // "C"
            67 => Ok(MempoolSequence::BlockConnection(hex::encode(&value[..32]))),
            // "D"
            68 => Ok(MempoolSequence::BlockDisconnection(hex::encode(
                &value[..32],
            ))),
            // "R"
            82 => Ok(MempoolSequence::TxRemoved {
                _txid: hex::encode(&value[..32]),
                _seq_num: u64::from_le_bytes(value[33..41].try_into().expect("Not enough size")),
            }),
            // "A"
            65 => Ok(MempoolSequence::TxAdded {
                txid: hex::encode(&value[..32]),
                _seq_num: u64::from_le_bytes(value[33..41].try_into().expect("Not enough size")),
            }),
            _ => Err("Invalid char code in message"),
        }
    }
}

// pub fn into_arr4<T>(v: Vec<T>) -> [T; 4] {
//     let boxed_slice = v.into_boxed_slice();
//     let boxed_array: Box<[T; 4]> = match boxed_slice.try_into() {
//         Ok(ba) => ba,
//         Err(o) => panic!("Expected a Vec of length {} but it was {}", 4, o.len()),
//     };
//     *boxed_array
// }
pub struct ZmqThread {
    rx: Receiver<String>,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl ZmqThread {
    pub fn spawn(zmq_address: &Url) -> Self {
        let context = zmq::Context::new();
        let subscriber = context.socket(zmq::SUB).unwrap();
        subscriber
            .connect(zmq_address.as_str())
            .expect("Cannot connect to publisher");
        subscriber
            .set_subscribe(b"sequence")
            .expect("Failed subscribing.");
        let stop_th = Arc::new(AtomicBool::new(false));
        let stop = stop_th.clone();
        let (tx, rx) = channel();
        //Use a barrier, the 'loadmempool' phase should execute after this method to not
        //loose any tx at the cost of loading/receiving twice some txs.
        let barrier = Arc::new(Barrier::new(2));

        let barrierc = barrier.clone();
        let thread = thread::spawn(move || {
            let mut wait = true;
            while !stop_th.load(Ordering::SeqCst) {
                let msg = subscriber.recv_multipart(0).unwrap();
                if wait {
                    barrier.wait();
                    wait = false;
                }
                let topic = str::from_utf8(msg.get(0).unwrap()).expect("Cannot unwrap topic");
                if topic.ne("sequence") {
                    panic!("ZMQ topic should be 'sequence' but it's: {}", topic);
                }
                let body = msg.get(1).unwrap();
                let mpsq = MempoolSequence::try_from(&body[..]).unwrap();
                // println!("{:?}", mpsq);
                // let seq = u32::from_le_bytes(into_arr4(msg.get(2).unwrap().to_vec()));
                // println!("Seq: {}", seq);
                match mpsq {
                    MempoolSequence::TxAdded {
                        txid: tx_id,
                        _seq_num: _,
                    } => tx.send(tx_id).unwrap(),
                    _ => (),
                }
            }
        });
        barrierc.wait();
        ZmqThread { rx, stop, thread }
    }

    pub fn for_each<F>(self, op: F) -> usize
    where
        F: Fn(&Txid),
    {
        let mut counter = 0;
        loop {
            match self.rx.try_iter().next() {
                Some(tx_id) => {
                    op(&Txid::from(Hash::from_str(tx_id.as_str()).unwrap()));
                    counter += 1;
                }
                None => {
                    self.stop.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
        self.thread.join().expect("Error in tread");
        counter
    }
}
