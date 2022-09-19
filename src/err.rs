//! Errors used by ``kv_mpsc`` when Send and Receive

/// Error occurs only when channel is disconnected or
/// all messages are conflict
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
pub enum RecvError {
    /// All senders are clodes
    #[doc(alias = "closed")]
    Disconnected,
    /// All message's keys in buffer are conflict with active keys
    AllConflict,
}

/// Error occurs only when channel is disconnected
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
#[doc(alias = "closed")]
pub struct SendError<T>(pub T);
