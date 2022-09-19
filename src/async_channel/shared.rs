//! async impl of shared queue between senders and receiver

//! A FIFO queue shared by sender and receiver

use tokio::sync::Semaphore;

use super::{Message, StoredMessage};
use crate::buff::State;
use crate::err::{RecvError, SendError};
use crate::message::{DeactivateKeys, Key};
use crate::unwrap_ok_or;
#[cfg(feature = "event_listener")]
use event_listener::Event;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "event_listener"))]
use tokio::sync::Notify;
#[cfg(feature = "profile")]
use tokio::time::Duration;

// it's safe here because all operations on rc will
// protect by the Mutex
#[allow(unsafe_code)]
unsafe impl<K: Key, V> Send for Shared<K, V> {}
#[cfg(not(feature = "profile"))]
#[allow(unsafe_code)]
unsafe impl<K: Key, V> Sync for Shared<K, V> {}

/// shared state between senders and receiver
#[derive(Debug)]
pub struct Shared<K: Key, V> {
    /// the queue state
    pub(crate) state: Mutex<State<StoredMessage<K, V>>>,
    /// semaphore that representes buffer resources
    pub(crate) slots: Arc<Semaphore>,
    /// notify receiver when send a message
    #[cfg(not(feature = "event_listener"))]
    pub(crate) notify_receiver: Notify,
    /// notify receiver when send a message
    #[cfg(feature = "event_listener")]
    pub(crate) notify_receiver: Event,
    /// try_recv time cost
    #[cfg(feature = "profile")]
    pub(crate) try_recv_cost: std::cell::UnsafeCell<Duration>,
    /// recv wait count
    #[cfg(feature = "profile")]
    pub(crate) wait_count: std::cell::UnsafeCell<usize>,
}

#[cfg(feature = "profile")]
#[allow(unsafe_code)]
unsafe impl<K: Key, V> Sync for Shared<K, V> {}

impl<K: Key, V> DeactivateKeys for Shared<K, V> {
    type Key = K;
    fn release_key<'a, I: IntoIterator<Item = &'a Self::Key>>(&'a self, keys: I) {
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        for k in keys {
            state.buff.deactivate_key(k);
        }
    }
}

impl<K: Key, V: Debug> Shared<K, V> {
    /// send a message
    pub(crate) async fn send(
        &self, message: Message<K, V>,
    ) -> Result<(), SendError<Message<K, V>>> {
        let slots = Arc::clone(&self.slots);
        let permit = unwrap_ok_or!(slots.acquire_owned().await, err, panic!("{:?}", err));
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        if state.disconnected {
            return Err(SendError(message));
        }
        state.buff.push_back((message, permit));
        drop(state);
        #[cfg(not(feature = "event_listener"))]
        self.notify_receiver.notify_one();
        #[cfg(feature = "event_listener")]
        self.notify_receiver.notify(1);
        Ok(())
    }

    /// try recv, return None if buff is empty
    fn try_recv(&self) -> Result<Option<Message<K, V>>, RecvError> {
        #[cfg(feature = "profile")]
        use std::time::Instant;
        #[cfg(feature = "profile")]
        let start = Instant::now();
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        // buffer is empty, wait sender to send
        if state.buff.is_empty() && !state.disconnected {
            #[cfg(feature = "profile")]
            {
                let cost = self.try_recv_cost.get();
                // only one receiver, it's safe
                #[allow(unsafe_code)]
                unsafe {
                    (*cost) += start.elapsed();
                }
            }
            return Ok(None);
        }

        if state.buff.is_empty() && state.disconnected {
            return Err(RecvError::Disconnected);
        }

        let (msg, _permit) = state.buff.pop_unconflict_front()?;
        #[cfg(feature = "profile")]
        {
            let cost = self.try_recv_cost.get();
            #[allow(unsafe_code)]
            unsafe {
                (*cost) += start.elapsed();
            }
        }
        Ok(Some(msg))
    }

    /// recv a message
    pub(crate) async fn recv(&self) -> Result<Message<K, V>, RecvError> {
        // for notify
        // use loop, consider
        // senders push x values, call x times `notify_one`, only a single permit is stored
        // receiver consume x values
        // when receiver calls `recv` for the n+1th time
        // receiver wait and there is a notify
        // receiver call `try_recv` again immediately and get None
        //
        // for event_listener, must call listen before try_recv to insert an entry to wait list
        // because it's notify will not store any permit when there is not task waiting, consider the following case:
        // rx try_recv, find empty -> tx send(tx all closed) -> tx notify -> rx wait, if no tx sends data after that
        // tx will wait forever

        loop {
            #[cfg(feature = "event_listener")]
            let listener = self.notify_receiver.listen();
            if let Some(msg) = self.try_recv()? {
                #[cfg(feature = "event_listener")]
                let _drop = listener.discard();
                return Ok(msg);
            }
            #[cfg(feature = "profile")]
            {
                let count = self.wait_count.get();
                #[allow(unsafe_code)]
                unsafe {
                    (*count) += 1;
                }
            }
            #[cfg(not(feature = "event_listener"))]
            self.notify_receiver.notified().await;
            #[cfg(feature = "event_listener")]
            listener.await;
        }
    }
}
