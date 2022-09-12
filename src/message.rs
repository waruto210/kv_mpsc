//! A message contains mutiple keys and single value

use crate::shared::Shared;
use crate::unwrap_ok_or;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::FromIterator;
use std::sync::Arc;

/// Trait bound for the message key
pub trait Key: Eq + Hash + Clone {}

impl<T: Eq + Hash + Clone> Key for T {}

/// Key of a message
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeySet<K: Key> {
    /// single key
    Single(K),
    /// mutiple keys
    Multiple(HashSet<K>),
}

impl<K: Key> KeySet<K> {
    /// is key disjoint with an set of keys
    pub(crate) fn is_disjoint(&self, other: &HashSet<K>) -> bool {
        match *self {
            Self::Single(ref k) => !other.contains(k),
            Self::Multiple(ref keys) => keys.is_disjoint(other),
        }
    }

    /// does it containes multiple keys
    pub(crate) fn is_multiple(&self) -> bool {
        !matches!(*self, Self::Single(_))
    }

    /// convert keys to owned a vec
    pub(crate) fn get_owned_keys(&self) -> Vec<K> {
        match *self {
            Self::Single(ref k) => vec![k.clone()],
            Self::Multiple(ref keys) => keys.iter().map(Clone::clone).collect(),
        }
    }

    /// get single key if the key is
    pub(crate) fn get_single_key(&self) -> Option<&K> {
        match *self {
            Self::Single(ref k) => Some(k),
            Self::Multiple(_) => None,
        }
    }

    /// get mutiple keyset if the key is
    pub(crate) fn get_key_set(&self) -> Option<&HashSet<K>> {
        match *self {
            Self::Multiple(ref keys) => Some(keys),
            Self::Single(_) => None,
        }
    }
}
///  Message type in channel
pub struct Message<K: Key, V> {
    /// message key
    key: KeySet<K>,
    /// messasge value
    value: V,
    /// use to control the active keys
    shared: Option<Arc<Shared<K, V>>>,
}

impl<K: Key, V: PartialEq> PartialEq for Message<K, V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.value == other.value
    }
}

impl<K: Key + Debug, V: Debug> Debug for Message<K, V> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("key", &self.key)
            .field("value", &self.value)
            .finish()
    }
}

impl<K: Key, V> Drop for Message<K, V> {
    #[inline]
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            let mut state = unwrap_ok_or!(shared.state.lock(), err, panic!("{:?}", err));
            match self.key {
                KeySet::Single(ref k) => state.buff.remove_active_key(k),
                KeySet::Multiple(ref keys) => {
                    for k in keys.iter() {
                        state.buff.remove_active_key(k);
                    }
                }
            }
        }
    }
}

impl<K: Key, V> Message<K, V> {
    /// new a message
    #[inline]
    pub fn multiple_keys<I>(keys: I, value: V) -> Self
    where
        I: IntoIterator<Item = K>,
    {
        Message { key: KeySet::Multiple(HashSet::from_iter(keys)), value, shared: None }
    }

    /// set the share queue
    #[inline]
    pub(crate) fn set_shared(&mut self, shared: Arc<Shared<K, V>>) {
        self.shared = Some(shared);
    }

    /// is the message's key disjoint with an set of keys
    pub(crate) fn is_disjoint(&self, other: &HashSet<K>) -> bool {
        self.key.is_disjoint(other)
    }

    /// new a single key message
    #[inline]
    pub fn single_key(key: K, value: V) -> Self {
        Message { key: KeySet::Single(key), value, shared: None }
    }

    /// is the message's keyset containes multiple keys
    #[inline]
    pub fn is_multiple(&self) -> bool {
        self.key.is_multiple()
    }

    /// collect all keys to an owned vector
    /// applicable to both key types
    #[inline]
    pub fn get_owned_keys(&self) -> Vec<K> {
        self.key.get_owned_keys()
    }

    /// return a ref to single key or None
    #[inline]
    pub fn get_single_key(&self) -> Option<&K> {
        self.key.get_single_key()
    }

    /// return a ref to keyset
    #[inline]
    pub fn get_key_set(&self) -> Option<&HashSet<K>> {
        self.key.get_key_set()
    }

    /// get message value
    #[inline]
    pub fn get_value(&self) -> &V {
        &self.value
    }
}
