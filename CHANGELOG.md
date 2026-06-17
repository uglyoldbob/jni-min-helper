# Changes

## 0.4.3
* Added public methods `has_category`, `set_package`, `set_type`, `add_category` and `set_class_name` for `Intent` binding.
* `Intent::set_class` is made private because it involves the `AndroidContext` binding type which is currently private. (Not a breaking change, because it could not be used outside)
* Added description for `ndk_context`.

## 0.4.2
* Fixed the problem of `BroadcastReceiver` under Android 14 and above versions: "One of RECEIVER_EXPORTED or RECEIVER_NOT_EXPORTED should be specified when a receiver isn't being registered exclusively for system broadcasts".

## 0.4.1
* Added `Intent::get_parcelable_extra` and `Intent::put_extra_parcelable`.
* Removed custom `JCharSequence` binding in examples, because `jni` 0.22.4 added `JCharSequence` bindings.
* Fixed the chooser dialog example for `android-activity` 0.6.1; noted about <https://github.com/rust-mobile/android-activity/issues/228> for the `android_context` function; removed the warning printed when `android_context` finds a null context pointer in `android_context` and tries to get the application context pointer from the activity thread, because the new 0.6.1 version of `android-activity` initializes the context pointer from the application context instead of the native activity.
* Fixed a wrong JNI method signature in the internal function `get_activity_thread`.
* Noted about `AndroidApp::run_on_java_main_thread` for `DynamicProxy::post_to_main_looper`.
* Added `as_number` method in bindings of Java numeric wrappers.

## 0.4.0
* Adapted to `jni` 0.22, introduced the usage of binding macros.
* Removed convenient mappers for creating `AutoLocal` and clearing exception (the `no-proxy` cargo feature is removed as well), because `jni` 0.22 pushes and pops local reference frames automatically; exception handling of `jni_with_env` becomes consistent with <https://docs.rs/jni/0.22.3/jni/vm/struct.JavaVM.html#exception-handling>.
* Introduced generated bindings for Java primitive type wrapper classes. Removed trait methods for other type convertion operations because `jni` has introduced many of them.
* Removed `JniClassLoader` because `jni` has introduced the `JClassLoader`; for Android, use trait `DexClassLoader` for `JClassLoader` instead.
* Removed the `replace_app_loader` method for Android.

## 0.3.2
* Added `PermissionRequest` for Android.
* Fixed `JniClassLoader::load_class` to use `ClassLoader.loadClass` instead of `ClassLoader.findClass`.
* Added `JniClassLoader::replace_app_loader`.

## 0.3.1
* `jni_with_env` now clears the exception if the closure's result is `Err(Error::JavaException)`; this means any `JNIEnv` method call followed by an `?` without `.map_err(jni_clear_ex)` becomes "acceptable" (?) within `jni_with_env` closures. However, temporary structs created in the closure with `Drop` implementation doing JNI operations not listed in <https://docs.oracle.com/javase/8/docs/technotes/guides/jni/spec/design.html#exception_handling> will still *crash* the program.
* Added `android-build` build dependency because `Dexer` is added in that crate.

## 0.3.0
* Workaround for <https://github.com/jni-rs/jni-rs/issues/558>: removed function `jni_attach_vm`, use `jni_with_env` instead; `jni_get_vm` and `jni_set_vm` are marked with `unsafe`; added function `jni_attach_permanently()`.
* Fixed a bug in `JObjectGet::get_string`: it did not support Unicode characters outside of the BMP (like emojis).
* Functions in `JObjectGet` may return `Error::JniCall(JniError::InvalidArguments)` instead of `Error::WrongJValueType(_, _)`; the error clue string cannot be passed to `class_check` for now.
* `JniProxy::build` now requires `&mut JNIEnv<'e>` as an argument, in order to use `interfaces` inside `jni_with_env` closure.
* Added `JniProxy::current_proxy_id()` for debugging purposes; added `JNIProxy::post_to_main_looper` for Android.
* Enhanced build script: it is now possible to use it as a template for other Android support crates to add jar dependencies for building the dex file.
* Added method `append_dex` in Android `JniClassLoader`.
* Some other fixes.

## 0.2.6
* Added `no-proxy` cargo feature, it disables `JniProxy` and anything that depends on it (e.g. `BroadcastReceiver`), as well as the dex/class building process.

## 0.2.5
* Prints Rust stack trace for PC platforms and for JNI errors other than Java exceptions.
* Gets the Android context object from `ActivityThread.currentActivityThread().getApplication()` and prints a warning message in case of `ndk_context::android_context().context()` is null: functionalities related to the UI or `Activity` will not work, and the glue crate should be checked.

## 0.2.3
* Fixed the bug of panicking during building instead of using the prebuilt dex fallback when some environment variables required by `android-build` are missing.
* Removed `android-build` build dependency, because of its complicated configuration.

## 0.2.2
* Fixed the problem of possible fatal exception when calling `BroadcastReceiver::unregister()` for an unregistered receiver.
* Eliminated the `javac` warning for `InvocHdl.java`.

## 0.2.1
* Fixed a problem about the performance cache in the `convert` module.
* Added `is_same_object()`, `equals()` and `to_string()` in trait `JObjectGet`.
* Added `get_intent_action()` in `BroadcastReceiver` and `BroadcastWaiter`.
* Added `count_received()`, `take_next()` and `futures_core::Stream::size_hint()` implementation in `BroadcastWaiter`.

## 0.2.0
* Optimized the API.
* Fixed the problem of not being able to create proxies for custom interfaces on desktop platforms.
* Introduced the optional asynchronous `BroadcastWaiter`.

## 0.1.1
* Fixed doc.rs build problem by prebuilt class/dex file fallback.

## 0.1.0
* Initial release.
