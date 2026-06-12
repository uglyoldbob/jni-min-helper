use crate::{
    AndroidContextApi33,
    android::{AndroidContext, get_android_context, get_helper_class_loader},
    android_api_level, jni_with_env,
    proxy::DynamicProxy,
};

use jni::{
    Env,
    errors::Error,
    objects::{JClass, JObject, JString},
    refs::{Global, Reference},
};

jni::bind_java_type! {
    pub Intent => "android.content.Intent",
    type_map = {
        AndroidContext => "android.content.Context",
        AndroidParcelable => "android.os.Parcelable",
    },
    constructors {
        fn new(),
        fn new_with_action(action: JString),
    },
    methods {
        fn get_package() -> JString,
        fn get_type() -> JString,
        fn get_action() -> JString,
        fn has_extra(name: JString) -> jboolean,
        fn get_string_extra(name: JString) -> JString,
        fn get_int_extra(name: JString, default_value: jint) -> jint,
        fn get_short_extra(name: JString, default_value: jshort) -> jshort,
        fn get_long_extra(name: JString, default_value: jlong) -> jlong,
        fn get_float_extra(name: JString, default_value: jfloat) -> jfloat,
        fn get_double_extra(name: JString, default_value: jdouble) -> jdouble,
        fn get_byte_extra(name: JString, default_value: jbyte) -> jbyte,
        fn get_char_extra(name: JString, default_value: jchar) -> jchar,
        fn get_boolean_extra(name: JString, default_value: jboolean) -> jboolean,
        fn get_byte_array_extra(name: JString) -> jbyte[],
        fn set_action(action: JString) -> Intent,
        fn set_class(package_context: AndroidContext, cls: JClass) -> Intent,
        fn put_extra_bool {
            name = "putExtra",
            sig = (name: JString, value: jboolean) -> Intent,
        },
        fn put_extra_byte {
            name = "putExtra",
            sig = (name: JString, value: jbyte) -> Intent,
        },
        fn put_extra_byte_array {
            name = "putExtra",
            sig = (name: JString, value: jbyte[]) -> Intent,
        },
        fn put_extra_char {
            name = "putExtra",
            sig = (name: JString, value: jchar) -> Intent,
        },
        fn put_extra_double {
            name = "putExtra",
            sig = (name: JString, value: jdouble) -> Intent,
        },
        fn put_extra_float {
            name = "putExtra",
            sig = (name: JString, value: jfloat) -> Intent,
        },
        fn put_extra_int {
            name = "putExtra",
            sig = (name: JString, value: jint) -> Intent,
        },
        fn put_extra_long {
            name = "putExtra",
            sig = (name: JString, value: jlong) -> Intent,
        },
        fn put_extra_short {
            name = "putExtra",
            sig = (name: JString, value: jshort) -> Intent,
        },
        fn put_extra_string {
            name = "putExtra",
            sig = (name: JString, value: JString) -> Intent,
        },
        fn put_extra_string_array {
            name = "putExtra",
            sig = (name: JString, value: JString[]) -> Intent,
        },
        priv fn put_extra_parcelable_internal {
            name = "putExtra",
            sig = (name: JString, value: AndroidParcelable) -> Intent,
        }
    },
}

jni::bind_java_type! {
    AndroidParcelable => "android.os.Parcelable",
}

mod parcelable_extra {
    use super::{AndroidParcelable, Intent};
    use crate::android_api_level;
    use jni::{
        Env,
        errors::Error,
        objects::{JClass, JObject, JString},
        refs::{Cast, Reference},
    };
    // TODO: remove these extra bindings when <https://github.com/jni-rs/jni-rs/issues/764> is resolved.
    jni::bind_java_type! {
        pub IntentApi32 => "android.content.Intent",
        type_map = {
            AndroidParcelable => "android.os.Parcelable",
        },
        methods {
            // public @Nullable <T extends Parcelable> T getParcelableExtra(String name)
            fn get_parcelable_extra {
                name = "getParcelableExtra",
                sig = (name: JString) -> AndroidParcelable,
            },
        },
    }
    jni::bind_java_type! {
        pub IntentApi33 => "android.content.Intent",
        methods {
            // public @Nullable <T> T getParcelableExtra(@Nullable String name, @NonNull Class<T> clazz)
            fn get_parcelable_extra {
                name = "getParcelableExtra",
                sig = (name: JString, class: JClass) -> JObject,
            },
        },
    }
    impl<'local> Intent<'local> {
        pub fn put_extra_parcelable<'local_0, 'local_1>(
            &self,
            env: &mut Env,
            name: impl AsRef<JString<'local_0>>,
            value: impl AsRef<JObject<'local_1>>,
        ) -> Result<(), Error> {
            let value: Cast<'_, '_, AndroidParcelable> = env.as_cast(value.as_ref())?;
            self.put_extra_parcelable_internal(env, name, value)?;
            Ok(())
        }

        pub fn get_parcelable_extra<'env_local, 'local_0, 'local_1>(
            &self,
            env: &mut Env<'env_local>,
            name: impl AsRef<JString<'local_0>>,
            class: impl AsRef<JClass<'local_1>>,
        ) -> Result<JObject<'env_local>, Error> {
            if name.as_ref().is_null() {
                return Err(Error::NullPtr(
                    "null `name` provided for `get_parcelable_extra`",
                ));
            }
            if class.as_ref().is_null() {
                return Err(Error::NullPtr(
                    "null `class` provided for `get_parcelable_extra`",
                ));
            }
            if android_api_level() >= 33 {
                // Safety: `Intent` and `IntentApi33` both map to `android.content.Intent`.
                let intent: Cast<'_, '_, IntentApi33> = unsafe { env.as_cast_unchecked(self) };
                intent.get_parcelable_extra(env, name, class)
            } else {
                // Safety: `Intent` and `IntentApi32` both map to `android.content.Intent`.
                let intent: Cast<'_, '_, IntentApi32> = unsafe { env.as_cast_unchecked(self) };
                let extra: JObject = intent.get_parcelable_extra(env, name).map(Into::into)?;
                if !extra.is_null() && !env.is_instance_of(&extra, class.as_ref())? {
                    return Err(Error::WrongObjectType);
                }
                Ok(extra)
            }
        }
    }
}

jni::bind_java_type! {
    pub IntentFilter => "android.content.IntentFilter",
    constructors {
        fn new(),
        fn new_with_action(action: JString),
    },
    methods {
        fn add_action(action: JString),
        fn add_category(category: JString),
        fn add_data_type(type_: JString),
    }
}

jni::bind_java_type! {
    pub(crate) AndroidBroadcastReceiver => "android.content.BroadcastReceiver",
}

jni::bind_java_type! {
    BroadcastRec => "rust.jniminhelper.BroadcastRec",
    type_map = {
        BroadcastRecHdl => "rust.jniminhelper.BroadcastRec$BroadcastRecHdl",
        AndroidBroadcastReceiver => "android.content.BroadcastReceiver",
    },
    constructors {
        fn new(hdl: BroadcastRecHdl),
    },
    is_instance_of = {
        AndroidBroadcastReceiver,
    }
}

jni::bind_java_type! {
    BroadcastRecHdl => "rust.jniminhelper.BroadcastRec$BroadcastRecHdl",
}

/// Handles `android.content.BroadcastReceiver` object backed by `JniProxy`.
///
/// Register/unregister functions are provided for convenience, but not for
/// maintaining any internal state. However, `unregister()` is called on `drop()`.
#[derive(Debug)]
pub struct BroadcastReceiver {
    receiver: Global<AndroidBroadcastReceiver<'static>>,
    proxy: Option<DynamicProxy>, // taken on `forget()`
    forget: bool,
}

impl AsRef<JObject<'static>> for BroadcastReceiver {
    fn as_ref(&self) -> &JObject<'static> {
        self.receiver.as_obj()
    }
}

impl std::ops::Deref for BroadcastReceiver {
    type Target = JObject<'static>;
    fn deref(&self) -> &Self::Target {
        self.receiver.as_obj()
    }
}

impl Drop for BroadcastReceiver {
    fn drop(&mut self) {
        if !self.forget {
            let _ = self.unregister();
        }
    }
}

impl BroadcastReceiver {
    /// Creates a `android.content.BroadcastReceiver` object backed by the Rust closure.
    ///
    /// The two Java object references passed to the closure are `context` and `intent`.
    ///
    /// Note: without a Rust panic, no exception may be thrown from `onReceive()`.
    pub fn build(
        handler: impl for<'a> Fn(&mut Env<'a>, JObject<'a>, Intent<'a>) -> Result<(), Error>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, Error> {
        jni_with_env(|env| {
            let loader = &jni::refs::LoaderContext::Loader(get_helper_class_loader()?);
            let _ = BroadcastRecHdlAPI::get(env, loader)?;
            let _ = BroadcastRecAPI::get(env, loader)?;
            let cls_rec_hdl = BroadcastRecHdl::lookup_class(env, loader)?;
            use std::ops::Deref;
            let proxy = DynamicProxy::build(
                env,
                loader,
                [AsRef::<JClass>::as_ref(&cls_rec_hdl.deref())],
                move |env, method, args| {
                    if &method.get_name(env)?.to_string() == "onReceive" && args.len(env)? == 2 {
                        let context = args.get_element(env, 0)?;
                        let intent = args.get_element(env, 1)?;
                        let intent = Intent::cast_local(env, intent)?;
                        let _ = handler(env, context, intent);
                        env.exception_clear();
                    }
                    Ok(JObject::null())
                },
            )?;

            let receiver_hdl = env.new_local_ref(proxy.as_ref())?;
            let receiver_hdl = env.cast_local::<BroadcastRecHdl>(receiver_hdl)?;
            let receiver = BroadcastRec::new(env, receiver_hdl)?;

            Ok(Self {
                receiver: env.new_global_ref(AndroidBroadcastReceiver::from(receiver))?,
                proxy: Some(proxy),
                forget: false,
            })
        })
    }

    // XXX: add the `exported` parameter in the next SemVer-breaking version.
    /// Registers the receiver to the current Android context.
    /// `RECEIVER_EXPORTED` flag is set for Android 13 and above versions.
    pub fn register(&self, intent_filter: &IntentFilter<'_>) -> Result<(), Error> {
        self.register_internal(intent_filter, true)
    }

    /// Registers the receiver to the current Android context.
    /// `RECEIVER_NOT_EXPORTED` flag is set for Android 13 and above versions.
    pub fn register_no_export(&self, intent_filter: &IntentFilter<'_>) -> Result<(), Error> {
        self.register_internal(intent_filter, false)
    }

    fn register_internal(
        &self,
        intent_filter: &IntentFilter<'_>,
        exported: bool,
    ) -> Result<(), Error> {
        jni_with_env(|env| {
            let context = get_android_context();
            if android_api_level() >= 33 {
                let flag = if exported {
                    AndroidContextApi33::RECEIVER_EXPORTED
                } else {
                    AndroidContextApi33::RECEIVER_NOT_EXPORTED
                };
                context.as_api_33(env).register_receiver(
                    env,
                    &self.receiver,
                    intent_filter,
                    flag,
                )?;
            } else {
                context.register_receiver(env, &self.receiver, intent_filter)?;
            }
            Ok(())
        })
    }

    // XXX: add the `exported` parameter in the next SemVer-breaking version.
    /// Registers the receiver to the current Android context, with an intent filter
    /// that matches a single `action` with no data.
    pub fn register_for_action(&self, action: &str) -> Result<(), Error> {
        jni_with_env(|env| {
            let action = JString::new(env, action)?;
            let filter = IntentFilter::new_with_action(env, action)?;
            self.register(&filter)
        })
    }

    /// The same as [Self::register_for_action], except using the `RECEIVER_NOT_EXPORTED` flag.
    pub fn register_for_action_no_export(&self, action: &str) -> Result<(), Error> {
        jni_with_env(|env| {
            let action = JString::new(env, action)?;
            let filter = IntentFilter::new_with_action(env, action)?;
            self.register_no_export(&filter)
        })
    }

    /// Unregister the previously registered broadcast receiver. All filters that have been
    /// registered for this receiver will be removed.
    #[inline(always)]
    pub fn unregister(&self) -> Result<(), Error> {
        jni_with_env(|env| {
            let context = get_android_context();
            context.unregister_receiver(env, &self.receiver).map(|_| ())
        })
    }

    /// Leaks the Rust handler and returns the global reference of the broadcast
    /// receiver. It prevents deregistering of the receiver on dropping. This is
    /// useful if it is created for *once* in the program.
    pub fn forget(mut self) -> Global<JObject<'static>> {
        self.forget = true;
        self.proxy.take().unwrap().forget();
        jni_with_env(|env| env.new_cast_global_ref::<JObject>(&self.receiver)).unwrap()
    }
}

#[cfg(feature = "futures")]
pub use waiter::*;

#[cfg(feature = "futures")]
mod waiter {
    use super::*;
    use futures_lite::StreamExt;
    use std::{
        collections::VecDeque,
        pin::Pin,
        sync::{Arc, Mutex},
        task,
        time::Duration,
    };

    /// Waits for intents received by the managed `BroadcastReceiver`.
    #[derive(Debug)]
    pub struct BroadcastWaiter {
        receiver: BroadcastReceiver,
        inner: Arc<BroadcastWaiterInner>,
    }

    #[derive(Debug)]
    struct BroadcastWaiterInner {
        waker: atomic_waker::AtomicWaker,
        intents: Mutex<VecDeque<Global<Intent<'static>>>>,
    }

    impl BroadcastWaiter {
        /// Creates the waiter with a new broadcast receiver.
        /// `actions` are passed to `BroadcastReceiver::register_for_action()`.
        pub fn build(
            actions: impl IntoIterator<Item = impl AsRef<str>>,
        ) -> Result<Self, jni::errors::Error> {
            let inner = Arc::new(BroadcastWaiterInner {
                waker: atomic_waker::AtomicWaker::new(),
                intents: Mutex::new(VecDeque::new()),
            });
            let inner_weak = Arc::downgrade(&inner);
            let receiver = BroadcastReceiver::build(move |env, _, intent| {
                if intent.is_null() {
                    return Ok(());
                }
                let intent = Intent::cast_local(env, intent)?;
                let Some(inner) = inner_weak.upgrade() else {
                    return Ok(());
                };
                let intent = env.new_global_ref(intent)?;
                inner.intents.lock().unwrap().push_back(intent);
                inner.waker.wake();
                Ok(())
            })?;
            for action in actions {
                receiver.register_for_action(action.as_ref())?;
            }
            Ok(Self { receiver, inner })
        }

        /// Exposes a reference to the broadcast receiver for manual registration.
        pub fn receiver(&self) -> &BroadcastReceiver {
            &self.receiver
        }

        /// Returns the amount of received intents available for checking.
        pub fn count_received(&self) -> usize {
            self.inner.intents.lock().unwrap().len()
        }

        /// Takes the next received intent if available. This shouldn't conflict
        /// with the asynchonous feature (which requires a mutable reference).
        pub fn take_next(&self) -> Option<Global<Intent<'static>>> {
            self.inner.intents.lock().unwrap().pop_front()
        }

        /// Waits for receiving an intent.
        /// Note: Waiting in the `android_main()` thread will prevent it from receiving.
        pub fn wait_timeout(&mut self, timeout: Duration) -> Option<Global<Intent<'static>>> {
            let fut = BroadcastWaiterFuture { waiter: self };
            block_with_timeout(fut, timeout).unwrap_or(None)
        }
    }

    /// Convenient blocker for asynchronous functions, based on `futures_lite` and `futures_timer`.
    /// Warning: Blocking in the `android_main()` thread will block the future's completion if it
    /// depends on event processing in this thread (check your glue crate like `android_activity`).
    pub fn block_with_timeout<T>(
        fut: impl std::future::Future<Output = T>,
        dur: std::time::Duration,
    ) -> Option<T> {
        use futures_lite::{FutureExt, future::block_on};
        let fut_comp = async { Some(fut.await) };
        let fut_cancel = async {
            futures_timer::Delay::new(dur).await;
            None
        };
        block_on(fut_comp.or(fut_cancel))
    }

    impl futures_core::Stream for BroadcastWaiter {
        type Item = Global<Intent<'static>>;

        fn poll_next(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Option<Self::Item>> {
            // <https://docs.rs/atomic-waker/1.1.2/atomic_waker/struct.AtomicWaker.html#examples>
            if let Some(intent) = self.take_next() {
                return task::Poll::Ready(Some(intent));
            }
            self.inner.waker.register(cx.waker());
            if let Some(intent) = self.take_next() {
                task::Poll::Ready(Some(intent))
            } else {
                task::Poll::Pending
            }
        }

        // Explanation for this trait function: the actual remaining length should fall
        // in the returned estimation "range" (min, max).
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.count_received(), None)
        }
    }

    struct BroadcastWaiterFuture<'a> {
        waiter: &'a mut BroadcastWaiter,
    }

    impl<'a> std::future::Future for BroadcastWaiterFuture<'a> {
        type Output = Option<Global<Intent<'static>>>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
            if let task::Poll::Ready(intent) = self.waiter.poll_next(cx) {
                task::Poll::Ready(intent)
            } else {
                task::Poll::Pending
            }
        }
    }
}
