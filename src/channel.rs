//! A mpsc channel that support key conflict resolution

use crate::err::{RecvError, SendError};
use crate::message::{Key, Message};
use crate::shared::{Buff, Shared, State};
use crate::{unwrap_ok_or, unwrap_some_or};
use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::{Arc, Condvar, Mutex};

/// A sync sender that will block when there no empty buff slot
#[derive(Debug)]
pub struct SyncSender<K: Key, V> {
    /// inner shared queue
    inner: Arc<Shared<K, V>>,
}

impl<K: Key, V> SyncSender<K, V> {
    /// send a message
    /// # Errors
    ///
    /// return `Err` if channel is disconnected
    #[inline]
    pub fn send(&self, message: Message<K, V>) -> Result<(), SendError<Message<K, V>>> {
        self.inner.send(message)
    }
}

impl<K: Key, V> Clone for SyncSender<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        let mut state = unwrap_ok_or!(self.inner.state.lock(), err, panic!("{:?}", err));
        let n_senders = state.n_senders;
        state.n_senders =
            unwrap_some_or!(n_senders.checked_add(1), panic!("too many senders"));
        drop(state);
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<K: Key, V> Drop for SyncSender<K, V> {
    #[inline]
    fn drop(&mut self) {
        let mut state = unwrap_ok_or!(self.inner.state.lock(), err, panic!("{:?}", err));
        let mut last_sender = false;
        let n_senders = state.n_senders;
        state.n_senders =
            unwrap_some_or!(n_senders.checked_sub(1), panic!("too many senders"));
        if state.n_senders == 0 {
            last_sender = true;
            state.disconnected = true;
        }
        drop(state);
        if last_sender {
            self.inner.fill.notify_one();
        }
    }
}

/// A sync receiver will block when buff is empty
#[derive(Debug)]
pub struct Receiver<K: Key, V> {
    /// shared FIFO queue
    inner: Arc<Shared<K, V>>,
    /// remove the auto `Sync` implentation, so only one
    /// thread can access the receiver
    _marker: std::marker::PhantomData<RefCell<()>>,
}

impl<K: Key, V> Receiver<K, V> {
    /// receive a message
    /// # Errors
    ///
    /// return `Err` if channel is all sender gone
    #[inline]
    pub fn recv(&self) -> Result<Message<K, V>, RecvError> {
        self.inner.recv().map(|mut msg| {
            msg.set_shared(Arc::<Shared<K, V>>::clone(&self.inner));
            msg
        })
    }
}

impl<K: Key, V> Drop for Receiver<K, V> {
    #[inline]
    fn drop(&mut self) {
        let state = self.inner.state.lock();
        let mut state = if let Ok(state) = state {
            state
        } else {
            panic!("lock error");
        };
        state.disconnected = true;
        drop(state);
        self.inner.empty.notify_all();
    }
}

/// A sync channel with capacity > 0
/// # Panics
///
/// panic is capicity less than zero
#[inline]
#[must_use]
#[doc(alias = "channel")]
pub fn bounded<K: Key, V>(cap: usize) -> (SyncSender<K, V>, Receiver<K, V>) {
    assert!(cap > 0, "The capacity of channel must be greater than 0");
    let inner = Arc::new(Shared {
        state: Mutex::new(State {
            buff: Buff::new(cap),
            n_senders: 1,
            disconnected: false,
        }),
        fill: Condvar::new(),
        empty: Condvar::new(),
    });
    let s = SyncSender { inner: Arc::<Shared<K, V>>::clone(&inner) };
    let r = Receiver { inner, _marker: std::marker::PhantomData };
    (s, r)
}
