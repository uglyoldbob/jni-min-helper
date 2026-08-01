use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::OnceLock,
};

use jni::{
    Env,
    errors::Error,
    jni_sig, jni_str,
    objects::{JClassLoader, JObject, JString, JThread},
    refs::Global,
};

use crate::{
    bindings::{
        AndroidBuildVersion, AndroidContext, AndroidLooper, DexFileClassLoader,
        InMemoryDexClassLoader,
    },
    jni_get_vm, jni_with_env,
};

const DEX_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/classes.dex"));

pub(crate) fn get_helper_class_loader(
    env: &mut Env,
) -> Result<&'static JClassLoader<'static>, Error> {
    static CLASS_LOADER: OnceLock<Global<JClassLoader<'static>>> = OnceLock::new();
    if CLASS_LOADER.get().is_none() {
        let dex_loader = get_android_context(env)?
            .get_class_loader(env)?
            .load_dex(env, DEX_DATA)?;
        let _ = CLASS_LOADER.set(env.new_global_ref(dex_loader)?);
    }
    Ok(CLASS_LOADER.get().unwrap())
}

/// Provides DEX class loading support for Android.
pub trait DexClassLoader<'local> {
    /// Creates a `dalvik.system.DexClassLoader` from given dex file data embeded at compile time,
    /// having the current loader as the parent loader. This function may do heavy operations.
    fn load_dex(
        &self,
        env: &mut Env<'local>,
        dex_data: &'static [u8],
    ) -> Result<JClassLoader<'local>, Error>;
}

impl<'local> DexClassLoader<'local> for JClassLoader<'local> {
    /// Creates a `dalvik.system.DexClassLoader` from given dex file data embeded at compile time,
    /// having the current loader as the parent loader. This function may do heavy operations.
    fn load_dex(
        &self,
        env: &mut Env<'local>,
        dex_data: &'static [u8],
    ) -> Result<JClassLoader<'local>, Error> {
        let context = get_android_context(env)?;
        if android_api_level() >= 26 {
            // Safety: dex_data is 'static and the `InMemoryDexClassLoader`` will not mutate it.
            // The data may be converted by `ConvertDexFilesToJavaArray()` and handled by the
            // created Java class loader, which shouldn't be freed before the class and its
            // objects are freed. So this local reference doesn't need to be leaked.
            let dex_buffer =
                unsafe { env.new_direct_byte_buffer(dex_data.as_ptr() as *mut _, dex_data.len()) }?;
            let dex_loader = InMemoryDexClassLoader::new(env, &dex_buffer, self)?;
            Ok(dex_loader.into())
        } else {
            // The dex data must be written in a file; this determines the output
            // directory path inside the application code cache directory.
            let code_cache_path = context
                .get_code_cache_dir(env)?
                .get_absolute_path(env)
                .map(|p| std::path::PathBuf::from(p.to_string()))?;

            // Creates the dex file. before creating, calculate the hash for a unique dex name, which
            // may determine names of oat files, which may be mapped to the virtual memory for execution.
            let dex_hash = {
                use std::hash::{DefaultHasher, Hasher};
                let mut hasher = DefaultHasher::new();
                hasher.write(dex_data);
                hasher.finish()
            };
            let dex_name = format!("{dex_hash:016x}.dex");
            let dex_file_path = code_cache_path.join(dex_name);
            std::fs::write(&dex_file_path, dex_data).unwrap(); // Note: this panics on failure
            let dex_file_path = JString::new(env, dex_file_path.to_string_lossy())?;

            // creates the oats directory
            let oats_dir_path = code_cache_path.join("oats");
            let _ = std::fs::create_dir(&oats_dir_path);
            let oats_dir_path = JString::new(env, oats_dir_path.to_string_lossy())?;

            // loads the dex file
            let dex_loader = DexFileClassLoader::new(
                env,
                &dex_file_path,
                &oats_dir_path,
                JString::null(),
                self,
            )?;
            Ok(dex_loader.into())
        }
    }
}

/// Gets the current `android.content.Context` from [ndk_context](https://docs.rs/ndk-context),
/// usually a reference of `android.app.Application`. Please check the completed issue
/// <https://github.com/rust-mobile/android-activity/issues/228> if you had expected a reference
/// of an activity to be returned by this function.
pub fn android_context() -> &'static JObject<'static> {
    jni_with_env(get_android_context).unwrap()
}

pub(crate) fn get_android_context(
    env: &mut Env,
) -> Result<&'static AndroidContext<'static>, Error> {
    static ANDROID_CONTEXT: OnceLock<Global<AndroidContext<'static>>> = OnceLock::new();

    if let Some(ctx) = ANDROID_CONTEXT.get() {
        return Ok(ctx);
    }

    let ndk_context_available = do_and_forget_panic(|| {
        let _ = ndk_context::android_context();
    });
    let ctx_raw_global = if ndk_context_available {
        ndk_context::android_context().context() as jni::sys::jobject
    } else {
        std::ptr::null_mut() as _
    };

    let ctx = if !ctx_raw_global.is_null() {
        // Safety: the context pointer initialized in `ndk_context` should be a raw global
        // reference of `android.content.Context`, which can be casted to `jni::sys::jobject`.
        // `Env::as_cast_raw` does not create an owned `Global` that deletes the reference on drop.
        let ctx = unsafe {
            env.as_cast_raw::<jni::refs::Global<AndroidContext<'static>>>(&ctx_raw_global)?
        };
        match ctx.get_application_context(env) {
            Ok(app) if !app.is_null() => env.new_global_ref(app),
            _ => {
                env.exception_clear();
                warn!(
                    "Cannot get the application context from the context pointed by `ndk_context`."
                );
                env.new_global_ref(ctx.as_ref())
            }
        }
    } else {
        get_app_from_activity_thread(env)
    }?;
    let _ = ANDROID_CONTEXT.set(ctx);

    Ok(ANDROID_CONTEXT.get().unwrap())
}

fn get_app_from_activity_thread(env: &mut Env) -> Result<Global<AndroidContext<'static>>, Error> {
    warn!("Doing the ActivityThread.getApplication() trick.");
    let th = env
        .call_static_method(
            jni_str!("android/app/ActivityThread"),
            jni_str!("currentActivityThread"),
            jni_sig!(() -> android.app.ActivityThread),
            &[],
        )?
        .l()?;
    let app = env
        .call_method(
            &th,
            jni_str!("getApplication"),
            jni::jni_sig!(() -> android.app.Application),
            &[],
        )?
        .l()?;
    let ctx = AndroidContext::cast_local(env, app)?;
    let ctx = env.new_global_ref(ctx)?;
    if ctx.is_null() {
        return Err(Error::NullPtr(
            "got null from ActivityThread.getApplication()",
        ));
    }
    Ok(ctx)
}

/// Attempts to initialize `ndk_context`, returning false if already initialized,
/// forgetting the panic produced in `ndk_context`. This is *unneeded* if the current
/// application framework initializes `ndk_context` automatically.
///
/// This can be used in an exported native method defined in a native library.
/// If needed, call that method in Java as early as possible.
///
/// DO NOT use this function if it is known that some component may try to initialize
/// `ndk_context` at some later point (without forgetting the panic on failure).
///
/// # Safety
///
/// `ndk_context::initialize_android_context` is not thread-safe. Make sure that
/// data racing with another initializer is impossible.
pub unsafe fn try_init_ndk_context<'local>(
    env: &mut jni::Env<'local>,
    context: impl AsRef<JObject<'local>>,
) -> Result<bool, jni::errors::Error> {
    let mut inited_here = false;
    let init_needed = !do_and_forget_panic(|| {
        let _ = ndk_context::android_context();
    });
    if init_needed {
        let vm = env.get_java_vm()?.get_raw() as _;
        let ctx = if !context.as_ref().is_null() {
            let casted = env.as_cast::<AndroidContext>(context.as_ref())?;
            env.new_global_ref(casted.as_ref())?
        } else {
            get_app_from_activity_thread(env)?
        }
        .into_raw() as _;
        // Safety: the JVM pointer will keep valid for the native library's lifetime;
        // it is checked that `context` is a non-null reference of `android.content.Context`;
        // the drop glue of `Global` is untouched because `Global::into_raw` is called.
        if do_and_forget_panic(|| unsafe {
            ndk_context::initialize_android_context(vm, ctx);
        }) {
            let ndk_ctx = ndk_context::android_context();
            if ndk_ctx.vm() != vm || ndk_ctx.context() != ctx {
                panic!("data race detected in `jni_min_helper::try_init_ndk_context`");
            }
            warn!("`ndk_context` is initialized by the `jni-min-helper` crate.");
            inited_here = true;
        }
    }
    Ok(inited_here)
}

fn do_and_forget_panic(f: impl FnOnce() + std::panic::UnwindSafe) -> bool {
    let res = std::panic::catch_unwind(f);
    if res.is_err() {
        std::mem::forget(res);
        false
    } else {
        true
    }
}

/// Gets the API level (SDK version) of the current Android OS.
pub fn android_api_level() -> i32 {
    static API_LEVEL: OnceLock<i32> = OnceLock::new();
    *API_LEVEL.get_or_init(|| {
        // Do not use `jni_with_env` which may do `verify_bindings` which hits here.
        jni_get_vm()
            .attach_current_thread(|env| AndroidBuildVersion::SDK_INT(env))
            .unwrap()
    })
}

/// Gets the raw name of the current Android application, parsed from the package name.
pub fn android_app_name() -> &'static str {
    static APP_NAME: OnceLock<String> = OnceLock::new();
    APP_NAME.get_or_init(|| {
        android_app_package_name()
            .split('.')
            .next_back()
            .unwrap()
            .to_string()
    })
}

/// Gets the package name of the current Android application.
pub fn android_app_package_name() -> &'static str {
    static PACKAGE_NAME: OnceLock<String> = OnceLock::new();
    PACKAGE_NAME.get_or_init(|| {
        jni_with_env(|env| {
            get_android_context(env)?
                .get_package_name(env)
                .map(|s| s.to_string())
        })
        .unwrap()
    })
}

/// Returns the absolute path to the directory holding application files. No permissions
/// are required for the calling app to read or write files under the returned path.
pub fn android_app_files_dir() -> &'static Path {
    static FILES_DIR: OnceLock<PathBuf> = OnceLock::new();
    FILES_DIR.get_or_init(|| {
        jni_with_env(|env| {
            get_android_context(env)?
                .get_files_dir(env)?
                .get_absolute_path(env)
                .map(|s| PathBuf::from_str(&s.to_string()).unwrap())
        })
        .unwrap()
    })
}

/// Returns the absolute path to the application specific cache directory.
pub fn android_app_cache_dir() -> &'static Path {
    static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
    CACHE_DIR.get_or_init(|| {
        jni_with_env(|env| {
            get_android_context(env)?
                .get_cache_dir(env)?
                .get_absolute_path(env)
                .map(|s| PathBuf::from_str(&s.to_string()).unwrap())
        })
        .unwrap()
    })
}

/// TODO: determine the naming of this function and make it public.
pub(crate) fn is_current_thread_main_looper() -> Result<bool, Error> {
    // Java thread ID should not change once the thread is created.
    static MAIN_LOOPER_TID: OnceLock<jni::sys::jlong> = OnceLock::new();
    if MAIN_LOOPER_TID.get().is_none() {
        jni_with_env(|env| {
            let main_looper = AndroidLooper::get_main_looper(env)?;
            let main_thread = main_looper.get_thread(env)?;
            let _ = MAIN_LOOPER_TID.set(main_thread.get_id(env)?);
            Ok(())
        })?;
    }
    let cur_id = jni_with_env(|env| {
        let current_thread = JThread::current_thread(env)?;
        current_thread.get_id(env)
    })?;
    Ok(cur_id == *MAIN_LOOPER_TID.get().unwrap())
}
