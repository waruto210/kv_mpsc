//! A FIFO queue shared by sender and receiver

use super::Message;
use crate::buff::State;
use crate::err::{RecvError, SendError};
use crate::message::{DeactivateKeys, Key};
use crate::unwrap_ok_or;
use std::fmt::Debug;
use std::sync::{Condvar, Mutex, MutexGuard};

/// shared state between senders and receiver
#[derive(Debug)]
pub struct Shared<K: Key, V> {
    /// the queue state
    pub(crate) state: Mutex<State<Message<K, V>>>,
    /// cond var that representes fill a new message into queue
    pub(crate) fill: Condvar,
    /// cond var that representes consume a message from queue
    pub(crate) empty: Condvar,
}

impl<K: Key, V> DeactivateKeys for Shared<K, V> {
    type Key = K;
    /// release all keys
    fn release_key<'a, I: IntoIterator<Item = &'a Self::Key>>(&'a self, keys: I) {
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        for k in keys {
            state.buff.deactivate_key(k);
        }
    }
}

impl<K: Key, V> Shared<K, V> {
    /// wait for an empty buff slot to put a message
    fn acquire_send_slot(&self) -> MutexGuard<'_, State<Message<K, V>>> {
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        loop {
            if !state.buff.is_full() || state.disconnected {
                return state;
            }
            state = unwrap_ok_or!(self.empty.wait(state), err, panic!("{:?}", err));
        }
    }
    /// send a message
    pub(crate) fn send(
        &self, message: Message<K, V>,
    ) -> Result<(), SendError<Message<K, V>>> {
        let mut state = self.acquire_send_slot();
        if state.disconnected {
            return Err(SendError(message));
        }
        state.buff.push_back(message);
        drop(state);
        self.fill.notify_one();
        Ok(())
    }

    /// recv a message
    pub(crate) fn recv(&self) -> Result<Message<K, V>, RecvError> {
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        if state.buff.is_empty() && !state.disconnected {
            state = unwrap_ok_or!(self.fill.wait(state), err, panic!("{:?}", err));
        }
        if state.buff.is_empty() && state.disconnected {
            return Err(RecvError::Disconnected);
        }
        let value = state.buff.pop_unconflict_front();
        // notify the blocked sender corrospend to this message
        drop(state);
        // notify other blocked sender
        self.empty.notify_one();
        value
    }
}
