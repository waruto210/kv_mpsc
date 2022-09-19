#[cfg(feature = "profile")]
use kv_mpsc::{async_channel, unwrap_ok_or, Message};
#[cfg(feature = "profile")]
const CAP: usize = 1000;
#[cfg(feature = "profile")]
const SEND: i32 = 500000;
#[cfg(feature = "profile")]
const THREADS: i32 = 16;

#[cfg(feature = "profile")]
async fn async_no_conflict() {
    let (tx, rx) = async_channel::bounded(CAP);
    let mut handles = vec![];
    for thread in 0..THREADS {
        let tx = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..SEND {
                let m = Message::single_key(thread * SEND + i, 1);
                unwrap_ok_or!(tx.send(m).await, err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    let start = tokio::time::Instant::now();
    for _ in 0..(SEND * THREADS) {
        let _drop = unwrap_ok_or!(rx.recv().await, err, panic!("{:?}", err));
    }
    println!("total time cost {:?}", start.elapsed());
    for handle in handles {
        let _drop = handle.await;
    }
    rx.print_stats();
}

fn main() {
    #[cfg(feature = "profile")]
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)
            .enable_time()
            .build()
            .unwrap();
        rt.block_on(async_no_conflict())
    }
}
