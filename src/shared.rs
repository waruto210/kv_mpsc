//! A FIFO queue shared by sender and receiver

use crate::err::{RecvError, SendError};
use crate::message::{Key, Message};
use crate::{unwrap_ok_or, unwrap_some_or};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Condvar, Mutex, MutexGuard};

#[cfg(feature = "list")]
use std::collections::LinkedList;
#[cfg(feature = "list")]
/// actual buffer type
type BuffType<T> = LinkedList<T>;
#[cfg(not(feature = "list"))]
use std::collections::VecDeque;
#[cfg(not(feature = "list"))]
/// actual buffer type
type BuffType<T> = VecDeque<T>;

/// A fixed size buff
#[derive(Debug)]
pub(crate) struct Buff<K: Key, V> {
    /// FIFO queue buff
    buff: BuffType<Message<K, V>>,
    /// capacity of buff
    cap: usize,
    /// all current active keys
    activate_keys: HashSet<K>,
}

impl<K: Key, V> Buff<K, V> {
    /// new a buff with cap
    pub(crate) fn new(cap: usize) -> Self {
        Buff { buff: BuffType::new(), cap, activate_keys: HashSet::with_capacity(cap) }
    }

    /// push back to buff
    pub(crate) fn push_back(&mut self, m: Message<K, V>) {
        self.buff.push_back(m);
    }

    /// pop an unconflict message as front as possible
    pub(crate) fn pop_unconflict_front(&mut self) -> Result<Message<K, V>, RecvError> {
        let mut index: usize = 0;
        for msg in &self.buff {
            if msg.is_disjoint(&self.activate_keys) {
                break;
            }
            let new_index = unwrap_some_or!(index.checked_add(1), panic!("fatal error"));
            index = new_index;
        }
        if index >= self.buff.len() {
            Err(RecvError::AllConflict)
        } else {
            #[cfg(not(feature = "list"))]
            let msg = unwrap_some_or!(self.buff.remove(index), panic!("fatal error"));
            #[cfg(feature = "list")]
            let msg = self.buff.remove(index);
            for key in msg.get_owned_keys() {
                let _ = self.activate_keys.insert(key);
            }
            Ok(msg)
        }
    }

    /// remove an active key
    pub(crate) fn remove_active_key<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let _ = self.activate_keys.remove(key);
    }

    /// is buffer full
    pub(crate) fn is_full(&self) -> bool {
        self.buff.len() == self.cap
    }

    /// is buffer empty
    pub(crate) fn is_empty(&self) -> bool {
        self.buff.len() == 0
    }
}

/// the state of queue
#[derive(Debug)]
pub(crate) struct State<K: Key, V> {
    /// queue buffer
    pub(crate) buff: Buff<K, V>,
    /// n senders of the queue
    pub(crate) n_senders: u32,
    /// is the queue disconnected
    /// all sender gone or receiver closed
    pub(crate) disconnected: bool,
}

/// shared state between senders and receiver
#[derive(Debug)]
pub(crate) struct Shared<K: Key, V> {
    /// the queue state
    pub(crate) state: Mutex<State<K, V>>,
    /// cond var that representes fill a new message into queue
    pub(crate) fill: Condvar,
    /// cond var that representes consume a message from queue
    pub(crate) empty: Condvar,
}

impl<K: Key, V> Shared<K, V> {
    /// wait for an empty buff slot to put a message
    fn acquire_send_slot(&self) -> MutexGuard<'_, State<K, V>> {
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
