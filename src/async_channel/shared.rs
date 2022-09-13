//! async impl of shared queue between senders and receiver

//! A FIFO queue shared by sender and receiver

use tokio::sync::{Notify, Semaphore};

use crate::err::{RecvError, SendError};
use crate::message::{Key, Message};
use crate::state::State;
use crate::unwrap_ok_or;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

/// shared state between senders and receiver
#[derive(Debug)]
pub(crate) struct Shared<K: Key, V> {
    /// the queue state
    pub(crate) state: Mutex<State<K, V>>,
    /// semaphore that representes buffer resources
    pub(crate) slots: Arc<Semaphore>,
    /// notify receiver when send a message
    pub(crate) notify_receiver: Notify,
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
        self.notify_receiver.notify_one();
        Ok(())
    }

    /// try recv, return None if buff is empty
    fn try_recv(&self) -> Result<Option<Message<K, V>>, RecvError> {
        let mut state = unwrap_ok_or!(self.state.lock(), err, panic!("{:?}", err));
        // buffer is empty, wait sender to send
        if state.buff.is_empty() && !state.disconnected {
            return Ok(None);
        }

        if state.buff.is_empty() && state.disconnected {
            return Err(RecvError::Disconnected);
        }

        let (msg, _permit) = state.buff.pop_unconflict_front()?;
        Ok(Some(msg))
    }

    /// recv a message
    pub(crate) async fn recv(&self) -> Result<Message<K, V>, RecvError> {
        let future = self.notify_receiver.notified();
        tokio::pin!(future);
        // use loop, consider
        // senders push x values, call x times `notify_one`, only a single permit is stored
        // receiver consume x values
        // when receiver calls `recv` for the n+1th time
        // receiver wait and there is a notify
        // receiver call `try_recv` again immediately and get None
        loop {
            if let Some(msg) = self.try_recv()? {
                return Ok(msg);
            }
            future.as_mut().await;
            future.set(self.notify_receiver.notified());
        }
    }
}
