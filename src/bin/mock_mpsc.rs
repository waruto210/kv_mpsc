use std::sync::atomic::Ordering::SeqCst;
use std::{
    sync::{atomic::AtomicI32, Arc},
    time::Duration,
};

use event_listener::Event;
use tokio::sync::Notify;
use tokio::time::Instant;

const TASK: usize = 4;
const NUM: usize = 2048;

async fn produce() {
    tokio::time::sleep(Duration::from_millis(40)).await;
}

async fn process() {
    tokio::time::sleep(Duration::from_millis(4)).await;
}

async fn try_get() -> bool {
    tokio::time::sleep(Duration::from_millis(1)).await;
    true
}

async fn mock_mpsc_notify() {
    let notify = Arc::new(Notify::new());
    let count = Arc::new(AtomicI32::new(0));
    let per_task = NUM / TASK;
    let mut handles = vec![];
    for _ in 0..TASK {
        let count = count.clone();
        let notify = notify.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..per_task {
                produce().await;
                count.fetch_add(1, SeqCst);
                notify.notify_one();
            }
        });
        handles.push(handle);
    }
    let start = Instant::now();
    let mut wait_count = 0;
    for _ in 0..NUM {
        while try_get().await && count.load(SeqCst) <= 0 {
            wait_count += 1;
            notify.notified().await;
        }
        count.fetch_sub(1, SeqCst);
        process().await;
    }
    println!("wait {} times", wait_count);
    println!("notify cost {:?}", start.elapsed());
    for handle in handles {
        let _ = handle.await;
    }
}

async fn mock_mpsc_event() {
    let notify = Arc::new(Event::new());
    let count = Arc::new(AtomicI32::new(0));
    let per_task = NUM / TASK;
    let mut handles = vec![];
    for _ in 0..TASK {
        let count = count.clone();
        let notify = notify.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..per_task {
                produce().await;
                count.fetch_add(1, SeqCst);
                notify.notify(1);
            }
        });
        handles.push(handle);
    }
    let start = Instant::now();
    let mut wait_count = 0;
    for _ in 0..NUM {
        loop {
            let listener = notify.listen();
            if try_get().await && count.load(SeqCst) <= 0 {
                wait_count += 1;
                listener.await;
            } else {
                listener.discard();
                break;
            }
        }
        count.fetch_sub(1, SeqCst);
        process().await;
    }
    println!("wait {} times", wait_count);
    println!("envent listener cost {:?}", start.elapsed());
    for handle in handles {
        let _ = handle.await;
    }
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async move {
        mock_mpsc_notify().await;
        mock_mpsc_event().await;
    });
}
