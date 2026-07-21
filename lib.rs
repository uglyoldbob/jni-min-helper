//! Minimal helper for `jni-rs`, supporting dynamic proxies, Android dex embedding,
//! permission request and broadcast receiver. Used for calling Java code from Rust.
//!
//! Version 0.4.x of this crate can be used with `jni` 0.22.x.
//!
//! This crate uses [ndk_context::AndroidContext] on Android, usually initialized
//! by `android_activity`. Examples for Android are provided in the crate page.
//! Please check out <https://cjycode.com/flutter_rust_bridge/guides/how-to/ndk-init>
//! if you are not using `android_activity` as the native activity entrypoint.
//!
//! Please make sure you are viewing documentation generated for your target.

pub use bindings::*;
pub use proxy::*;

#[cfg(target_os = "android")]
pub use {android::*, permission::*, permission_lifecycle::*, receiver::*};

#[cfg(not(target_os = "android"))]
macro_rules! warn {
    ($($arg:tt)+) => (eprintln!($($arg)+))
}

#[cfg(target_os = "android")]
macro_rules! warn {
    ($($arg:tt)+) => (log::warn!($($arg)+))
}

mod bindings;
mod proxy;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod permission;
#[cfg(target_os = "android")]
mod permission_lifecycle;
#[cfg(target_os = "android")]
mod receiver;

use jni::{Env, JavaVM, errors::Error};

/// Calls [jni_get_vm], attaches the current thread to the JVM and executes the closure;
/// The thread may stay attached even if it has not been attached previously.
#[inline(always)]
pub fn jni_with_env<R>(f: impl FnOnce(&mut Env) -> Result<R, Error>) -> Result<R, Error> {
    jni_get_vm().attach_current_thread(f)
}

/// Try to get the `JavaVM` from  `jni::JavaVM::singleton`, otherwise it launches
/// a new JVM with no arguments (which may panic on failure).
#[cfg(not(target_os = "android"))]
#[inline(always)]
pub fn jni_get_vm() -> JavaVM {
    if let Ok(vm) = jni::JavaVM::singleton() {
        return vm;
    }
    let args = jni::InitArgsBuilder::new().build().unwrap();
    JavaVM::new(args).unwrap()
}

/// This is needed because the `JAVA_VM_SINGLETON` in `jni` crate somehow drops earlier than the
/// `OnceLock` defined in the current crate; the Java VM may not be destroyed between unit tests
/// because they may be executed in the same process.
#[doc(hidden)]
#[cfg(not(target_os = "android"))]
pub fn jni_init_vm_for_unit_test() {
    use std::sync::OnceLock;
    static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();
    let _vm = JAVA_VM.get_or_init(|| {
        let args = jni::InitArgsBuilder::new()
            .option("-Xcheck:jni")
            .build()
            .unwrap();
        jni::JavaVM::new(args).unwrap()
    });
}

/// Try to get the `JavaVM` from  `jni::JavaVM::singleton`, otherwise it gets
/// the `JavaVM` from the `ndk_context` crate.
#[cfg(target_os = "android")]
#[inline(always)]
pub fn jni_get_vm() -> JavaVM {
    if let Ok(vm) = jni::JavaVM::singleton() {
        return vm;
    }
    let ctx = ndk_context::android_context();
    assert!(
        !ctx.vm().is_null(),
        "JVM pointer is uninitialized in `ndk_context` crate"
    );
    // Safety: as documented in `ndk-context` to obtain the `jni::JavaVM`
    unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
}
