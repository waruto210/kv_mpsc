//! S simple benchmark
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(feature = "async")]
use kv_mpsc::async_channel;
use kv_mpsc::{sync_channel, unwrap_ok_or, Message, RecvError};

const CAP: usize = 1000;
const SEND: i32 = 10000;
const THREADS: i32 = 16;

#[inline]
fn std_mpsc() {
    let (tx, rx) = std::sync::mpsc::sync_channel(CAP);
    let mut handles = vec![];
    for thread in 0..THREADS {
        let tx = tx.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..SEND {
                let m = (thread * SEND + i, 1, Option::None::<Arc<i32>>);
                unwrap_ok_or!(tx.send(m), err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..SEND * THREADS {
        let _drop = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
    }

    for handle in handles {
        let _drop = handle.join();
    }
}

#[inline]
fn no_conflict() {
    let (tx, rx) = sync_channel::bounded(CAP);
    let mut handles = vec![];
    for thread in 0..THREADS {
        let tx = tx.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..SEND {
                let m = Message::single_key(thread * SEND + i, 1);
                unwrap_ok_or!(tx.send(m), err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..SEND * THREADS {
        let _drop = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
    }

    for handle in handles {
        let _drop = handle.join();
    }
}

#[inline]
fn with_conflict() {
    let mut handles = vec![];
    let (tx, rx) = sync_channel::bounded(CAP);
    for _ in 0..THREADS {
        let tx = tx.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..SEND {
                let m = Message::single_key(i, 1);
                unwrap_ok_or!(tx.send(m), err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }

    let mut msgs = vec![];
    let mut count = 0;
    loop {
        let ret = rx.recv();
        match ret {
            Ok(msg) => {
                msgs.push(msg);
                count += 1;
            }
            Err(e) => match e {
                RecvError::AllConflict => {
                    while let Some(msg) = msgs.pop() {
                        drop(msg);
                    }
                }
                RecvError::Disconnected => {
                    panic!("unexpected disconnected");
                }
                _ => {}
            },
        }
        if count == SEND * THREADS {
            break;
        }
    }

    for handle in handles {
        let _drop = handle.join();
    }
}

#[inline]
#[cfg(feature = "async")]
async fn tokio_mpsc() {
    use kv_mpsc::unwrap_some_or;

    let (tx, mut rx) = tokio::sync::mpsc::channel(CAP);
    let mut handles = vec![];
    for thread in 0..THREADS {
        let tx = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..SEND {
                let m = (thread * SEND + i, 1, Option::None::<Arc<i32>>);
                unwrap_ok_or!(tx.send(m).await, err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..SEND * THREADS {
        let _drop = unwrap_some_or!(rx.recv().await, panic!("fatal error"));
    }

    for handle in handles {
        let _drop = handle.await;
    }
}

#[inline]
#[cfg(feature = "async")]
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
    for _ in 0..SEND * THREADS {
        let _drop = unwrap_ok_or!(rx.recv().await, err, panic!("{:?}", err));
    }

    for handle in handles {
        let _drop = handle.await;
    }
}

#[inline]
#[cfg(feature = "async")]
async fn async_with_conflict() {
    let mut handles = vec![];
    let (tx, rx) = async_channel::bounded(CAP);
    for _ in 0..THREADS {
        let tx = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..SEND {
                let m = Message::single_key(i, 1);
                unwrap_ok_or!(tx.send(m).await, err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }

    let mut msgs = vec![];
    let mut count = 0;
    loop {
        let ret = rx.recv().await;
        match ret {
            Ok(msg) => {
                msgs.push(msg);
                count += 1;
            }
            Err(e) => match e {
                RecvError::AllConflict => {
                    if !msgs.is_empty() {
                        msgs.remove(0);
                    }
                }
                RecvError::Disconnected => {
                    panic!("unexpected disconnected");
                }
                _ => {}
            },
        }
        if count == SEND * THREADS {
            break;
        }
    }

    for handle in handles {
        let _drop = handle.await;
    }
}

pub fn send_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync send_recv");
    group.bench_function("std mpsc", |b| b.iter(std_mpsc));
    group.bench_function("kv_mpsc no conflict", |b| b.iter(no_conflict));
    group.bench_function("kv_mpsc with conflict", |b| b.iter(with_conflict));
    group.finish();
}

#[cfg(feature = "async")]
pub fn async_send_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("async send_recv");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .build()
        .unwrap();
    // group.bench_function("tokio mpsc", |b| b.to_async(&rt).iter(tokio_mpsc));
    // group.bench_function("async kv_mpsc no conflict", |b| {
    //     b.to_async(&rt).iter(async_no_conflict)
    // });
    group.bench_function("async kv_mpsc with conflict", |b| {
        b.to_async(&rt).iter(async_with_conflict)
    });
    group.finish();
}

#[cfg(not(feature = "async"))]
criterion_group!(benches, send_recv);
#[cfg(feature = "async")]
criterion_group!(benches, async_send_recv);
criterion_main!(benches);
