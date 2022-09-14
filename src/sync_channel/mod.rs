//! Sync impl of `kv_mpsc`
//!
//!
//! # Examples
//!
//! Simple usage:
//! ```rust
//! use std::thread;
//! use kv_mpsc::sync_channel::bounded;
//! use kv_mpsc::Message;
//!
//! // create a simple channel
//! let (tx, rx) = bounded(1);
//! thread::spawn(move || {
//!     let msg = Message::single_key(1, 1);
//!     tx.send(msg).unwrap();
//! });
//! let msg = rx.recv().unwrap();
//! assert_eq!(msg.get_single_key().unwrap(), &1);
//! assert_eq!(msg.get_value(), &1);
//!
//! ```
//!
//!
//! Key conflict
//!
//! ```rust
//! use std::thread;
//! use kv_mpsc::sync_channel::bounded;
//! use kv_mpsc::Message;
//! use kv_mpsc::RecvError;
//!
//! // create a simple channel
//! let (tx, rx) = bounded(1);
//! thread::spawn(move || {
//!     let msg = Message::single_key(1, 1);
//!     tx.send(msg).unwrap();
//!     let msg = Message::single_key(1, 2);
//!     tx.send(msg).unwrap();
//! });
//! let msg = rx.recv().unwrap();
//! assert_eq!(msg.get_single_key().unwrap(), &1);
//! assert_eq!(msg.get_value(), &1);
//! assert_eq!(rx.recv(), Err(RecvError::AllConflict));
//! drop(msg);
//! let msg = rx.recv().unwrap();
//! assert_eq!(msg.get_single_key().unwrap(), &1);
//! assert_eq!(msg.get_value(), &2);
//! assert_eq!(rx.recv(), Err(RecvError::Disconnected));
//!
//! ```

mod channel;

pub use channel::{bounded, BoundedSender, Receiver};
mod shared;

/// the real messge used in sync channel
type Message<K, V> = crate::Message<K, V, shared::Shared<K, V>>;

#[cfg(test)]
mod test {

    use crate::sync_channel::bounded;
    use crate::{unwrap_ok_or, unwrap_some_or, Message, RecvError, SendError};
    use std::{
        collections::HashSet,
        iter::FromIterator,
        sync::{atomic::AtomicBool, Arc},
        thread,
    };

    #[test]
    fn test_sender_close() {
        let cap = 10;
        let (tx, rx) = bounded(cap);
        let handle = thread::spawn(move || {
            let msg = Message::single_key(1, 1);
            let _drop = tx.send(msg);
        });
        let _drop = handle.join();
        assert_eq!(rx.recv(), Ok(Message::single_key(1, 1)));
        assert_eq!(rx.recv(), Err(RecvError::Disconnected));
    }

    #[test]
    fn test_receiver_close() {
        let cap = 10;
        let (tx, rx) = bounded(cap);
        drop(rx);
        let msg = Message::single_key(1, 1);
        assert_eq!(tx.send(msg), Err(SendError(Message::single_key(1, 1))));
    }

    #[test]
    fn test_no_conflict_single_key_send_recv() {
        let cap = 10;
        let send = 100;
        let threads = 10;
        let (tx, rx) = bounded(cap);
        let mut handles = vec![];
        for thread_id in 0..threads {
            let tx = tx.clone();
            let handle = thread::spawn(move || {
                for i in 0..send {
                    let msg =
                        Message::single_key(thread_id * send + i, thread_id * send + i);
                    let _drop = tx.send(msg);
                }
            });
            handles.push(handle);
        }
        let mut sum = 0;
        for _ in 0..(send * threads) {
            // all keys no conflict, so it's ok to unwarp
            let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            assert_eq!(
                unwrap_some_or!(msg.get_single_key(), panic!("fatal error")),
                msg.get_value()
            );
            sum += msg.get_value();
        }
        assert_eq!((0..threads * send).sum::<i32>(), sum);
        // make sure all tx is droped
        for handle in handles {
            let _drop = handle.join();
        }
    }

    #[test]
    fn test_no_conflict_multiple_keys_send_recv() {
        let cap = 10;
        let send = 100;
        let threads = 10;
        let (tx, rx) = bounded(cap);
        let mut handles = vec![];
        for thread_id in 0..threads {
            let tx = tx.clone();
            let handle = thread::spawn(move || {
                for i in 0..send {
                    let key1 = thread_id * send + i;
                    let msg = Message::multiple_keys(
                        vec![key1, key1 * 2],
                        thread_id * send + i,
                    );
                    let _drop = tx.send(msg);
                }
            });
            handles.push(handle);
        }
        let mut sum = 0;
        for _ in 0..(send * threads) {
            // all keys no conflict, so it's ok to unwarp
            let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            sum += msg.get_value();
        }
        assert_eq!((0..threads * send).sum::<i32>(), sum);
        // make sure all tx is droped
        for handle in handles {
            let _drop = handle.join();
        }
    }

    #[test]
    fn test_conflict_single_key_send_recv() {
        use std::sync::atomic::Ordering::SeqCst;
        // the test case is as follow
        // tx send 10 k1 msgs
        // rx recv a k1 msg, then recv will return `AllConflict`
        // tx send 1 k2 msg
        // rx recv k2, and then drop the k1 msg above
        // tx send 1 k1 msg
        // rx recv k1, and then drop the k1 msg above
        // rx recv remained k1 msgs and drop them
        // rx drop the k2 msg above and recv the last k2 msg
        let cap = 10;
        let (tx, rx) = bounded(cap);
        let can_send = Arc::new(AtomicBool::new(false));
        let key1 = 1;
        let key2 = 2;
        let send_new_key = Arc::<AtomicBool>::clone(&can_send);
        let handle = thread::spawn(move || {
            for i in 0..cap {
                let msg = Message::single_key(key1, i);
                let _drop = tx.send(msg);
            }
            while !send_new_key.load(SeqCst) {}
            let msg = Message::single_key(key2, cap);
            let _drop = tx.send(msg);
            send_new_key.store(false, SeqCst);

            while !send_new_key.load(SeqCst) {}
            let msg1 = Message::single_key(key2, cap);
            let _drop1 = tx.send(msg1);
            send_new_key.store(false, SeqCst);
        });

        let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg.get_single_key(), panic!("fatal error")), &key1);
        assert_eq!(rx.recv(), Err(RecvError::AllConflict));

        can_send.store(true, SeqCst);
        while can_send.load(SeqCst) {}
        let msg2 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg2.get_single_key(), panic!("fatal error")), &key2);
        drop(msg);

        can_send.store(true, SeqCst);
        while can_send.load(SeqCst) {}

        let msg3 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg3.get_single_key(), panic!("fatal error")), &key1);
        assert_eq!(rx.recv(), Err(RecvError::AllConflict));
        drop(msg3);

        let remained_key1 = cap - 2;
        for _ in 0..remained_key1 {
            let msg4 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            assert_eq!(
                unwrap_some_or!(msg4.get_single_key(), panic!("fatal error")),
                &key1
            );
            assert_eq!(rx.recv(), Err(RecvError::AllConflict));
            drop(msg4);
        }
        drop(msg2);
        let msg5 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg5.get_single_key(), panic!("fatal error")), &key2);
        let _drop = handle.join();
    }

    #[test]
    fn test_conflict_multiple_key_send_recv() {
        let cap = 10;
        let (tx, rx) = bounded(cap);

        let keys = (0..cap).collect::<Vec<usize>>();
        let recv_keys = keys.clone();
        let handle = thread::spawn(move || {
            // All keyset will conflict with the keyset after it
            for i in 0..keys.len() {
                let msg = Message::multiple_keys(
                    unwrap_some_or!(keys.get(0..=i), panic!("fatal error")).to_vec(),
                    i,
                );
                let _drop = tx.send(msg);
            }
        });

        let _drop = handle.join();

        for i in 0..cap {
            let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            assert_eq!(
                unwrap_some_or!(msg.get_key_set(), panic!("fatal error")),
                &HashSet::from_iter(
                    unwrap_some_or!(recv_keys.get(0..=i), panic!("fatal error")).to_vec()
                )
            );
            assert_eq!(
                rx.recv(),
                if i < cap - 1 {
                    Err(RecvError::AllConflict)
                } else {
                    Err(RecvError::Disconnected)
                }
            );
        }
    }
}
