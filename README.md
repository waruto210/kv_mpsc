# kv_mpsc

A `kv_mpsc` is a bounded channel like `mpsc` in rust std, but it support message with key.

All messages in `kv_mpsc` have single/multiple key(s), once a message is consumed by a receiver, it's key(s) will be active, and other messages that have key(s) conflict with active keys could not be consumed by receivers; when the message is droped, it's key(s) will be removed from the active keyset.



## Design

### Shared Queue

The core data structure of `kv_mpsc` is `Shared`. 
- `state` is the state of a share queue, use mutext to protect it.
- When the queue is empty, receiver will wait on condvar `fill`.
- when the queue is full, sender will wait on condvar `empty`.

```rust
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
```

The queue `State` is as follows:
- Buff is a fixed size vec to temporarily store messages.
- n_senders is the number of senders.
- disconnected is a flag to indicate whether the channel is disconnected(all sender gone or receiver closed).


```rust
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
```

The `Buff` is as follows:
- `buff` is the actual buffer.
- `cap` is the capacity of the buffer.
- `activate_keys` is the set of current active keys. 

```rust
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
```

Use a feature `list` to control the based buff type. I think LinkedList maybe better than `VecDeque` if there are frequent conflicts, because `VecDeque` need to move all elements after the removed one;

```rust
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
```

### Send/Recv

The process of send is as follows:
- acquire a empty buffer slot
- check whether the channel is disconnected
- push message
- notify receiver

```rust
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
```

The process of receive is as follows:
- wait until buffer is not empty
- check whether the channel is disconnected
- consume a message that is not conflict with active keys
- notify a sender

the `pop_unconflict_front` method scan messages from front to back, and pop the first message that is not conflict with current active keys.
```rust
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
```
A message also contains a Ref to the shared queye state `Shared`, when it drops, it will remove it's key(s) from the current active keyset, then other messages conflict with it could be consumed.

```rust
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
```

The `SyncSender` is just a wrapper of `Shared`, and it's `send` method is just a wrapper of `Shared::send`.

The `Receiver` contains a marker to make it `!Sync` so that only one thread could receive message from it.
and it's `send` method is responsible for setting `Shared` for msg returned by `Shared::recv`.

```rust
/// A sync sender that will block when there no empty buff slot
#[derive(Debug)]
pub struct SyncSender<K: Key, V> {
    /// inner shared queue
    inner: Arc<Shared<K, V>>,
}

/// A sync receiver will block when buff is empty
#[derive(Debug)]
pub struct Receiver<K: Key, V> {
    /// shared FIFO queue
    inner: Arc<Shared<K, V>>,
    /// remove the auto `Sync` implentation, so only one
    /// thread can access the receiver
    _marker: std::marker::PhantomData<RefCell<()>>,
}
```

## Bench
[`send_recv`](benches/send_recv.rs) is a simple bench program containes 3 bench functions send on 10 threads and recv on 1 thread, the three functions are std mpsc, kv_mpsc without key conflict and kv_mpsc with key conflict respectively.

bench env:
 - cpu: 5700G@3.800GHz with 8 cores and 16 threads
 - OS: Arch Linux x86_64


From the result, we can see that std mpsc is much faster and more stable than kv_mpsc. The overhead of my impl maybe too large.

The kv_mpsc channel based on `LinkedList` is about 13% slower than the one based on `VecDeque` when no conflicts happens, and even worse when conflicts happens. Maybe list operations are even more expensive than move value in vec. The propgram is naive, if I have more time, maybe I could find the exact reason.


The `VecDeque` result:

```bash

MultiThread Send and Recv/std_mpsc
                        time:   [38.472 ms 39.203 ms 39.912 ms]
Benchmarking MultiThread Send and Recv/kv_mpsc no conflict: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 6.1s, or reduce sample count to 80.
MultiThread Send and Recv/kv_mpsc no conflict
                        time:   [59.860 ms 60.282 ms 60.659 ms]
Found 18 outliers among 100 measurements (18.00%)
  14 (14.00%) low severe
  3 (3.00%) low mild
  1 (1.00%) high mild
Benchmarking MultiThread Send and Recv/kv_mpsc with conflict: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 32.6s, or reduce sample count to 10.
MultiThread Send and Recv/kv_mpsc with conflict
                        time:   [314.42 ms 319.45 ms 324.52 ms]
Found 2 outliers among 100 measurements (2.00%)
  1 (1.00%) low mild
  1 (1.00%) high mild


```

The `LinkedList` result:
```bashMultiThread Send and Recv/std_mpsc
                        time:   [39.038 ms 39.865 ms 40.682 ms]
Benchmarking MultiThread Send and Recv/kv_mpsc no conflict: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 6.8s, or reduce sample count to 70.
MultiThread Send and Recv/kv_mpsc no conflict
                        time:   [67.133 ms 67.439 ms 67.712 ms]
Found 11 outliers among 100 measurements (11.00%)
  4 (4.00%) low severe
  6 (6.00%) low mild
  1 (1.00%) high mild
Benchmarking MultiThread Send and Recv/kv_mpsc with conflict: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 50.7s, or reduce sample count to 10.
MultiThread Send and Recv/kv_mpsc with conflict
                        time:   [448.57 ms 455.93 ms 463.10 ms]
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) low mild

```
