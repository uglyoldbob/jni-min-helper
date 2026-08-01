use std::sync::OnceLock;

use jni::{Env, bind_java_type, errors::Error, refs::LoaderContext};

static ONCE_VERIFY_BINDINGS: OnceLock<()> = OnceLock::new();

#[inline(always)]
pub(crate) fn verify_bindings(env: &mut Env) -> Result<(), Error> {
    if ONCE_VERIFY_BINDINGS.get().is_some() {
        return Ok(());
    }
    verify_bindings_impl(env)
}

pub(crate) fn verify_bindings_impl(env: &mut Env) -> Result<(), Error> {
    let ctx = LoaderContext::None;

    env.with_local_frame(32, |env| {
        JavaSystemAPI::get(env, &ctx)?;
        JProxyAPI::get(env, &ctx)?;
        JInvocationHandlerAPI::get(env, &ctx)?;
        JRunnableAPI::get(env, &ctx)?;
        JFileAPI::get(env, &ctx)?;

        JMethodAPI::get(env, &ctx)?;
        JBooleanAPI::get(env, &ctx)?;
        JCharacterAPI::get(env, &ctx)?;
        JNumberAPI::get(env, &ctx)?;
        JByteAPI::get(env, &ctx)?;
        JShortAPI::get(env, &ctx)?;
        JIntegerAPI::get(env, &ctx)?;
        JLongAPI::get(env, &ctx)?;
        JFloatAPI::get(env, &ctx)?;
        JDoubleAPI::get(env, &ctx)?;

        let jinteger = JInteger::new(env, 1)?;
        let _jnum: JNumber = JNumber::cast_local(env, jinteger)?;

        pin_current_cdylib();
        crate::proxy::InvocHdlAPI::get(env, &ctx)?;

        Ok::<_, Error>(())
    })?;

    #[cfg(target_os = "android")]
    env.with_local_frame(32, |env| {
        android_bindings::verify_bindings(env)?;
        crate::poster::RunOnceAPI::get(env, &ctx)?;
        Ok::<_, Error>(())
    })?;

    let _ = ONCE_VERIFY_BINDINGS.set(());
    Ok::<_, jni::errors::Error>(())
}

#[test]
#[cfg(not(target_os = "android"))]
fn test_verify_bindings() {
    use crate::{jni_get_vm, jni_init_vm_for_unit_test};
    jni_init_vm_for_unit_test();
    jni_get_vm()
        .attach_current_thread(|env| verify_bindings(env))
        .unwrap();
}

jni::bind_java_type! {
    pub(crate) JavaSystem => "java.lang.System",
    methods {
        /// Returns the same hash code for the given object as would be returned by the default method hashCode(),
        /// whether or not the given object's class overrides hashCode(). As much as is reasonably practical,
        /// the default `hashCode` method defined by class `Object` does return distinct integers for distinct objects.
        static fn identity_hash_code(obj: JObject) -> jint,
    }
}

bind_java_type! {
    pub(crate) JProxy => "java.lang.reflect.Proxy",
    type_map = {
        JInvocationHandler => "java.lang.reflect.InvocationHandler",
    },
    methods {
        static fn new_proxy_instance(
            class_loader: JClassLoader,
            interfaces: JClass[],
            invoc_hdl: JInvocationHandler
        ) -> JObject,
    },
}

bind_java_type! {
    pub(crate) JInvocationHandler => "java.lang.reflect.InvocationHandler",
}

jni::bind_java_type! {
    pub(crate) JRunnable => "java.lang.Runnable",
}

bind_java_type! {
    pub(crate) JFile => "java.io.File",
    methods {
        fn get_absolute_path() -> JString,
    }
}

bind_java_type! {
    pub JMethod => "java.lang.reflect.Method",
    methods {
        fn equals(arg0: JObject) -> jboolean,
        fn get_name() -> JString,
        fn get_parameter_types() -> JClass[],
        fn get_return_type() -> JClass,
    },
}

bind_java_type! {
    pub JBoolean => "java.lang.Boolean",
    constructors {
        fn new(arg0: jboolean),
    },
    methods {
        fn value {
            name = "booleanValue",
            sig = () -> jboolean,
        },
    },
}

bind_java_type! {
    pub JCharacter => "java.lang.Character",
    constructors {
        fn new(arg0: jchar),
    },
    methods {
        fn value {
            name = "charValue",
            sig = () -> jchar,
        },
    }
}

bind_java_type! {
    pub JNumber => "java.lang.Number",
    constructors {
        fn new(),
    },
    methods {
        fn byte_value() -> jbyte,
        fn double_value() -> jdouble,
        fn float_value() -> jfloat,
        fn int_value() -> jint,
        fn long_value() -> jlong,
        fn short_value() -> jshort,
    },
}

bind_java_type! {
    pub JByte => "java.lang.Byte",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jbyte),
    },
    methods {
        fn value {
            name = "byteValue",
            sig = () -> jbyte,
        },
        static fn parse_byte(arg0: JString) -> jbyte,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JShort => "java.lang.Short",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jshort),
    },
    methods {
        fn value {
            name = "shortValue",
            sig = () -> jshort,
        },
        static fn parse_short(arg0: JString) -> jshort,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JInteger => "java.lang.Integer",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jint),
    },
    methods {
        fn value {
            name = "intValue",
            sig = () -> jint,
        },
        static fn parse_int(arg0: JString) -> jint,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JLong => "java.lang.Long",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jlong),
    },
    methods {
        fn value {
            name = "longValue",
            sig = () -> jlong,
        },
        static fn parse_long(arg0: JString) -> jlong,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JFloat => "java.lang.Float",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jfloat),
    },
    methods {
        fn value {
            name = "floatValue",
            sig = () -> jfloat,
        },
        static fn parse_float(arg0: JString) -> jfloat,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JDouble => "java.lang.Double",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jdouble),
    },
    methods {
        fn value {
            name = "doubleValue",
            sig = () -> jdouble,
        },
        static fn parse_double(arg0: JString) -> jdouble,
    },
    is_instance_of = {
        number: JNumber,
    },
}

#[cfg(target_os = "android")]
pub use android_bindings::*;

use crate::pin_current_cdylib;

#[cfg(target_os = "android")]
mod android_bindings {
    use super::*;
    use crate::android_api_level;
    use jni::bind_java_type;

    /// NOTE: Put all Android SDK API bindings here. This should not fail on Android 7.0
    /// and Android 13.
    pub(crate) fn verify_bindings(env: &mut jni::Env) -> Result<(), jni::errors::Error> {
        let ctx = jni::refs::LoaderContext::None;

        AndroidBuildVersionAPI::get(env, &ctx)?;
        AndroidContextAPI::get(env, &ctx)?;
        if android_api_level() >= 33 {
            AndroidContextApi33API::get(env, &ctx)?;
        }
        AndroidApplicationAPI::get(env, &ctx)?;
        AndroidLifecycleCallbacksAPI::get(env, &ctx)?;
        AndroidActivityAPI::get(env, &ctx)?;
        IntentAPI::get(env, &ctx)?;
        if android_api_level() >= 33 {
            parcelable_extra::IntentApi33API::get(env, &ctx)?;
        } else {
            parcelable_extra::IntentApi32API::get(env, &ctx)?;
        }
        AndroidParcelableAPI::get(env, &ctx)?;
        IntentFilterAPI::get(env, &ctx)?;
        AndroidBroadcastReceiverAPI::get(env, &ctx)?;
        AndroidLooperAPI::get(env, &ctx)?;
        AndroidHandlerAPI::get(env, &ctx)?;
        DexFileClassLoaderAPI::get(env, &ctx)?;
        if android_api_level() >= 26 {
            InMemoryDexClassLoaderAPI::get(env, &ctx)?;
        }
        Ok(())
    }

    bind_java_type! {
        pub(crate) AndroidBuildVersion => "android.os.Build$VERSION",
        fields {
            #[allow(non_snake_case)]
            static SDK_INT {
                sig = jint,
                get = SDK_INT,
            },
        },
    }

    bind_java_type! {
        pub(crate) AndroidContext => "android.content.Context",
        type_map = {
            JFile => "java.io.File",
            AndroidBroadcastReceiver => "android.content.BroadcastReceiver",
            Intent => "android.content.Intent",
            IntentFilter => "android.content.IntentFilter",
        },
        methods {
            fn get_application_context() -> AndroidContext,
            fn get_files_dir() -> JFile,
            fn get_cache_dir() -> JFile,
            fn get_code_cache_dir() -> JFile, // API level >= 21
            fn get_dir(name: JString, mode: jint) -> JFile,
            fn get_class_loader() -> JClassLoader,
            fn get_package_name() -> JString,
            fn register_receiver {
                name = "registerReceiver",
                sig = (receiver: AndroidBroadcastReceiver, filter: IntentFilter) -> Intent,
            },
            fn unregister_receiver(receiver: AndroidBroadcastReceiver),
            fn check_self_permission(permission: JString) -> jint,
            fn start_activity(intent: Intent) -> (),
        }
    }

    // TODO: remove this when <https://github.com/jni-rs/jni-rs/issues/764> is resolved.
    bind_java_type! {
        pub(crate) AndroidContextApi33 => "android.content.Context",
        type_map = {
            AndroidBroadcastReceiver => "android.content.BroadcastReceiver",
            Intent => "android.content.Intent",
            IntentFilter => "android.content.IntentFilter",
        },
        methods {
            // NOTE: this is actually added in API level 26.
            fn register_receiver {
                name = "registerReceiver",
                sig = (receiver: AndroidBroadcastReceiver, filter: IntentFilter, flags: jint) -> Intent,
            },
        }
    }
    impl AndroidContextApi33<'_> {
        pub const RECEIVER_EXPORTED: i32 = 0x00000002;
        pub const RECEIVER_NOT_EXPORTED: i32 = 0x00000004;
    }
    impl<'local> AndroidContext<'local> {
        pub fn as_api_33<'env: 'local>(
            &self,
            env: &jni::Env<'env>,
        ) -> jni::objects::Cast<'_, '_, AndroidContextApi33<'local>> {
            // Safety: `AndroidContext` and `AndroidContextApi33` both map to `android.content.Context`.
            unsafe { env.as_cast_unchecked::<AndroidContextApi33>(self) }
        }
    }

    jni::bind_java_type! {
        pub(crate) AndroidApplication => "android.app.Application",
        type_map = {
            AndroidApplication => "android.app.Application",
            AndroidLifecycleCallbacks => "android.app.Application$ActivityLifecycleCallbacks",
        },
        methods {
            fn register_activity_lifecycle_callbacks(callbacks: AndroidLifecycleCallbacks),
            fn unregister_activity_lifecycle_callbacks(callbacks: AndroidLifecycleCallbacks),
        }
    }

    jni::bind_java_type! {
        pub(crate) AndroidLifecycleCallbacks => "android.app.Application$ActivityLifecycleCallbacks",
    }

    bind_java_type! {
        pub(crate) AndroidActivity => "android.app.Activity",
        methods {
            fn request_permissions(perms: JString[], code: jint),
        }
    }

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
        fields {
            #[allow(non_snake_case)]
            pub(crate) static FLAG_ACTIVITY_NEW_TASK {
                sig = jint,
                get = FLAG_ACTIVITY_NEW_TASK,
            },
        },
        methods {
            fn get_package() -> JString,
            fn get_type() -> JString,
            fn get_action() -> JString,
            fn get_flags() -> jint,
            fn has_extra(name: JString) -> jboolean,
            fn has_category(category: JString) -> jboolean,
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
            fn set_package(package_name: JString) -> Intent,
            fn set_type(type_name: JString) -> Intent,
            fn add_category(category: JString) -> Intent,
            fn set_action(action: JString) -> Intent,
            pub(crate) fn set_class(package_context: AndroidContext, cls: JClass) -> Intent,
            fn set_class_name(package_name: JString, class_name: JString) -> Intent,
            fn set_flags(flags: jint) -> Intent,
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
        pub(crate) AndroidParcelable => "android.os.Parcelable",
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
        impl Intent<'_> {
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
        pub(crate) AndroidLooper => "android.os.Looper",
        methods {
            static non_null fn get_main_looper() -> AndroidLooper,
            non_null fn get_thread() -> JThread,
        }
    }

    jni::bind_java_type! {
        pub(crate) AndroidHandler => "android.os.Handler",
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

    bind_java_type! {
        pub(crate) InMemoryDexClassLoader => "dalvik.system.InMemoryDexClassLoader",
        constructors {
            fn new(dex_buffer: JByteBuffer, parent: JClassLoader),
        },
        is_instance_of = {
            JClassLoader,
        }
    }

    bind_java_type! {
        pub(crate) DexFileClassLoader => "dalvik.system.DexClassLoader",
        constructors {
            fn new(dex_path: JString, optimized_directory: JString, library_search_path: JString, parent: JClassLoader),
        },
        is_instance_of = {
            JClassLoader,
        }
    }
}
