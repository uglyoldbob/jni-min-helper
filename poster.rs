//! This is an optimization effort for `post_to_main_looper`.
//!
//! # Safety of Java-owned Rust heap objects
//!
//! The risks described in <https://github.com/jni-rs/jni-rs/issues/833> is reduced
//! by the trick being used here, making sure the `Box` pointer usage is safe. After
//! applying the `pin_current_cdylib` fix, this becomes an extra double check:
//!
//! In case of the library is unloaded and then loaded again, a new `LifeChecker`
//! is created here; in case of a previous `RunOnce` can enter its native handler,
//! `check_lib_life_against` will detect that its `LifeChecker` and the current
//! `NATIVE_LIB_LIFE_CHECKER` are not the same object, it will prevent the invalid
//! pointer value from this `RunOnce` from being casted.
//!
//! # Specific requirements about `RunOnce`
//!
//! For the current Rust-side code, the Java-side `RunOnce` proxy must:
//! - Not implement `Clonable`;
//! - Mark the raw pointer as invalid (by an atomic flag) once it is used.
//!
//! Otherwise the Rust code would be *unsound*.

use std::ops::Deref;
use std::ptr::with_exposed_provenance_mut;
use std::sync::{LazyLock, OnceLock};

use jni::{
    Env,
    errors::Error,
    jni_sig, jni_str,
    objects::LoaderContext,
    refs::{Global, Reference},
    sys::jlong,
};

use crate::{
    DynamicProxy,
    android::get_helper_class_loader,
    bindings::{AndroidHandler, AndroidLooper, JRunnable},
    jni_with_env,
};

type RunnableClosure = dyn FnOnce(&mut jni::Env) -> Result<(), Error> + Send + Sync + 'static;
type RunnableClosureBox = Box<Box<RunnableClosure>>;

use jni::objects::JObject as LifeChecker;
static NATIVE_LIB_LIFE_CHECKER: LazyLock<Global<LifeChecker<'static>>> = LazyLock::new(|| {
    jni_with_env(|env| {
        let cls = LifeChecker::lookup_class(env, &LoaderContext::None)?;
        let cls = cls.deref();
        let obj = env.new_object(cls, jni_sig!(()), &[])?;
        let obj_another = env.new_object(cls, jni_sig!(()), &[])?;
        assert!(!env.is_same_object(&obj, &obj_another)?);
        env.new_global_ref(obj)
    })
    .unwrap()
});

fn check_lib_life_against(env: &mut Env, current: LifeChecker) -> Result<bool, Error> {
    if env.is_same_object(NATIVE_LIB_LIFE_CHECKER.as_obj(), current)? {
        Ok(true)
    } else {
        warn!("WARNING: Detected native library reload, previous `Runnable` proxies are invaid.");
        Ok(false)
    }
}

jni::bind_java_type! {
    pub(crate) RunOnce => "rust.jniminhelper.RunOnce",
    type_map = {
        JRunnable => "java.lang.Runnable",
    },
    constructors {
        fn new(id: jlong, checker: JObject),
    },
    methods {
        fn mark_invalid(),
    },
    native_methods_error_policy = jni::errors::LogErrorAndDefault,
    native_methods_export = false,
    native_methods {
        fn rust_hdl {
            sig = (id: jlong, checker: JObject),
            fn = runonce_handler,
        },
    },
    is_instance_of = {
        JRunnable,
    },
    hooks = {
        load_class = |env, _load_context, initialize| {
            let loader_context = LoaderContext::Loader(get_helper_class_loader(env)?);
            loader_context.load_class(env, jni_str!("rust.jniminhelper.RunOnce"), initialize)
        },
    }
}

impl DynamicProxy {
    /// Posts a `Runnable` for the Android main looper thread to do UI-related operations.
    /// Returns false on failure (usually because the looper is exiting).
    ///
    /// Please also consider using `AndroidApp::run_on_java_main_thread` if you are building
    /// an application based on the `android-activity` crate.
    pub fn post_to_main_looper<F>(closure: F) -> Result<bool, Error>
    where
        F: FnOnce(&mut jni::Env) -> Result<(), Error> + Send + Sync + 'static,
    {
        static MAIN_HANDLER: OnceLock<Global<AndroidHandler<'static>>> = OnceLock::new();

        jni_with_env(|env| {
            let closure_box: RunnableClosureBox = Box::new(Box::new(closure));
            let ptr = Box::into_raw(closure_box);
            let runonce = RunOnce::new(
                env,
                ptr.expose_provenance() as i64,
                NATIVE_LIB_LIFE_CHECKER.as_obj(),
            )?;

            if MAIN_HANDLER.get().is_none() {
                let main_looper = AndroidLooper::get_main_looper(env)?;
                let handler = AndroidHandler::new(env, main_looper)?;
                let _ = MAIN_HANDLER.set(env.new_global_ref(handler)?);
            }

            let is_posted = MAIN_HANDLER.get().unwrap().post(env, &runonce)?;
            if !is_posted && runonce.mark_invalid(env).is_ok() {
                // Safety: `android.os.Handler.post()` returns false, meaning
                // it failed to place the `Runnable` into the queue.
                // Also, `mark_invalid` is called so that `runonce` cannot
                // treat the pointer as an owned Rust `Box` anymore.
                let _ = unsafe { RunnableClosureBox::from_raw(ptr) };
            }
            Ok(is_posted)
        })
    }

    /// This does `post_to_main_looper` and waits for the result from `closure` if the current
    /// thread is not the Android main looper thread; otherwise it runs `closure` immediately.
    ///
    /// Warning: Do not block on this with something like `futures_lite::futures::block_on`
    /// in `android_main()` of the glue crate `android_activity`.
    ///
    /// Returns `Ok(None)` if `post_to_main_looper` returns `false`, or the `timeout` is reached
    /// while waiting for the result to be returned from `closure`.
    #[cfg(feature = "futures")]
    pub async fn post_to_main_looper_async<F, R, E>(
        closure: F,
        timeout: Option<std::time::Duration>,
    ) -> Result<Option<R>, E>
    where
        F: FnOnce(&mut jni::Env) -> Result<R, E> + Send + Sync + 'static,
        R: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        if crate::android::is_current_thread_main_looper()? {
            let vm = crate::jni_get_vm();
            vm.attach_current_thread(|env| closure(env).map(|r| Some(r)))
        } else {
            let (tx, rx) = futures_channel::oneshot::channel();
            if !Self::post_to_main_looper(move |env| {
                let result = closure(env);
                let _ = tx.send(result);
                Ok(())
            })? {
                return Ok(None);
            }
            if let Some(dur) = timeout {
                match crate::async_with_timeout(rx, dur).await {
                    Some(Ok(res)) => res.map(|r| Some(r)),
                    _ => Ok(None),
                }
            } else {
                match rx.await {
                    Ok(res) => res.map(|r| Some(r)),
                    Err(_) => Ok(None),
                }
            }
        }
    }
}

fn runonce_handler<'local>(
    env: &mut Env<'local>,
    _this: RunOnce<'local>,
    id: jlong,
    checker: LifeChecker<'local>,
) -> Result<(), jni::errors::Error> {
    if !check_lib_life_against(env, checker)? {
        return Ok(());
    }
    let ptr = with_exposed_provenance_mut(id as usize);
    // Safety: refer to the module-level comments.
    let closure_box = unsafe { RunnableClosureBox::from_raw(ptr) };
    closure_box(env)
}
