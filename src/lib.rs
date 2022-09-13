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
//! * [`BoundedSender`]
//! * [`Receiver`]
//!
//! [`BoundedSender`] is used to send data to a [`Receiver`]. [`BoundedSender`] is
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
//! ## Async/Sync
//! Implement an async version based on tokio, use `async` to enable it.

#[cfg(feature = "async")]
mod async_channel;
#[cfg(feature = "async")]
pub(crate) use async_channel::Shared;
#[cfg(feature = "async")]
pub use async_channel::{bounded, BoundedSender, Receiver};
mod err;
mod message;
mod state;
#[cfg(not(feature = "async"))]
mod sync_channel;
mod util;

pub use err::*;
pub use message::Message;
#[cfg(not(feature = "async"))]
pub(crate) use sync_channel::Shared;
#[cfg(not(feature = "async"))]
pub use sync_channel::{bounded, Receiver, SyncSender};
