//! A FIFO queue shared by sender and receiver

#[cfg(feature = "async")]
use tokio::sync::OwnedSemaphorePermit;

use crate::err::RecvError;
use crate::message::{Key, Message};
use crate::unwrap_some_or;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

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

#[cfg(not(feature = "async"))]
/// actual buffer item type
type BuffItemType<K, V> = Message<K, V>;
#[cfg(feature = "async")]
/// actual buffer item type
type BuffItemType<K, V> = (Message<K, V>, OwnedSemaphorePermit);

/// A fixed size buff
#[derive(Debug)]
pub(crate) struct Buff<K: Key, V> {
    /// FIFO queue buff
    buff: BuffType<BuffItemType<K, V>>,
    #[cfg(not(feature = "async"))]
    /// capacity of buff
    cap: usize,
    /// all current active keys
    activate_keys: HashSet<K>,
}

impl<K: Key, V> Buff<K, V> {
    #[cfg(not(feature = "async"))]
    /// new a buff with cap
    pub(crate) fn new(cap: usize) -> Self {
        Buff { buff: BuffType::new(), cap, activate_keys: HashSet::with_capacity(cap) }
    }

    #[cfg(feature = "async")]
    /// new a buff with cap
    pub(crate) fn new(cap: usize) -> Self {
        Buff { buff: BuffType::new(), activate_keys: HashSet::with_capacity(cap) }
    }

    /// push back to buff
    pub(crate) fn push_back(&mut self, m: BuffItemType<K, V>) {
        self.buff.push_back(m);
    }

    /// pop an unconflict message as front as possible
    pub(crate) fn pop_unconflict_front(
        &mut self,
    ) -> Result<BuffItemType<K, V>, RecvError> {
        let mut index: usize = 0;
        for msg in &self.buff {
            #[cfg(feature = "async")]
            let msg = &msg.0;
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

            #[cfg(not(feature = "async"))]
            for key in msg.get_owned_keys() {
                let _ = self.activate_keys.insert(key);
            }

            #[cfg(feature = "async")]
            for key in msg.0.get_owned_keys() {
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

    #[cfg(not(feature = "async"))]
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
    pub(crate) n_senders: usize,
    /// is the queue disconnected
    /// all sender gone or receiver closed
    pub(crate) disconnected: bool,
}
