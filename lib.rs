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

use std::sync::OnceLock;

pub use bindings::*;
pub use proxy::*;

#[cfg(target_os = "android")]
pub use {android::*, permission::*, receiver::*};

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
mod poster;
#[cfg(target_os = "android")]
mod receiver;

use jni::{Env, JavaVM, errors::Error};

/// Calls [jni_get_vm], attaches the current thread to the JVM and executes the closure;
/// The thread may stay attached afterwards, even if it has not been attached previously.
#[inline(always)]
pub fn jni_with_env<R>(f: impl FnOnce(&mut Env) -> Result<R, Error>) -> Result<R, Error> {
    jni_get_vm().attach_current_thread(|env| {
        bindings::verify_bindings(env)?;
        f(env)
    })
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

/// Workaround for <https://github.com/jni-rs/jni-rs/issues/833>.
/// This prevents the current dynamic library (if exists) from being unloaded.
///
/// Returns false if this crate is not compiled into a dynamic library.
/// On Android, this should always return true, otherwise it should panic.
#[inline(never)]
pub fn pin_current_cdylib() -> bool {
    // Tested on Windows 10, Ubuntu 26.04, Android 7,9,13,16.
    static ONCE_PIN_LIB: OnceLock<bool> = OnceLock::new();
    *ONCE_PIN_LIB.get_or_init(|| {
        #[cfg(not(target_os = "android"))]
        if let Some(path) = process_path::get_dylib_path() {
            if process_path::get_executable_path().as_ref() == Some(&path) {
                return false;
            }
            let hdl = dlopen2::raw::Library::open(path.as_os_str()).unwrap();
            std::mem::forget(hdl);
            eprintln!("jni_min_helper::pin_current_cdylib: done for {path:?}.");
            true
        } else {
            false
        }
        // NOTE: at the time of writing this function, `process_path` do not support Android.
        #[cfg(target_os = "android")]
        // Safety: `dladdr` and `dlopen` are MT-safe; `fn_ptr` is inside the address space
        // of current native library; recheck the library base address after `dlopen`.
        unsafe {
            let fn_ptr = pin_current_cdylib as *const ();
            let mut info: libc::Dl_info = std::mem::zeroed();
            // On success, `dladdr` returns a non-zero value.
            if libc::dladdr(fn_ptr as *const libc::c_void, &mut info) != 0 {
                let base_addr = info.dli_fbase;
                let lib_path = std::ffi::CStr::from_ptr(info.dli_fname).to_str().unwrap();
                log::info!("pin_current_cdylib: found library path `{lib_path:?}`");

                // On success, `dlopen` returns a non-null handle.
                let hdl = libc::dlopen(info.dli_fname, libc::RTLD_LAZY | libc::RTLD_LOCAL);
                assert!(!hdl.is_null(), "pin_current_cdylib: `dlopen` call failed");

                let mut info: libc::Dl_info = std::mem::zeroed();
                assert!(libc::dladdr(fn_ptr as *const libc::c_void, &mut info) != 0);
                assert_eq!(info.dli_fbase, base_addr);
            } else {
                panic!("pin_current_cdylib: `dladdr` call failed");
            }
            log::info!("pin_current_cdylib: succeeded.");
            true
        }
    })
}
