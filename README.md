# jni-min-helper

Minimal helper for `jni-rs`, supporting dynamic proxies, Android dex embedding, runtime permission request and broadcast receiver. Used for calling Java code from Rust.

Documentation: <https://docs.rs/jni-min-helper/latest>.

The dynamic proxy implementation is inspired by [droid-wrap-utils](https://crates.io/crates/droid-wrap-utils).

To see how a dex file can be embedded, just check the source of this crate. Note: `InvocHdl.class` and `classes.dex` are *unmanaged* prebuilt files for docs.rs to build documentation successfully. `build.rs` will print a warning and use the prebuilt file as a fallback on failure. 

## Desktop

To test it on a desktop OS, just make sure the JDK is installed, then add `jni-min-helper` dependency into your new binary crate, fill in `main()` with the example given in `jni_min_helper::DynamicProxy` documentation.

Of course, the dex class loader and the broadcast receiver are not available. Call `jni::vm::JavaVM::new` (before using other functions) to prevent the library from creating a new JVM by itself.

## Android

Make sure the Android SDK, NDK, Rust target `aarch64-linux-android` and [cargo-apk](https://docs.rs/crate/cargo-apk/latest) are installed.

Build an example with `cargo-apk` and install it on the Android device, then check the log output: `adb logcat android_simple_test:D '*:S'`.

Note: building for the release profile produces a much smaller package.

<details>
<summary>Registering a broadcast receiver</summary>

`Cargo.toml`:

```toml
[package]
name = "android-simple-test"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
log = "0.4"
jni = "0.22.4"
jni-min-helper = "0.4.3"
android-activity = { version = "0.6.1", features = ["native-activity"] }
android_logger = "0.15"

[lib]
name = "android_simple_test"
crate-type = ["cdylib"]
path = "lib.rs"

[package.metadata.android]
package = "com.example.android_simple_test"
build_targets = [ "aarch64-linux-android" ]

[package.metadata.android.sdk]
min_sdk_version = 23
target_sdk_version = 33

[[package.metadata.android.uses_permission]]
name = "android.permission.ACCESS_NETWORK_STATE"
```

`lib.rs`:

```rust
use android_activity::{AndroidApp, MainEvent, PollEvent};
use jni::{
    Env,
    errors::Error,
    objects::{JObject, JString},
};
use jni_min_helper::{BroadcastReceiver, Intent, android_app_name};

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag(android_app_name().as_bytes()),
    );

    let receiver = BroadcastReceiver::build(on_receive).unwrap();
    receiver
        .register_for_action("android.net.conn.CONNECTIVITY_CHANGE")
        .unwrap();

    let mut on_destroy = false;
    loop {
        app.poll_events(None, |event| match event {
            PollEvent::Main(MainEvent::Destroy) => {
                on_destroy = true;
            }
            _ => (),
        });
        if on_destroy {
            return;
        }
    }
}

jni::bind_java_type! {
    AndroidContext => "android.content.Context",
    methods {
        fn get_system_service(name: JString) -> JObject,
    }
}

jni::bind_java_type! {
    ConnectivityManager => "android.net.ConnectivityManager",
    type_map = {
        NetworkInfo => "android.net.NetworkInfo",
    },
    methods {
        fn get_active_network_info() -> NetworkInfo,
    }
}

jni::bind_java_type! {
    NetworkInfo => "android.net.NetworkInfo",
    methods {
        fn is_connected() -> jboolean,
    }
}

jni::bind_java_type! {
    Toast => "android.widget.Toast",
    type_map = {
        AndroidContext => "android.content.Context",
    },
    methods {
        static fn make_text(ctx: AndroidContext, text: JCharSequence, dur: jint) -> Toast,
        fn show(),
    }
}

fn on_receive<'a>(
    env: &mut Env<'a>,
    context: JObject<'a>,
    intent: Intent<'a>,
) -> Result<(), Error> {
    let context = AndroidContext::cast_local(env, context)?;
    let action = intent.get_action(env)?.to_string();
    log::info!("Received an intent of action '{action}'.");

    let conn_service = JString::new(env, "connectivity")?;
    let conn_man = context.get_system_service(env, conn_service)?;
    let conn_man = ConnectivityManager::cast_local(env, conn_man)?;
    let net_info = conn_man.get_active_network_info(env)?;
    let connected = if !net_info.is_null() {
        net_info.is_connected(env)?
    } else {
        false
    };

    let msg = if connected {
        "Network is connected."
    } else {
        "Network is currently disconnected."
    };
    log::info!("{msg}");

    let msg = JString::new(env, msg)?;
    let toast = Toast::make_text(env, &context, msg.as_char_sequence(), 0)?;
    toast.show(env)
}
```

</details>


<details>
<summary>Using the asynchronous broadcast waiter</summary>

```toml
jni-min-helper = { version = "0.4.3", features = ["futures"] }
```

```rust
use android_activity::{AndroidApp, MainEvent, PollEvent};
use jni_min_helper::{BroadcastWaiter, android_app_name, jni_with_env};
use std::time::Duration;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag(android_app_name().as_bytes()),
    );

    std::thread::spawn(background_loop);

    let mut on_destroy = false;
    loop {
        app.poll_events(None, |event| match event {
            PollEvent::Main(MainEvent::Destroy) => {
                on_destroy = true;
            }
            _ => (),
        });
        if on_destroy {
            return;
        }
    }
}

fn background_loop() {
    let mut waiter = BroadcastWaiter::build([
        "android.intent.action.TIME_TICK",
        "android.net.conn.CONNECTIVITY_CHANGE",
    ])
    .unwrap();
    log::info!("Built broadcast waiter.");
    // TODO: the android_main() thread should tell this thread to exit on stop/destroy event.
    loop {
        if let Some(intent) = waiter.wait_timeout(Duration::from_secs(1)) {
            let _ = jni_with_env(|env| {
                let action = intent.get_action(env)?;
                log::info!("Received an intent of action '{action}'.");
                Ok(())
            });
        }
    }
}
```

</details>

<details>
<summary>Receiving result from the chooser dialog</summary>

```rust
use android_activity::{AndroidApp, MainEvent, PollEvent};
use jni::{
    jni_str,
    objects::{JCharSequence, JString},
    refs::LoaderContext,
};
use jni_min_helper::{DynamicProxy, JInteger, android_app_name, jni_with_env};

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag(android_app_name().as_bytes()),
    );

    log::info!("starting dialog_test 1 in Rust `android_main` thread...");
    if dialog_test(app.clone(), true) {
        log::info!("starting dialog_test 2 in background thread...");
        let app = app.clone();
        // this will not dismiss the dialog on main stop event.
        std::thread::spawn(|| dialog_test(app, false));
    }

    let mut on_destroy = false;
    loop {
        app.poll_events(None, |event| match event {
            PollEvent::Main(MainEvent::Destroy) => {
                on_destroy = true;
            }
            _ => (),
        });
        if on_destroy {
            return;
        }
    }
}

fn dialog_test(app: AndroidApp, in_android_main: bool) -> bool {
    let result = chooser_dialog(&app, in_android_main, "Choose", &["i", "j", "k"]).unwrap();
    if let Some(c) = result {
        log::info!("The user choosed {c}.");
        true
    } else {
        log::info!("The dialog has been dismissed.");
        false
    }
}

jni::bind_java_type! {
    AndroidContext => "android.content.Context",
}

jni::bind_java_type! {
    AlertDialog => "android.app.AlertDialog",
    methods {
        fn show(),
        fn dismiss(),
    }
}
jni::bind_java_type! {
    AlertDialogBuilder => "android.app.AlertDialog$Builder",
    type_map = {
        AndroidContext => "android.content.Context",
        AlertDialog => "android.app.AlertDialog",
        DialogOnClickListener => "android.content.DialogInterface$OnClickListener",
        DialogOnDismissListener => "android.content.DialogInterface$OnDismissListener",
    },
    constructors {
        fn new(context: AndroidContext),
    },
    methods {
        fn set_title {
            name = "setTitle",
            sig = (title: JCharSequence) -> AlertDialogBuilder,
        },
        fn set_items_listener {
            name = "setItems",
            sig = (items: JCharSequence[], listener: DialogOnClickListener) -> AlertDialogBuilder,
        },
        fn set_on_dismiss_listener(on_dismiss_listener: DialogOnDismissListener) -> AlertDialogBuilder,
        fn create() -> AlertDialog,
    }
}
jni::bind_java_type! {
    DialogOnClickListener => "android.content.DialogInterface$OnClickListener",
}
jni::bind_java_type! {
    DialogOnDismissListener => "android.content.DialogInterface$OnDismissListener",
}

fn chooser_dialog<'a>(
    app: &AndroidApp,
    in_android_main: bool,
    title: &str,
    choices: &'a [&'a str],
) -> Result<Option<&'a str>, jni::errors::Error> {
    use jni::objects::{JObject, JObjectArray};
    use std::sync::{Arc, Mutex, mpsc};

    jni_with_env(|env| {
        let raw_activity_global = app.activity_as_ptr() as jni::sys::jobject;
        let context = unsafe {
            // SAFETY: The reference / pointer is valid as long as `app` is valid
            env.as_cast_raw::<jni::refs::Global<AndroidContext>>(&raw_activity_global)?
        };

        // creates the dialog builder
        let dialog_builder = AlertDialogBuilder::new(env, context)?;

        let title = JString::new(env, title)?;
        let choice_items = JObjectArray::<JString>::new(env, choices.len(), JString::null())?;
        for (i, choice_name) in choices.iter().enumerate() {
            let choice_name = JString::new(env, choice_name)?;
            choice_items.set_element(env, i, choice_name)?;
        }
        let choice_items = JObjectArray::<JCharSequence>::cast_local(env, choice_items)?;

        let (tx1, rx) = mpsc::channel();
        let tx2 = tx1.clone();

        // creates OnClickListener
        let on_click_listener = DynamicProxy::build(
            env,
            &LoaderContext::None,
            [jni_str!("android.content.DialogInterface$OnClickListener")],
            move |env, method, args| {
                if method.get_name(env)?.to_string() == "onClick" {
                    let i = args.get_element(env, 1)?;
                    let i = JInteger::cast_local(env, i)?;
                    let _ = tx1.send(Some(i.value(env)?));
                }
                Ok(JObject::null())
            },
        )?;
        let on_click_listener = env.as_cast::<DialogOnClickListener>(on_click_listener.as_ref())?;

        // creates OnDismissListener
        let on_dismiss_listener = DynamicProxy::build(
            env,
            &LoaderContext::None,
            [jni_str!(
                "android/content/DialogInterface$OnDismissListener"
            )],
            move |env, method, _| {
                if method.get_name(env)?.to_string() == "onDismiss" {
                    let _ = tx2.send(None);
                }
                Ok(JObject::null())
            },
        )?;
        let on_dismiss_listener =
            env.as_cast::<DialogOnDismissListener>(on_dismiss_listener.as_ref())?;

        // configure the dialog builder
        dialog_builder.set_items_listener(env, choice_items, on_click_listener)?;
        dialog_builder.set_on_dismiss_listener(env, on_dismiss_listener)?;
        dialog_builder.set_title(env, title.as_char_sequence())?;

        // creating and showing the dialog must be done in the Java main thread
        let dialog_builder = env.new_global_ref(dialog_builder)?;
        let dialog_arc = Arc::new(Mutex::new(None));
        let dialog_arc_2 = dialog_arc.clone(); // Note: a weak reference might be used instead
        DynamicProxy::post_to_main_looper(move |env| {
            let dialog = dialog_builder.create(env)?;
            dialog.show(env)?;
            let dialog = env.new_global_ref(dialog)?;
            dialog_arc_2.lock().unwrap().replace(dialog);
            Ok(())
        })?;

        if let Some(r) = wait_recv(&rx, app, in_android_main) {
            Ok(r.map(|i| choices[i as usize]))
        } else {
            let dialog = dialog_arc.lock().unwrap().take();
            if let Some(dialog) = dialog {
                dialog.dismiss(env)?;
            }
            Ok(None)
        }
    })
}

fn wait_recv<T>(
    rx: &std::sync::mpsc::Receiver<T>,
    app: &AndroidApp,
    in_android_main: bool,
) -> Option<T> {
    if in_android_main {
        // it runs in the native main thread
        let mut on_stop = false;
        loop {
            // `rx.recv()` may block forever.
            if let Ok(r) = rx.try_recv() {
                return Some(r);
            } else {
                // Let the native main thread process events from the Java main thread.
                // It's tested that `ndk::looper::ThreadLooper::poll_once()` doesn't work here,
                // check `android_activity::AndroidApp::poll_events()` documentation.
                app.poll_events(None, |event| {
                    if let PollEvent::Main(MainEvent::Stop) = event {
                        on_stop = true;
                    }
                });
                if on_stop {
                    return None;
                }
            }
        }
    } else {
        // it runs in another background thread
        rx.recv().ok()
    }
}
```

Note: this is not a perfect implementation.

</details>

<details>
<summary>Permission request</summary>

Please make sure the [cargo-apk2](https://docs.rs/crate/cargo-apk2/latest) is used for this test case, and `PermActivity.java` (which can be found in the source of this crate) is placed in the `java` subfolder. 

```toml
[package]
name = "android-simple-test"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
log = "0.4"
jni-min-helper = "0.4.3"
android-activity = { version = "0.6.1", features = ["native-activity"] }
android_logger = "0.15"

[lib]
name = "android_simple_test"
crate-type = ["cdylib"]
path = "lib.rs"

[package.metadata.android]
package = "com.example.android_simple_test"
build_targets = [ "aarch64-linux-android" ]

java_sources = "java"

[package.metadata.android.sdk]
min_sdk_version = 23
target_sdk_version = 33

[[package.metadata.android.uses_permission]]
name = "android.permission.READ_EXTERNAL_STORAGE"

[[package.metadata.android.uses_permission]]
name = "android.permission.WRITE_EXTERNAL_STORAGE"

[[package.metadata.android.application.activity]]
name = "android.app.NativeActivity"

[[package.metadata.android.application.activity.intent_filter]]
actions = ["android.intent.action.VIEW", "android.intent.action.MAIN"]
categories = ["android.intent.category.LAUNCHER"]

[[package.metadata.android.application.activity.meta_data]]
name = "android.app.lib_name"
value = "android_simple_test"

[[package.metadata.android.application.activity]]
name = "rust.jniminhelper.PermActivity"
```

```rust
use android_activity::{AndroidApp, MainEvent, PollEvent};
use jni_min_helper::{PermissionRequest, android_app_name};

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag(android_app_name().as_bytes()),
    );

    let mut request = PermissionRequest::request(
        "Read and write external files",
        [
            "android.permission.READ_EXTERNAL_STORAGE",
            "android.permission.WRITE_EXTERNAL_STORAGE",
        ],
    )
    .unwrap();

    let mut on_destroy = false;
    loop {
        app.poll_events(None, |event| match event {
            PollEvent::Main(MainEvent::Resume { loader: _, .. }) => {
                if request.is_some() {
                    if !PermissionRequest::is_pending() {
                        // `is_pending` returned false, this means `wait` will not block
                        let result = request.take().unwrap().wait();
                        log::info!("request result: {result:#?}");
                    }
                }
            }
            PollEvent::Main(MainEvent::Destroy) => {
                on_destroy = true;
            }
            _ => (),
        });
        if on_destroy {
            return;
        }
    }
}
```

</details>
