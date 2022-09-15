//! message store in async channel buffer

use std::collections::HashMap;

use tokio::sync::OwnedSemaphorePermit;

use crate::{
    buff::BuffMessage,
    message::{DeactivateKeys, Key},
};

/// the message type stored in buffer
pub(super) type StoredMessage<K, V, T> = (crate::Message<K, V, T>, OwnedSemaphorePermit);

impl<K: Key, V, T: DeactivateKeys<Key = K>> BuffMessage for StoredMessage<K, V, T> {
    type Key = K;

    /// is the message's key disjoint with an set of keys
    fn conflict_keys(
        &self, other: &HashMap<Self::Key, usize>,
    ) -> Option<Vec<&Self::Key>> {
        self.0.key.conflict_keys(other)
    }

    /// collect all keys to an owned vector
    /// applicable to both key types
    fn get_owned_keys(&self) -> Vec<Self::Key> {
        self.0.key.get_owned_keys()
    }
}
