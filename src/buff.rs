//! A FIFO queue shared by sender and receiver

use crate::err::RecvError;
use crate::message::Key;
use crate::{unwrap_ok_or, unwrap_some_or};
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
use std::rc::Rc;
#[cfg(not(feature = "list"))]
/// actual buffer type
type BuffType<T> = VecDeque<T>;

/// A fixed size buff
#[derive(Debug)]
pub(crate) struct KeyedBuff<T: BuffMessage> {
    /// FIFO queue buff, store msgs that without conflitc
    ready: BuffType<T>,
    /// msgs that conflict with that key
    pending_on_key: HashMap<<T as BuffMessage>::Key, Vec<Rc<T>>>,
    /// capacity of buff
    cap: usize,
    /// size of buff now
    size: usize,
}

impl<T: BuffMessage> KeyedBuff<T> {
    /// new a buff with cap
    pub(crate) fn new(cap: usize) -> Self {
        KeyedBuff {
            ready: BuffType::with_capacity(cap),
            pending_on_key: HashMap::with_capacity(cap),
            cap,
            size: 0,
        }
    }

    /// push back to buff
    pub(crate) fn push_back(&mut self, m: T) {
        let size = unwrap_some_or!(self.size.checked_add(1), panic!("fatal error"));
        self.size = size;
        let keys = m.get_owned_keys();
        let mut pending = false;
        let msg = Rc::new(m);
        for k in keys {
            if let Some(pendings) = self.pending_on_key.get_mut(&k) {
                pending = true;
                pendings.push(Rc::clone(&msg));
            } else {
                let _drop = self.pending_on_key.insert(k, vec![]);
            }
        }
        if !pending {
            let inner_msg = unwrap_ok_or!(
                Rc::try_unwrap(msg),
                _,
                panic!("there should be only on ref")
            );
            self.ready.push_back(inner_msg);
        }
    }

    /// pop an unconflict message as front as possible
    pub(crate) fn pop_unconflict_front(&mut self) -> Result<T, RecvError> {
        if self.ready.is_empty() && self.size != 0 {
            Err(RecvError::AllConflict)
        } else {
            #[cfg(not(feature = "list"))]
            let msg = unwrap_some_or!(self.ready.pop_front(), panic!("fatal error"));
            #[cfg(feature = "list")]
            let msg = self.buff.remove(index);
            let size = unwrap_some_or!(self.size.checked_sub(1), panic!("fatal error"));
            self.size = size;
            Ok(msg)
        }
    }

    /// remove an active key
    pub(crate) fn deactivate_key<Q>(&mut self, key: &Q)
    where
        <T as BuffMessage>::Key: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(pending_msgs) = self.pending_on_key.get_mut(key) {
            if !pending_msgs.is_empty() {
                let first = pending_msgs.remove(0);
                if Rc::strong_count(&first) == 1 {
                    let msg = unwrap_ok_or!(
                        Rc::try_unwrap(first),
                        _,
                        panic!("there should be only on ref")
                    );
                    self.ready.push_back(msg);
                }
            }
            if pending_msgs.is_empty() {
                let _drop = self.pending_on_key.remove(key);
            }
        }
    }

    /// is buffer full
    pub(crate) fn is_full(&self) -> bool {
        self.size == self.cap
    }

    /// is buffer empty
    pub(crate) fn is_empty(&self) -> bool {
        self.size == 0
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
