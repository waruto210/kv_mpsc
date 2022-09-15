//! A FIFO queue shared by sender and receiver

use crate::err::RecvError;
use crate::message::Key;
use crate::unwrap_some_or;
use std::borrow::Borrow;
use std::collections::HashMap;
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

/// A fixed size buff
#[derive(Debug)]
pub(crate) struct KeyedBuff<T: BuffMessage> {
    /// FIFO queue buff
    buff: BuffType<T>,
    /// capacity of buff
    cap: usize,
    /// keys is current active key, value point to first msg
    /// in buff that conflict with that key, cap means None
    activate_keys: HashMap<<T as BuffMessage>::Key, usize>,
    /// curr scan start position
    curr: usize,
}

impl<T: BuffMessage> KeyedBuff<T> {
    /// new a buff with cap
    pub(crate) fn new(cap: usize) -> Self {
        KeyedBuff {
            buff: BuffType::new(),
            cap,
            activate_keys: HashMap::with_capacity(cap),
            curr: 0,
        }
    }

    /// push back to buff
    pub(crate) fn push_back(&mut self, m: T) {
        self.buff.push_back(m);
    }

    /// pop an unconflict message as front as possible
    pub(crate) fn pop_unconflict_front(&mut self) -> Result<T, RecvError> {
        let mut index: usize = self.curr;
        for msg in self.buff.iter().skip(index) {
            if let Some(conflict_keys) = msg.conflict_keys(&self.activate_keys) {
                for key in conflict_keys {
                    if let Some(v) = self.activate_keys.get_mut(key) {
                        if index < *v {
                            *v = index;
                        }
                    }
                }
            } else {
                break;
            }
            let new_index = unwrap_some_or!(index.checked_add(1), panic!("fatal error"));
            index = new_index;
        }
        self.curr = index;
        if index >= self.buff.len() {
            Err(RecvError::AllConflict)
        } else {
            #[cfg(not(feature = "list"))]
            let msg = unwrap_some_or!(self.buff.remove(index), panic!("fatal error"));
            #[cfg(feature = "list")]
            let msg = self.buff.remove(index);

            for key in msg.get_owned_keys() {
                let _ = self.activate_keys.insert(key, self.cap);
            }
            Ok(msg)
        }
    }

    /// remove an active key
    pub(crate) fn deactivate_key<Q>(&mut self, key: &Q)
    where
        <T as BuffMessage>::Key: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(index) = self.activate_keys.remove(key) {
            if index < self.cap && index < self.curr {
                // point to the first msg that conflict with that key
                self.curr = index;
            }
        }
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

/// A trait that represents keyed message stored in buffer
pub(crate) trait BuffMessage {
    /// key type
    type Key: Key;

    /// is the message's key disjoint with an set of keys
    fn conflict_keys(&self, other: &HashMap<Self::Key, usize>)
        -> Option<Vec<&Self::Key>>;

    /// collect all keys to an owned vector
    /// applicable to both key types
    fn get_owned_keys(&self) -> Vec<Self::Key>;
}

/// The state of queue
#[derive(Debug)]
pub(crate) struct State<T: BuffMessage> {
    /// queue buffer
    pub(crate) buff: KeyedBuff<T>,
    /// n senders of the queue
    pub(crate) n_senders: usize,
    /// is the queue disconnected
    /// all sender gone or receiver closed
    pub(crate) disconnected: bool,
}
