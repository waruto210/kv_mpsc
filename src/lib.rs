#![deny(
    // The following are allowed by default lints according to
    // https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html
    absolute_paths_not_starting_with_crate,
    // box_pointers, async trait must use it
    // elided_lifetimes_in_paths,  // allow anonymous lifetime
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_abi,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    // must_not_suspend, unstable
    non_ascii_idents,
    // non_exhaustive_omitted_patterns, unstable
    noop_method_call,
    pointer_structural_match,
    rust_2021_incompatible_closure_captures,
    rust_2021_incompatible_or_patterns,
    rust_2021_prefixes_incompatible_syntax,
    rust_2021_prelude_collisions,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    // unstable_features, // sorry for comment that, just want to test deque/list
    // unused_crate_dependencies, the false positive case blocks us
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences,
    warnings, // treat all wanings as errors
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    // The followings are selected restriction lints for rust 1.57
    clippy::as_conversions,
    clippy::clone_on_ref_ptr,
    clippy::create_dir,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    // clippy::default_numeric_fallback, too verbose when dealing with numbers
    clippy::disallowed_script_idents,
    clippy::else_if_without_else,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::exit,
    clippy::expect_used,
    clippy::filetype_is_file,
    clippy::float_arithmetic,
    clippy::float_cmp_const,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    // clippy::implicit_return, it's idiomatic Rust code.
    clippy::indexing_slicing,
    // clippy::inline_asm_x86_att_syntax, stick to intel syntax
    clippy::inline_asm_x86_intel_syntax,
    clippy::integer_arithmetic,
    // clippy::integer_division, required in the project
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::missing_docs_in_private_items,
    clippy::missing_enforced_import_renames,
    clippy::missing_inline_in_public_items,
    // clippy::mod_module_files, mod.rs file is used
    clippy::modulo_arithmetic,
    clippy::multiple_inherent_impl,
    // clippy::panic, allow in application code
    // clippy::panic_in_result_fn, not necessary as panic is banned
    clippy::pattern_type_mismatch,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    // clippy::shadow_reuse, it’s a common pattern in Rust code
    // clippy::shadow_same, it’s a common pattern in Rust code
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_to_string,
    clippy::todo,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    // clippy::unreachable, allow unreachable panic, which is out of expectation
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    // clippy::use_debug, debug is allow for debug log
    clippy::verbose_file_reads,
    clippy::wildcard_enum_match_arm,
)]
#![allow(
    clippy::panic, // allow debug_assert, panic in production code
    clippy::multiple_crate_versions, // caused by the dependency, can't be fixed
)]
#![feature(linked_list_remove)]
//! `kv_mpsc` is a mpsc channel that support key conflict resolution.
//! //!
//! This module provides message-based communication over channels, concretely
//! defined among two types:
//! * [`SyncSender`]
//! * [`Receiver`]
//!
//! [`SyncSender`] is used to send data to a [`Receiver`]. [`SyncSender`] is
//! clone-able (multi-producer) such that many threads can send simultaneously
//! to one receiver (single-consumer).
//!
//! The channel is a synchronous, bounded channel. The [`bounded`] function will
//! return a `(SyncSender, Receiver)` tuple where the storage for pending
//! messages is a pre-allocated buffer of a fixed size. All sends will be
//! **synchronous** by blocking until there is buffer space available. For simplicity
//! of implementation, capacity of channel must be greater that zero.
//!
//! ## Key of message and conflict
//! All messages in [`bounded`] have single/multiple key(s), once a message is consumed
//! by a receiver, it's key(s) will be active, and other messages that have key(s) conflict with
//! active keys could not be consumed by receivers; when the message is droped, it's key(s) will be removed
//! from the active keyset
//!
//! ## Disconnection
//!
//! The send and receive operations on channels will all return a [`Result`]
//! indicating whether the operation succeeded or not. An unsuccessful operation
//! is normally indicative of the other half of a channel having closed(droped by another thread).
//!
//! If all senders all gone, the receiver will consume the remaining messages in buffer,
//! subsequent recv calls will return an [`err::RecvError`].
//! If the receiver closed, all sender's `send` invocation will return an [`err::SendError`].
//!
//! # Examples
//!
//! Simple usage:
//! ```rust
//! use std::thread;
//! use kv_mpsc::bounded;
//! use kv_mpsc::Message;
//!
//! // create a simple channel
//! let (tx, rx) = bounded(1);
//! thread::spawn(move || {
//!     let msg = Message::single_key(1, 1);
//!     tx.send(msg).unwrap();
//! });
//! let msg = rx.recv().unwrap();
//! assert_eq!(msg.get_single_key().unwrap(), &1);
//! assert_eq!(msg.get_value(), &1);
//!
//! ```
//!
//!
//! Key conflict
//!
//! ```rust
//! use std::thread;
//! use kv_mpsc::bounded;
//! use kv_mpsc::Message;
//! use kv_mpsc::RecvError;
//!
//! // create a simple channel
//! let (tx, rx) = bounded(1);
//! thread::spawn(move || {
//!     let msg = Message::single_key(1, 1);
//!     tx.send(msg).unwrap();
//!     let msg = Message::single_key(1, 2);
//!     tx.send(msg).unwrap();
//! });
//! let msg = rx.recv().unwrap();
//! assert_eq!(msg.get_single_key().unwrap(), &1);
//! assert_eq!(msg.get_value(), &1);
//! assert_eq!(rx.recv(), Err(RecvError::AllConflict));
//! drop(msg);
//! let msg = rx.recv().unwrap();
//! assert_eq!(msg.get_single_key().unwrap(), &1);
//! assert_eq!(msg.get_value(), &2);
//! assert_eq!(rx.recv(), Err(RecvError::Disconnected));
//!
//! ```
pub mod channel;
mod err;
mod message;
mod shared;
mod util;

pub use channel::{bounded, Receiver, SyncSender};
pub use err::*;
pub use message::Message;

#[cfg(test)]
mod test {

    use super::*;
    use std::{
        collections::HashSet,
        iter::FromIterator,
        sync::{atomic::AtomicBool, Arc},
        thread,
    };

    #[test]
    fn test_sender_close() {
        let cap = 10;
        let (tx, rx) = bounded(cap);
        let handle = thread::spawn(move || {
            let msg = Message::single_key(1, 1);
            let _drop = tx.send(msg);
        });
        let _drop = handle.join();
        assert_eq!(rx.recv(), Ok(Message::single_key(1, 1)));
        assert_eq!(rx.recv(), Err(RecvError::Disconnected));
    }

    #[test]
    fn test_receiver_close() {
        let cap = 10;
        let (tx, rx) = bounded(cap);
        drop(rx);
        let msg = Message::single_key(1, 1);
        assert_eq!(tx.send(msg), Err(SendError(Message::single_key(1, 1))));
    }

    #[test]
    fn test_no_conflict_single_key_send_recv() {
        let cap = 10;
        let send = 100;
        let threads = 10;
        let (tx, rx) = bounded(cap);
        let mut handles = vec![];
        for thread_id in 0..threads {
            let tx = tx.clone();
            let handle = thread::spawn(move || {
                for i in 0..send {
                    let msg =
                        Message::single_key(thread_id * send + i, thread_id * send + i);
                    let _drop = tx.send(msg);
                }
            });
            handles.push(handle);
        }
        let mut sum = 0;
        for _ in 0..(send * threads) {
            // all keys no conflict, so it's ok to unwarp
            let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            assert_eq!(
                unwrap_some_or!(msg.get_single_key(), panic!("fatal error")),
                msg.get_value()
            );
            sum += msg.get_value();
        }
        assert_eq!((0..threads * send).sum::<i32>(), sum);
        // make sure all tx is droped
        for handle in handles {
            let _drop = handle.join();
        }
    }

    #[test]
    fn test_no_conflict_multiple_keys_send_recv() {
        let cap = 10;
        let send = 100;
        let threads = 10;
        let (tx, rx) = bounded(cap);
        let mut handles = vec![];
        for thread_id in 0..threads {
            let tx = tx.clone();
            let handle = thread::spawn(move || {
                for i in 0..send {
                    let key1 = thread_id * send + i;
                    let msg = Message::multiple_keys(
                        vec![key1, key1 * 2],
                        thread_id * send + i,
                    );
                    let _drop = tx.send(msg);
                }
            });
            handles.push(handle);
        }
        let mut sum = 0;
        for _ in 0..(send * threads) {
            // all keys no conflict, so it's ok to unwarp
            let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            sum += msg.get_value();
        }
        assert_eq!((0..threads * send).sum::<i32>(), sum);
        // make sure all tx is droped
        for handle in handles {
            let _drop = handle.join();
        }
    }

    #[test]
    fn test_conflict_single_key_send_recv() {
        use std::sync::atomic::Ordering::SeqCst;
        // the test case is as follow
        // tx send 10 k1 msgs
        // rx recv a k1 msg, then recv will return `AllConflict`
        // tx send 1 k2 msg
        // rx recv k2, and then drop the k1 msg above
        // tx send 1 k1 msg
        // rx recv k1, and then drop the k1 msg above
        // rx recv remained k1 msgs and drop them
        // rx drop the k2 msg above and recv the last k2 msg
        let cap = 10;
        let (tx, rx) = bounded(cap);
        let can_send = Arc::new(AtomicBool::new(false));
        let key1 = 1;
        let key2 = 2;
        let send_new_key = Arc::<AtomicBool>::clone(&can_send);
        let handle = thread::spawn(move || {
            for i in 0..cap {
                let msg = Message::single_key(key1, i);
                let _drop = tx.send(msg);
            }
            while !send_new_key.load(SeqCst) {}
            let msg = Message::single_key(key2, cap);
            let _drop = tx.send(msg);
            send_new_key.store(false, SeqCst);

            while !send_new_key.load(SeqCst) {}
            let msg1 = Message::single_key(key2, cap);
            let _drop1 = tx.send(msg1);
            send_new_key.store(false, SeqCst);
        });

        let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg.get_single_key(), panic!("fatal error")), &key1);
        assert_eq!(rx.recv(), Err(RecvError::AllConflict));

        can_send.store(true, SeqCst);
        while can_send.load(SeqCst) {}
        let msg2 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg2.get_single_key(), panic!("fatal error")), &key2);
        drop(msg);

        can_send.store(true, SeqCst);
        while can_send.load(SeqCst) {}

        let msg3 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg3.get_single_key(), panic!("fatal error")), &key1);
        assert_eq!(rx.recv(), Err(RecvError::AllConflict));
        drop(msg3);

        let remained_key1 = cap - 2;
        for _ in 0..remained_key1 {
            let msg4 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            assert_eq!(
                unwrap_some_or!(msg4.get_single_key(), panic!("fatal error")),
                &key1
            );
            assert_eq!(rx.recv(), Err(RecvError::AllConflict));
            drop(msg4);
        }
        drop(msg2);
        let msg5 = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
        assert_eq!(unwrap_some_or!(msg5.get_single_key(), panic!("fatal error")), &key2);
        let _drop = handle.join();
    }

    #[test]
    fn test_conflict_multiple_key_send_recv() {
        let cap = 10;
        let (tx, rx) = bounded(cap);

        let keys = (0..cap).collect::<Vec<usize>>();
        let recv_keys = keys.clone();
        let handle = thread::spawn(move || {
            // All keyset will conflict with the keyset after it
            for i in 0..keys.len() {
                let msg = Message::multiple_keys(
                    unwrap_some_or!(keys.get(0..=i), panic!("fatal error")).to_vec(),
                    i,
                );
                let _drop = tx.send(msg);
            }
        });

        let _drop = handle.join();

        for i in 0..cap {
            let msg = unwrap_ok_or!(rx.recv(), err, panic!("{:?}", err));
            assert_eq!(
                unwrap_some_or!(msg.get_key_set(), panic!("fatal error")),
                &HashSet::from_iter(
                    unwrap_some_or!(recv_keys.get(0..=i), panic!("fatal error")).to_vec()
                )
            );
            assert_eq!(
                rx.recv(),
                if i < cap - 1 {
                    Err(RecvError::AllConflict)
                } else {
                    Err(RecvError::Disconnected)
                }
            );
        }
    }
}
