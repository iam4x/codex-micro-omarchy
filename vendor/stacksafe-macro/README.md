# stacksafe-macro patch

The source in `src/lib.rs` comes from [stacksafe-macro 0.1.4](https://crates.io/crates/stacksafe-macro/0.1.4),
licensed under Apache-2.0.

This copy replaces `proc-macro-error2` with `syn::Error::into_compile_error`
for the two macro validation errors. It removes the transitive dependency
that triggers Rust's `pub_use_of_private_extern_crate` future incompatibility.
The function transformation is unchanged.

Remove this patch when GPUI accepts a stacksafe release that no longer depends
on `proc-macro-error2`.
