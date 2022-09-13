//! S simple benchmark
use criterion::{criterion_group, criterion_main, Criterion};
use kv_mpsc::{bounded, unwrap_ok_or, Message, RecvError};

#[inline]
#[cfg(not(feature = "async"))]
fn std_mpsc() {
    let cap = 1000;
    let send = 10000;
    let threads = 16;
    let (tx, rx) = std::sync::mpsc::sync_channel(cap);
    let mut handles = vec![];
    for thread in 0..threads {
        let tx = tx.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..send {
                let m = Message::single_key(thread * send + i, 1);
                unwrap_ok_or!(tx.send(m), err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..send * threads {
        let _drop = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
    }

    for handle in handles {
        let _drop = handle.join();
    }
}

#[inline]
#[cfg(not(feature = "async"))]
fn no_conflict() {
    let cap = 1000;
    let send = 10000;
    let threads = 16;
    let (tx, rx) = bounded(cap);
    let mut handles = vec![];
    for thread in 0..threads {
        let tx = tx.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..send {
                let m = Message::single_key(thread * send + i, 1);
                unwrap_ok_or!(tx.send(m), err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..send * threads {
        let _drop = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
    }

    for handle in handles {
        let _drop = handle.join();
    }
}

#[inline]
#[cfg(not(feature = "async"))]
fn with_conflict() {
    let cap = 1000;
    let send = 10000;
    let threads = 16;
    let mut handles = vec![];
    let (tx, rx) = bounded(cap);
    for _ in 0..threads {
        let tx = tx.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..send {
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
        if count == send * threads {
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

    let cap = 1000;
    let send = 10000;
    let threads = 16;
    let (tx, mut rx) = tokio::sync::mpsc::channel(cap);
    let mut handles = vec![];
    for thread in 0..threads {
        let tx = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..send {
                let m = Message::single_key(thread * send + i, 1);
                unwrap_ok_or!(tx.send(m).await, err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..send * threads {
        let _drop = unwrap_some_or!(rx.recv().await, panic!("fatal error"));
    }

    for handle in handles {
        let _drop = handle.await;
    }
}

#[inline]
#[cfg(feature = "async")]
async fn no_conflict() {
    let cap = 1000;
    let send = 10000;
    let threads = 16;
    let (tx, rx) = bounded(cap);
    let mut handles = vec![];
    for thread in 0..threads {
        let tx = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..send {
                let m = Message::single_key(thread * send + i, 1);
                unwrap_ok_or!(tx.send(m).await, err, panic!("{:?}", err));
            }
        });
        handles.push(handle);
    }
    for _ in 0..send * threads {
        let _drop = unwrap_ok_or!(rx.recv().await, err, panic!("{:?}", err));
    }

    for handle in handles {
        let _drop = handle.await;
    }
}

#[inline]
#[cfg(feature = "async")]
async fn with_conflict() {
    let cap = 1000;
    let send = 10000;
    let threads = 16;
    let mut handles = vec![];
    let (tx, rx) = bounded(cap);
    for _ in 0..threads {
        let tx = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..send {
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
        if count == send * threads {
            break;
        }
    }

    for handle in handles {
        let _drop = handle.await;
    }
}

#[cfg(not(feature = "async"))]
pub fn send_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("MultiThread Send and Recv");
    group.bench_function("std mpsc", |b| b.iter(std_mpsc));
    group.bench_function("kv_mpsc no conflict", |b| b.iter(no_conflict));
    group.bench_function("kv_mpsc with conflict", |b| b.iter(with_conflict));
    group.finish();
}

#[cfg(feature = "async")]
pub fn send_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("MultiThread Send and Recv");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .build()
        .unwrap();
    group.bench_function("tokio mpsc", |b| b.to_async(&rt).iter(tokio_mpsc));
    group.bench_function("kv_mpsc no conflict", |b| b.to_async(&rt).iter(no_conflict));
    group
        .bench_function("kv_mpsc with conflict", |b| b.to_async(&rt).iter(with_conflict));
    group.finish();
}

criterion_group!(benches, send_recv);
criterion_main!(benches);
