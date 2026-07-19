use crate::bindings::{JInvocationHandler, JMethod, JProxy};

use jni::{
    Env,
    descriptors::Desc,
    errors::Error,
    jni_str,
    objects::{JClass, JClassLoader, JObject, JObjectArray},
    refs::{Global, LoaderContext},
    sys::jlong,
};
use std::{
    cell::Cell,
    collections::HashMap,
    mem::forget,
    sync::{Arc, LazyLock, Mutex},
    time::Instant,
};

#[cfg(not(target_os = "android"))]
const CLASS_DATA: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/rust/jniminhelper/InvocHdl.class"
));

jni::bind_java_type! {
    pub(crate) InvocHdl => "rust.jniminhelper.InvocHdl",
    type_map = {
        JMethod => "java.lang.reflect.Method",
        JInvocationHandler => "java.lang.reflect.InvocationHandler",
    },
    constructors {
        fn new(arg0: jlong),
    },
    methods {
        fn get_id() -> jlong,
    },
    native_methods_export = false,
    native_methods {
        fn rust_hdl {
            sig = (id: jlong, method: JMethod, args: JObject[]) -> JObject,
            fn = rust_proxy_handler,
        },
    },
    is_instance_of = {
        JInvocationHandler,
    },
    hooks = {
        load_class = |env, load_context, initialize| {
            #[cfg(not(target_os = "android"))]
            let class_loader = {
                let class_loader = match load_context {
                    LoaderContext::Loader(loader) => env.new_local_ref(loader)?,
                    LoaderContext::FromObject(obj) => env.get_object_class(obj)?.get_class_loader(env)?,
                    LoaderContext::None => JClassLoader::get_system_class_loader(env)?,
                };
                env.define_class(
                    Some(jni::jni_str!("rust/jniminhelper/InvocHdl")),
                    &class_loader,
                    CLASS_DATA,
                )?;
                class_loader
            };
            #[cfg(target_os = "android")]
            let class_loader = {
                let _ = load_context;
                crate::get_helper_class_loader()?
            };
            #[allow(clippy::needless_borrow)]
            let loader_context = LoaderContext::Loader(&class_loader);
            loader_context.load_class(env, jni_str!("rust.jniminhelper.InvocHdl"), initialize)
        },
    },
}

#[cfg(target_os = "android")]
jni::bind_java_type! {
    JRunnable => "java.lang.Runnable",
}

#[cfg(target_os = "android")]
jni::bind_java_type! {
    AndroidLooper => "android.os.Looper",
    methods {
        static fn get_main_looper() -> AndroidLooper,
    }
}

#[cfg(target_os = "android")]
jni::bind_java_type! {
    AndroidHandler => "android.os.Handler",
    type_map = {
        AndroidLooper => "android.os.Looper",
        JRunnable => "java.lang.Runnable",
    },
    constructors {
        fn new(looper: AndroidLooper),
    },
    methods {
        fn post(r: JRunnable) -> jboolean,
    }
}

// Maps Java invocation handler IDs to Rust closures.
// `LazyLock` is required for a const initializer.
// `Arc` is required for having `dyn` closures and using them after dropping the MutexGuard.
static RUST_HANDLERS: LazyLock<Mutex<HashMap<i64, Arc<RustHandler>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// The lifetime sugar cannot apply here, because the closure requires multiple reference
// as parameters. Reference: <https://doc.rust-lang.org/stable/nomicon/hrtb.html>.
// Requiring all references here to have the same lifetime bounds doesn't introduce
// any inconvenience outside, because these closures are called only in `rust_callback()`.
// It's tested that returning a new local reference to the Java caller doesn't leak.
type RustHandler = dyn for<'a> Fn(&mut Env<'a>, JMethod<'a>, JObjectArray<'a, JObject<'a>>) -> Result<JObject<'a>, Error>
    + Send
    + Sync
    + 'static;

// This indicates the invoked proxy ID for the Rust handler; it should be `None` elsewhere.
thread_local! {
    // There's a false-positive Clippy bug: <https://github.com/rust-lang/rust-clippy/issues/13422>
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local))]
    static CURRENT_PROXY_ID: Cell<Option<i64>> = const { Cell::new(None) };
}

/// Java dynamic proxy with an invocation handler backed by the Rust closure.
///
/// It removes the Rust handler on dropping. Dropping the handler will cause problems
/// if methods with non-void returning type are still called from the Java side.
///
/// References:
/// - <https://developer.classpath.org/doc/java/lang/reflect/InvocationHandler.html>
/// - <https://docs.oracle.com/javase/8/docs/api/java/lang/reflect/InvocationHandler.html>
/// - <https://docs.oracle.com/javase/8/docs/api/java/lang/reflect/Proxy.html>
///
/// ```
/// use jni::{jni_sig, jni_str, objects::*};
/// use jni_min_helper::*;
/// jni_init_vm_for_unit_test();
/// jni_with_env(|env| {
///     let proxy = DynamicProxy::build(
///         env,
///         &LoaderContext::None,
///         &[jni_str!("java.util.concurrent.Callable")],
///         |env, method, args| {
///             assert_eq!(args.len(env)?, 0);
///             let out = format!(
///                 "Method `{}` is called with proxy {}.",
///                 method.get_name(env)?,
///                 DynamicProxy::current_proxy_id().unwrap()
///             );
///             let out = JString::new(env, out)?.into();
///             Ok(out)
///         },
///     )?;
///     let result = env
///         .call_method(&proxy, jni_str!("call"), jni_sig!(() -> JObject), &[])?
///         .l()
///         .and_then(|l| JString::cast_local(env, l))?;
///     assert_eq!(
///         result.to_string(),
///         format!("Method `call` is called with proxy {}.", proxy.id())
///     );
///
///     // Now throw an exception inside the handler
///     assert!(!env.exception_check());
///     let proxy = DynamicProxy::build(
///         env,
///         &LoaderContext::None,
///         &[jni_str!("java.lang.Runnable")],
///         |env, _, _| {
///             let s = JString::new(env, "a")?;
///             let _ = JInteger::parse_int(env, s)?;
///             Ok(JObject::null())
///         },
///     )?;
///     let result = env.call_method(&proxy, jni_str!("run"), jni_sig!(() -> ()), &[]);
///     assert!(matches!(result, Err(jni::errors::Error::JavaException)));
///     let last_ex = env.exception_catch().unwrap_err(); // takes the exception
///     assert!(last_ex.to_string().contains("NumberFormatException"));
///     assert!(!env.exception_check());
///     Ok(())
/// })
/// .unwrap();
/// ```
#[derive(Debug)]
pub struct DynamicProxy {
    rust_hdl_id: i64,
    java_proxy: Option<Global<JObject<'static>>>, // always `Some` before `drop` or `forget`
}

impl AsRef<JObject<'static>> for DynamicProxy {
    fn as_ref(&self) -> &JObject<'static> {
        self.java_proxy.as_ref().unwrap().as_obj()
    }
}

impl std::ops::Deref for DynamicProxy {
    type Target = JObject<'static>;
    fn deref(&self) -> &Self::Target {
        self.java_proxy.as_ref().unwrap().as_obj()
    }
}

impl DynamicProxy {
    /// Gets the proxy handler ID for debugging.
    pub fn id(&self) -> i64 {
        self.rust_hdl_id
    }

    /// Leaks the Rust handler and returns the global reference of the Java proxy.
    /// This is useful if the proxy is created for *once* in the program.
    pub fn forget(mut self) -> Global<JObject<'static>> {
        let obj = self.java_proxy.take().unwrap();
        forget(self);
        obj
    }
}

impl Drop for DynamicProxy {
    fn drop(&mut self) {
        if let Ok(mut hdls_locked) = RUST_HANDLERS.lock() {
            let _ = hdls_locked.remove(&self.rust_hdl_id);
        }
    }
}

impl DynamicProxy {
    /// Creates a Java dynamic proxy with a new invocation handler backed by the Rust closure.
    ///
    /// A class loader is needed if the interface definition is loaded from embeded class/dex data;
    /// in such case, `interfaces` should not be strings. Otherwise they can be strings of Java
    /// binary names to look up them at runtime.
    ///
    /// The Rust `handler` should implement methods required by these interfaces. Primitive types
    /// have to be wrapped.
    ///
    /// Returning an error in the Rust handler function causes a Java exception to be thrown.
    ///
    /// `equals()`, `hashCode()` and `toString()` are already implemented in the Java handler.
    pub fn build<'e, T, E, I, F>(
        env: &mut jni::Env<'e>,
        loader_context: &LoaderContext,
        interfaces: I,
        handler: F,
    ) -> Result<Self, Error>
    where
        T: Desc<'e, JClass<'e>>,
        E: ExactSizeIterator<Item = T>,
        I: IntoIterator<Item = T, IntoIter = E>,
        F: for<'f> Fn(
                &mut Env<'f>,
                JMethod<'f>,
                JObjectArray<'f, JObject<'f>>,
            ) -> Result<JObject<'f>, Error>
            + Send
            + Sync
            + 'static,
    {
        let class_loader = match loader_context {
            LoaderContext::Loader(loader) => env.new_local_ref(loader)?,
            LoaderContext::FromObject(obj) => env.get_object_class(obj)?.get_class_loader(env)?,
            LoaderContext::None => JClassLoader::get_system_class_loader(env)?,
        };

        // creates a Java class array for interfaces that should be supported
        let interfaces = interfaces.into_iter();
        let arr_interfaces =
            env.new_object_type_array::<JClass>(interfaces.len(), JClass::null())?;
        for (i, intr) in interfaces.enumerate() {
            let intr = intr.lookup(env)?;
            arr_interfaces.set_element(env, i, intr.as_ref())?;
        }

        // creates the proxy object with a new invocation handler, register the Rust handler with its ID
        let mut handlers_locked = RUST_HANDLERS.lock().unwrap();
        let id: i64 = new_hdl_id(&handlers_locked);
        let invoc_hdl = InvocHdl::new(env, id)?;
        let proxy = JProxy::new_proxy_instance(env, &class_loader, &arr_interfaces, &invoc_hdl)
            .inspect_err(|_| {
                env.exception_describe();
            })?;
        let proxy = env.new_global_ref(proxy)?;
        handlers_locked.insert(id, Arc::new(handler));
        Ok(Self {
            rust_hdl_id: id,
            java_proxy: Some(proxy),
        })
    }

    /// Gets the invoked proxy ID inside the Rust handler closure for debugging;
    /// returns `None` elsewhere.
    pub fn current_proxy_id() -> Option<i64> {
        CURRENT_PROXY_ID.get()
    }
}

#[cfg(target_os = "android")]
impl DynamicProxy {
    /// Posts a `Runnable` for the Android main looper thread to do UI-related operations.
    /// Returns false on failure (usually because the looper is exiting).
    ///
    /// Please also consider using `AndroidApp::run_on_java_main_thread` if you are building
    /// an application based on the `android-activity` crate.
    pub fn post_to_main_looper(
        runnable: impl Fn(&mut jni::Env) -> Result<(), Error> + Send + Sync + 'static,
    ) -> Result<bool, Error> {
        crate::jni_with_env(|env| {
            let runnable = DynamicProxy::build(
                env,
                &LoaderContext::None,
                [jni_str!("java/lang/Runnable")],
                move |env, method, _| {
                    if &method.get_name(env)?.to_string() == "run" {
                        let _ = runnable(env);
                        env.exception_clear();
                    }
                    if let (Some(cur_id), Ok(mut hdls_locked)) =
                        (DynamicProxy::current_proxy_id(), RUST_HANDLERS.lock())
                    {
                        let _ = hdls_locked.remove(&cur_id);
                    }
                    Ok(JObject::null())
                },
            )?;
            let main_looper = AndroidLooper::get_main_looper(env)?;
            if main_looper.is_null() {
                return Err(Error::NullPtr(
                    "android.os.Looper.getMainLooper() returned null",
                ));
            }
            let handler = AndroidHandler::new(env, main_looper)?;
            let new_runnable_ref = env.new_local_ref(runnable.as_ref())?;
            let casted_runnable = JRunnable::cast_local(env, new_runnable_ref)?;
            let is_posted = handler.post(env, casted_runnable)?;
            if is_posted {
                // the runnable will remove the handler by itself, when it is called for once
                let _ = runnable.forget();
            }
            Ok(is_posted)
        })
    }
}

// Note: this function depends on `clock_gettime()` on UNIX, including Android.
fn new_hdl_id(handlers_locked: &HashMap<i64, Arc<RustHandler>>) -> i64 {
    static STARTUP_INSTANT: LazyLock<Instant> = LazyLock::new(Instant::now);
    loop {
        let nanos = STARTUP_INSTANT.elapsed().as_nanos();
        let num = (nanos % (i64::MAX as u128)) as i64;
        if !handlers_locked.contains_key(&num) {
            return num;
        }
    }
}

fn rust_proxy_handler<'local>(
    env: &mut Env<'local>,
    _this: InvocHdl<'local>,
    id: jlong,
    method: JMethod<'local>,
    mut args: JObjectArray<'local, JObject<'local>>,
) -> Result<JObject<'local>, jni::errors::Error> {
    if method.is_null() {
        warn!("Proxy {id}: null `method` received in `rust_proxy_handler`."); // unreachable?
        return Ok(JObject::null());
    }
    if args.is_null() {
        args = JObjectArray::<JObject>::new(env, 0, JObject::null())?;
    }
    let lock = RUST_HANDLERS.lock().unwrap();
    let rust_hdl = if let Some(f) = (*lock).get(&id) {
        f.clone()
    } else {
        warn!("Proxy {id} is used, but the Rust handler has been dropped.");
        return Ok(JObject::null());
    };
    // ReentrantMutex is not needed(?) even if `rust_hdl()` registers another handler.
    drop(lock);
    CURRENT_PROXY_ID.replace(Some(id));
    let result = rust_hdl(env, method, args);
    let _ = CURRENT_PROXY_ID.take();
    result
}
