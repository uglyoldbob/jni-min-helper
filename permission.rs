use std::{
    ops::Deref,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

#[cfg(not(feature = "futures"))]
use std::sync::mpsc::{Receiver, Sender, channel};

#[cfg(feature = "futures")]
use futures_channel::oneshot::{Receiver, Sender, channel};

use crate::{
    AndroidActivity, AndroidApplication, DynamicProxy,
    android::{android_api_level, get_android_context},
    bindings::{AndroidLifecycleCallbacks, Intent},
    is_current_thread_main_looper, jni_with_env,
};

use jni::{
    Env,
    errors::Error,
    objects::{JClass, JIntArray, JObject, JObjectArray, JString, LoaderContext},
    refs::{Global, Reference},
};

const PERMISSION_GRANTED: i32 = 0;
const EXTRA_PERM_ARRAY: &str = "rust.jniminhelper.perm_array";
const EXTRA_TITLE: &str = "rust.jniminhelper.perm_activity_title";

jni::bind_java_type! {
    PermActivity => "rust.jniminhelper.PermActivity",
    native_methods {
        fn native_on_request_permissions_result {
            sig = (permissions: JString[], grant_results: jint[]) -> (),
            fn = perm_activity_result_handler,
        }
    },
}

type RequestResult = Vec<(String, bool)>;

static MUTEX_PERM_REQ: Mutex<Option<PendingRequest>> = Mutex::new(None);
struct PendingRequest {
    result_sender: Sender<RequestResult>,
    lifecycle_callbacks: DynamicProxy,
    wait_launch_flag: Option<WaitLaunchFlag>,
}

enum WaitLaunchFlag {
    WaitStart,                                   // wait the `PermActivity` to start and resume
    WaitPause(Global<AndroidActivity<'static>>), // wait the provided activity to pause
}

/// Android runtime permission request utility.
///
/// Using this utility *requires* the activity `rust.jniminhelper.PermActivity` to be declared
/// in the `AndroidManifest.xml`, and this activity must be compiled in the package's `classes.dex`
/// file. `PermActivity.java` can be found in the source code.
///
/// For native activity applications, `cargo-apk` does not support these things at the time of
/// publishing this version of `jni-min-helper` (`cargo-apk2` has introduced these features).
/// An alternative is the [Self::request_with_activity], which is probably less reliable.
pub struct PermissionRequest {
    receiver: Receiver<RequestResult>,
}

impl PermissionRequest {
    /// Checks if a permission is already granted.
    /// Returns `Error::MethodNotFound` if the Android API level is less than 23.
    pub fn has_permission(permission: &str) -> Result<bool, Error> {
        if android_api_level() < 23 {
            return Err(Error::MethodNotFound {
                name: "checkSelfPermission".to_string(),
                sig: "Android API level < 23".to_string(),
            });
        }
        jni_with_env(|env| {
            let context = get_android_context(env)?;
            let permission = JString::new(env, permission)?;
            context
                .check_self_permission(env, permission)
                .map(|i| i == PERMISSION_GRANTED)
        })
    }

    /// Returns true if there is an ongoing request managed by this crate.
    pub fn is_pending() -> bool {
        MUTEX_PERM_REQ.lock().unwrap().is_some()
    }

    /// Starts a permission request for permission names listed in `permissions`.
    /// Returns `Error::TryLock` if a previous request is unfinished;
    /// returns `Ok(None)` if all permissions are already granted or the Android
    /// API level is less than 23.
    pub fn request<'a>(
        title: &str,
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Result<Option<Self>, Error> {
        let Some((perms, mut req_lock)) = Self::build_check(permissions)? else {
            return Ok(None);
        };

        let receiver = jni_with_env(|env| {
            let context = get_android_context(env)?;
            let intent = Self::build_start_activity_intent(env, title, &perms)?;
            let (tx, rx) = channel();

            let callbacks = Self::build_activity_lifecycle_callback(env, &perms, true)?;

            req_lock.replace(PendingRequest {
                result_sender: tx,
                lifecycle_callbacks: callbacks,
                wait_launch_flag: Some(WaitLaunchFlag::WaitStart),
            });

            context.start_activity(env, &intent)?;
            Ok(rx)
        })
        .inspect_err(|_| {
            let _ = req_lock.take();
        })?;

        Ok(Some(Self { receiver }))
    }

    /// (Experimental) Alternative for [Self::request] that does not require a custom
    /// activity, for use with `NativeActivity`.
    ///
    /// `activity` can be obtained from `android_activity::AndroidApp::activity_as_ptr`.
    ///
    /// Inspired by <https://github.com/enomado/android-permission-lifecycle-probe>.
    pub fn request_with_activity<'local, 'a>(
        activity: impl AsRef<JObject<'local>>,
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Result<Option<Self>, Error> {
        let Some((perms, mut req_lock)) = Self::build_check(permissions)? else {
            return Ok(None);
        };

        let receiver = jni_with_env(|env| {
            let arr_perms = Self::build_jstring_array(env, &perms)?;
            let activity = env.as_cast::<AndroidActivity>(activity.as_ref())?;
            let (tx, rx) = channel();

            let callbacks = Self::build_activity_lifecycle_callback(env, &perms, false)?;

            req_lock.replace(PendingRequest {
                result_sender: tx,
                lifecycle_callbacks: callbacks,
                wait_launch_flag: Some(WaitLaunchFlag::WaitPause(
                    env.new_global_ref(activity.as_ref())?,
                )),
            });

            activity.request_permissions(env, arr_perms, 0)?;
            Ok(rx)
        })
        .inspect_err(|_| {
            let _ = req_lock.take();
        })?;

        Ok(Some(Self { receiver }))
    }

    #[allow(clippy::type_complexity)]
    fn build_check<'a>(
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Result<Option<(Vec<String>, MutexGuard<'static, Option<PendingRequest>>)>, Error> {
        if android_api_level() < 23 {
            return Ok(None);
        }
        let mut perms = Vec::new();
        for perm in permissions.into_iter() {
            if !Self::has_permission(perm)? {
                perms.push(perm.to_string());
            }
        }
        if perms.is_empty() {
            return Ok(None);
        }
        let lock = MUTEX_PERM_REQ.lock().unwrap();
        if lock.is_some() {
            return Err(Error::TryLock);
        }
        Ok(Some((perms, lock)))
    }

    fn build_jstring_array<'local>(
        env: &mut Env<'local>,
        perms: &[String],
    ) -> Result<JObjectArray<'local, JString<'local>>, Error> {
        let arr_perms = JObjectArray::<JString>::new(env, perms.len(), JString::null())?;
        for (i, perm) in perms.iter().enumerate() {
            let perm = JString::new(env, perm)?;
            arr_perms.set_element(env, i, perm)?;
        }
        Ok(arr_perms)
    }

    fn build_start_activity_intent<'local>(
        env: &mut Env<'local>,
        title: &str,
        perms: &[String],
    ) -> Result<Intent<'local>, Error> {
        // Activities are loaded by the application class loader.
        let loader = get_android_context(env)?.get_class_loader(env)?;
        let loader = jni::refs::LoaderContext::Loader(&loader);

        let _ = PermActivityAPI::get(env, &loader)?; // registers the callback dynamically
        let cls_perm = PermActivity::lookup_class(env, &loader)?;

        let context = get_android_context(env)?;
        let intent = Intent::new(env)?;
        use std::ops::Deref;
        intent.set_class(env, context, AsRef::<JClass>::as_ref(&cls_perm.deref()))?;

        let extra_title = JString::new(env, EXTRA_TITLE)?;
        let title = JString::new(env, title)?;
        intent.put_extra_string(env, extra_title, title)?;

        let arr_perms = Self::build_jstring_array(env, perms)?;
        let extra_perm_array = JString::new(env, EXTRA_PERM_ARRAY)?;
        intent.put_extra_string_array(env, &extra_perm_array, &arr_perms)?;

        let flag = Intent::FLAG_ACTIVITY_NEW_TASK(env)?;
        intent.set_flags(env, flag)?;

        Ok(intent)
    }

    fn build_activity_lifecycle_callback(
        env: &mut Env,
        perms: &[String],
        for_perm_activity: bool,
    ) -> Result<DynamicProxy, Error> {
        let ctx = get_android_context(env)?;
        let app = env.as_cast::<AndroidApplication>(ctx)?;
        let perms_for_handler = perms.to_owned();
        let proxy = DynamicProxy::build(
            env,
            &jni::refs::LoaderContext::None,
            [jni::jni_str!(
                "android.app.Application$ActivityLifecycleCallbacks"
            )],
            move |env, method, args| {
                let callback_name = method.get_name(env)?.to_string();
                let activity = args.get_element(env, 0)?;
                let activity = env.cast_local::<AndroidActivity>(activity)?;
                on_activity_lifecycle_callback(env, &callback_name, &activity, &perms_for_handler)?;
                // Every `ActivityLifecycleCallbacks` method returns void.
                Ok(JObject::null())
            },
        )?;
        let callbacks = env.as_cast::<AndroidLifecycleCallbacks>(proxy.as_ref())?;
        app.register_activity_lifecycle_callbacks(env, callbacks)?;

        // The request dialog might not be opened if the app is in background state,
        // so wait for a while and check if it has launched.
        std::thread::spawn(move || {
            // XXX: the delay may be longer than 1 sec on old Android versions
            // if the app was in background state but activated for the dialog;
            // but setting sleep duration longer here may be troublesome for the
            // `WaitPause` (native activity) variant because it may pause for
            // reasons other than showing the permission dialog.
            let dur = if for_perm_activity {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(1)
            };
            std::thread::sleep(dur);
            let mut req_lock = MUTEX_PERM_REQ.lock().unwrap();
            if let Some(pending) = req_lock.as_ref() {
                if pending.wait_launch_flag.is_none() {
                    return;
                }
                let req = req_lock.take().unwrap();
                let _ = req.result_sender.send(Vec::new());
                let _ = jni_with_env(|env| {
                    PermissionRequest::drop_activity_lifecycle_callback(
                        env,
                        req.lifecycle_callbacks,
                    )
                });
            }
        });

        Ok(proxy)
    }

    fn drop_activity_lifecycle_callback(env: &mut Env, proxy: DynamicProxy) -> Result<(), Error> {
        let ctx = get_android_context(env)?;
        let app = env.as_cast::<AndroidApplication>(ctx)?;
        let callbacks = env.as_cast::<AndroidLifecycleCallbacks>(proxy.as_ref())?;
        app.unregister_activity_lifecycle_callbacks(env, callbacks)?;
        Ok(())
    }

    /// Blocks on waiting the permission request and returns the result.
    /// If the request itself failed, this returns an empty `Vec`.
    ///
    /// Warning: blocking in `android_main()` thread of `android_activity` may block
    /// the future's completion if it depends on event processing of the main looper.
    #[allow(unused_mut)]
    pub fn wait(mut self) -> RequestResult {
        if is_current_thread_main_looper().unwrap_or(false) {
            #[cfg(not(feature = "futures"))]
            let res = self.receiver.try_recv().ok();
            #[cfg(feature = "futures")]
            let res = self.receiver.try_recv().ok().flatten();
            if res.is_none() {
                warn!("`PermissionRequest::wait` called in main looper, not waiting.");
            }
            return res.unwrap_or_default();
        }
        #[cfg(not(feature = "futures"))]
        {
            self.receiver.recv().unwrap_or_default()
        }
        #[cfg(feature = "futures")]
        {
            futures_lite::future::block_on(self).unwrap_or_default()
        }
    }
}

#[cfg(feature = "futures")]
impl std::future::Future for PermissionRequest {
    type Output = Result<RequestResult, futures_channel::oneshot::Canceled>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use futures_lite::FutureExt;
        self.receiver.poll(cx)
    }
}

fn perm_activity_result_handler<'local>(
    env: &mut Env<'local>,
    _this: PermActivity<'local>,
    permissions: JObjectArray<'local, jni::objects::JString<'local>>,
    grant_results: JIntArray<'local>,
) -> Result<(), Error> {
    let Some(req) = MUTEX_PERM_REQ.lock().unwrap().take() else {
        warn!("Unexpected: perm_callback() received, but MUTEX_PERM_REQ is None.");
        return Ok(());
    };

    PermissionRequest::drop_activity_lifecycle_callback(env, req.lifecycle_callbacks)?;

    if permissions.is_null() || grant_results.is_null() {
        // it should be unreachable
        warn!("Unexpected: perm_callback() received null.");
        let _ = req.result_sender.send(Vec::new());
        return Err(Error::NullPtr("Unexpected: perm_callback() received null."));
    }

    let mut result = Vec::new();

    let mut grant_vals = vec![0; grant_results.len(env)?];
    grant_results.get_region(env, 0, &mut grant_vals)?;
    for (i, &res_val) in grant_vals.iter().enumerate() {
        result.push((
            permissions.get_element(env, i)?.to_string(),
            res_val == PERMISSION_GRANTED,
        ));
    }

    if let Err(e) = req.result_sender.send(result) {
        warn!("Error in perm_callback(): sender.send() failed: {e:?}.");
    }
    Ok(())
}

fn on_activity_lifecycle_callback<'local>(
    env: &mut Env<'local>,
    callback_name: &str,
    activity: &AndroidActivity<'local>,
    checked_perms: &[String],
) -> Result<(), Error> {
    log::debug!(
        "on_activity_lifecycle_callback: {} of {}",
        callback_name,
        env.get_object_class(activity)?.get_name(env)?
    );

    let mut req_lock = MUTEX_PERM_REQ.lock().unwrap();

    let Some(pending) = req_lock.as_mut() else {
        return Ok(());
    };

    if let Some(wait_launch) = pending.wait_launch_flag.as_ref() {
        match wait_launch {
            WaitLaunchFlag::WaitStart if callback_name == "onActivityResumed" => {
                let cls_perm_activity = PermActivity::lookup_class(env, &LoaderContext::None)?;
                if env.is_instance_of(activity, cls_perm_activity.deref())? {
                    let _ = pending.wait_launch_flag.take();
                }
            }
            // XXX: if the application is in background state, `requestPermissions`
            // may start (but not resume) the activity on some Android versions,
            // instead of doing nothing. Is it problematic to have `onActivityStarted`
            // accepted by the match guard of `WaitPause` ?
            WaitLaunchFlag::WaitPause(checked_activity)
                if (callback_name == "onActivityPaused")
                    && env.is_same_object(checked_activity, activity)? =>
            {
                let _ = pending.wait_launch_flag.take();
            }
            _ => (),
        }
    } else if callback_name == "onActivityResumed" {
        let req = req_lock.take().unwrap();
        drop(req_lock);

        // An activity just resumed, so the dialog is gone. `checkSelfPermission`
        // now reflects the user's choice. (verified by the probe: the permission
        // status is already updated by the time `Resume` fires).

        let mut result = Vec::with_capacity(checked_perms.len());
        for perm in checked_perms {
            let granted = PermissionRequest::has_permission(perm)?;
            result.push((perm.clone(), granted));
        }
        let _ = req.result_sender.send(result);
        PermissionRequest::drop_activity_lifecycle_callback(env, req.lifecycle_callbacks)?;
    }

    Ok(())
}
