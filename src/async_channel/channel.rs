//! Async mpsc channel that support key conflict resolution

use super::shared::Shared;
use super::Message;
use crate::buff::{KeyedBuff, State};
use crate::err::{RecvError, SendError};
use crate::message::Key;
use crate::{unwrap_ok_or, unwrap_some_or};
#[cfg(feature = "event_listener")]
use event_listener::Event;
use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "event_listener"))]
use tokio::sync::Notify;
use tokio::sync::Semaphore;

/// A bounded sender that will wait when there is no empty buff slot
#[derive(Debug)]
pub struct BoundedSender<K: Key, V> {
    /// inner shared queue
    inner: Arc<Shared<K, V>>,
}

impl<K: Key, V: Debug> BoundedSender<K, V> {
    /// send a message
    /// # Errors
    ///
    /// return `Err` if channel is disconnected
    #[inline]
    pub async fn send(
        &self, message: Message<K, V>,
    ) -> Result<(), SendError<Message<K, V>>> {
        self.inner.send(message).await
    }
}

impl<K: Key, V> Clone for BoundedSender<K, V> {
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

impl<K: Key, V> Drop for BoundedSender<K, V> {
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
            #[cfg(not(feature = "event_listener"))]
            self.inner.notify_receiver.notify_one();
            #[cfg(feature = "event_listener")]
            self.inner.notify_receiver.notify(1);
        }
    }
}

/// A sync receiver will wait when buff is empty
#[derive(Debug)]
pub struct Receiver<K: Key, V> {
    /// shared FIFO queue
    inner: Arc<Shared<K, V>>,
    /// remove the auto `Sync` implentation, so only one
    /// thread can access the receiver
    _marker: std::marker::PhantomData<RefCell<()>>,
}

impl<K: Key, V: Debug> Receiver<K, V> {
    /// receive a message
    /// # Errors
    ///
    /// return `Err` if channel is all sender gone
    #[inline]
    pub async fn recv(&self) -> Result<Message<K, V>, RecvError> {
        self.inner.recv().await.map(|mut msg| {
            msg.set_shared(Arc::<Shared<K, V>>::clone(&self.inner));
            msg
        })
    }

    /// print stats
    #[cfg(feature = "profile")]
    #[inline]
    #[allow(unsafe_code)]
    pub fn print_stats(&self) {
        // it's safe because there is only one receiver and sender whill not modify these
        unsafe {
            println!(
                "wait count {}, try_recv cost time {:?}",
                *self.inner.wait_count.get(),
                *self.inner.try_recv_cost.get(),
            )
        }
    }
}

impl<K: Key, V> Drop for Receiver<K, V> {
    #[inline]
    fn drop(&mut self) {
        let mut state =
            unwrap_ok_or!(self.inner.state.lock(), err, panic!("lock err {:?}", err));
        state.disconnected = true;
        drop(state);
        // pending senders will get a permit immediately
        // and check the `state.disconnected`, then return Err
        // strictly speaking, add one permit is enough
        self.inner.slots.add_permits(1);
    }
}

/// A sync channel with capacity > 0
/// # Panics
///
/// panic is capicity less than zero
#[inline]
#[must_use]
#[doc(alias = "channel")]
pub fn bounded<K: Key, V>(cap: usize) -> (BoundedSender<K, V>, Receiver<K, V>) {
    assert!(cap > 0, "The capacity of channel must be greater than 0");
    let inner = Arc::new(Shared {
        state: Mutex::new(State {
            buff: KeyedBuff::new(cap),
            n_senders: 1,
            disconnected: false,
        }),
        slots: Arc::new(Semaphore::new(cap)),
        #[cfg(not(feature = "event_listener"))]
        notify_receiver: Notify::new(),
        #[cfg(feature = "event_listener")]
        notify_receiver: Event::new(),
        #[cfg(feature = "profile")]
        try_recv_cost: std::cell::UnsafeCell::new(tokio::time::Duration::new(0, 0)),
        #[cfg(feature = "profile")]
        wait_count: std::cell::UnsafeCell::new(0),
    });
    let s = BoundedSender { inner: Arc::<Shared<K, V>>::clone(&inner) };
    let r = Receiver { inner, _marker: std::marker::PhantomData };
    (s, r)
}
