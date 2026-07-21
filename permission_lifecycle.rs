//! Dex-free runtime permission request, driven by `ActivityLifecycleCallbacks`.
//!
//! # Why this exists
//!
//! [`PermissionRequest`](crate::PermissionRequest) works by launching a bundled
//! Java `PermActivity`, which means that activity must be compiled into the
//! package's `classes.dex`. `cargo-apk` cannot do that (only `cargo-apk2` can),
//! so pure-`cargo-apk` / `NativeActivity` projects cannot use it at all.
//!
//! This is the alternative the maintainer floated in
//! <https://github.com/rust-mobile/android-activity/issues/174>: instead of a
//! custom activity that receives `onRequestPermissionsResult`, call
//! `Activity.requestPermissions(...)` directly and observe the outcome through
//! the activity lifecycle. Under `NativeActivity` there is no result callback,
//! but the dialog *does* pause the activity underneath and resume it on
//! dismissal — so "the activity resumed" is a usable signal that the dialog
//! closed, and by then `checkSelfPermission` already reflects the user's choice.
//!
//! That behaviour was measured on a device before writing this — the raw log and
//! the probe are at
//! <https://github.com/enomado/android-permission-lifecycle-probe>:
//!
//! * user answers the dialog  → real `Pause`/`Resume` (not merely a focus
//!   change), and `checkSelfPermission` is already updated by the time `Resume`
//!   arrives, so re-checking on resume is race-free;
//! * user walks away without answering, then returns → `Start` **without**
//!   `Resume` (the dialog is restored on top), so the request correctly stays
//!   pending rather than being falsely reported as closed.
//!
//! # Two design points left for the maintainer (this is a starting point)
//!
//! 1. **The caller passes its `Activity`.** `requestPermissions` is an
//!    `Activity`-only method, and this crate deliberately holds only the
//!    `Application` (see [`android_context`](crate::android_context) and
//!    android-activity#228). There is no framework-agnostic way to *derive* the
//!    current activity from an `Application` alone, so the caller supplies it —
//!    e.g. `android_activity`'s `AndroidApp::activity_as_ptr()`. The lifecycle
//!    callbacks themselves are registered on the `Application`, which this crate
//!    already has.
//! 2. **Re-check-on-every-resume.** The handler re-reads `checkSelfPermission`
//!    for the requested permissions on *any* `onActivityResumed` and completes on
//!    the first resume where the pending request is still live. This is simple
//!    and matched the measurement, but a stricter implementation might want to
//!    correlate the resume with *this* request's activity.

use std::sync::Mutex;

#[cfg(not(feature = "futures"))]
use std::sync::mpsc::{Receiver, Sender, channel};

#[cfg(feature = "futures")]
use futures_channel::oneshot::{Receiver, Sender, channel};

use crate::{
    PermissionRequest,
    android::{android_api_level, android_context, get_android_context, get_helper_class_loader},
    jni_with_env,
    proxy::DynamicProxy,
};

use jni::{
    JValue, jni_sig, jni_str,
    errors::Error,
    objects::{JObject, JObjectArray, JString},
    refs::LoaderContext,
};

const PERMISSION_GRANTED: i32 = 0;

// The framework interface we proxy is `android.app.Application$ActivityLifecycleCallbacks`.
// It lives on the boot classpath, so it is resolved by name at runtime — no embedded
// class/dex data, which is the whole point of this path. (`jni_str!` requires a literal,
// so the name is written inline at the call site rather than via a `const`.)

type RequestResult = Vec<(String, bool)>;

/// Holds the sender for the single in-flight request. `Some` between
/// [`PermissionRequestLifecycle::request`] and the resume that resolves it;
/// taken (→ `None`) exactly once, when the result is delivered. Mirrors the
/// `MUTEX_PERM_REQ` guard used by the dex-based path.
static MUTEX_PERM_REQ: Mutex<Option<Sender<RequestResult>>> = Mutex::new(None);

/// Dex-free counterpart of [`PermissionRequest`], resolved via the activity
/// lifecycle instead of a bundled `PermActivity`.
///
/// On `drop` the lifecycle callbacks are unregistered from the `Application`.
pub struct PermissionRequestLifecycle {
    receiver: Receiver<RequestResult>,
    /// Kept alive so the Java proxy (and its backing Rust closure) outlives the
    /// registration; taken on `drop` to unregister. Always `Some` until `drop`.
    proxy: Option<DynamicProxy>,
}

impl PermissionRequestLifecycle {
    /// Returns true if there is an ongoing lifecycle-driven request.
    pub fn is_pending() -> bool {
        MUTEX_PERM_REQ.lock().unwrap().is_some()
    }

    /// Starts a permission request against `activity` (an `android.app.Activity`
    /// JNI reference the caller owns), resolved via `ActivityLifecycleCallbacks`.
    ///
    /// Returns `Ok(None)` if every permission is already granted or the Android
    /// API level is below 23; `Err(Error::TryLock)` if a previous lifecycle
    /// request is still unfinished.
    ///
    /// Contract: `activity` must be a valid reference to the *current* Activity
    /// (the one visible to the user) — that is the object `requestPermissions`
    /// posts its dialog from.
    pub fn request<'a>(
        activity: &JObject,
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Result<Option<Self>, Error> {
        if android_api_level() < 23 {
            return Ok(None);
        }
        if Self::is_pending() {
            return Err(Error::TryLock);
        }

        // Keep only the not-yet-granted permissions; nothing to do if empty.
        let mut perms = Vec::new();
        for perm in permissions.into_iter() {
            if !PermissionRequest::has_permission(perm)? {
                perms.push(perm.to_string());
            }
        }
        if perms.is_empty() {
            return Ok(None);
        }

        jni_with_env(|env| {
            // The set the resume handler re-checks. Moved into the closure.
            let perms_for_handler = perms.clone();

            // Build the ActivityLifecycleCallbacks proxy. The interface is a
            // framework class → passed by name; the *helper* class loader is used
            // as the proxy's defining loader so it can also see `InvocHdl`.
            let loader = get_helper_class_loader()?;
            let proxy = DynamicProxy::build(
                env,
                &LoaderContext::Loader(loader),
                [jni_str!("android.app.Application$ActivityLifecycleCallbacks")],
                move |env, method, _args| {
                    // Every ActivityLifecycleCallbacks method returns void; all we
                    // act on is a resume, and only while our request is live.
                    if &method.get_name(env)?.to_string() != "onActivityResumed" {
                        return Ok(JObject::null());
                    }
                    let Some(sender) = MUTEX_PERM_REQ.lock().unwrap().take() else {
                        // Not our resume (no request pending) — ignore.
                        return Ok(JObject::null());
                    };

                    // The dialog is gone; `checkSelfPermission` now reflects the
                    // user's choice (verified by the probe: already updated by the
                    // time `Resume` fires).
                    let context = get_android_context();
                    let mut result = Vec::with_capacity(perms_for_handler.len());
                    for perm in &perms_for_handler {
                        let jperm = JString::new(env, perm)?;
                        let granted =
                            context.check_self_permission(env, jperm)? == PERMISSION_GRANTED;
                        result.push((perm.clone(), granted));
                    }
                    let _ = sender.send(result);
                    Ok(JObject::null())
                },
            )?;

            // Register the callbacks on the Application (this crate's context IS
            // the process Application) BEFORE firing the request: the dialog
            // pauses the activity within ~2 ms of the call returning, so a
            // listener armed afterwards could miss the transition.
            env.call_method(
                android_context(),
                jni_str!("registerActivityLifecycleCallbacks"),
                jni_sig!("(Landroid/app/Application$ActivityLifecycleCallbacks;)V"),
                &[JValue::Object(proxy.as_ref())],
            )?;

            // Fire the system dialog on the caller's activity.
            let arr = JObjectArray::<JString>::new(env, perms.len(), JString::null())?;
            for (i, perm) in perms.iter().enumerate() {
                let perm = JString::new(env, perm)?;
                arr.set_element(env, i, perm)?;
            }
            let (tx, rx) = channel();
            MUTEX_PERM_REQ.lock().unwrap().replace(tx);

            // requestCode is irrelevant: under NativeActivity the result callback
            // never arrives — that is the whole reason for this lifecycle path.
            let request = env.call_method(
                activity,
                jni_str!("requestPermissions"),
                jni_sig!("([Ljava/lang/String;I)V"),
                &[JValue::Object(&arr), JValue::Int(0)],
            );
            if let Err(e) = request {
                // Roll back the pending-request guard so the crate isn't wedged.
                let _ = MUTEX_PERM_REQ.lock().unwrap().take();
                let _ = unregister(env, &proxy);
                return Err(e);
            }

            Ok(Some(Self {
                receiver: rx,
                proxy: Some(proxy),
            }))
        })
    }

    /// Blocks until the request resolves and returns the per-permission result.
    ///
    /// Warning: blocking the `android_main()` thread will deadlock if the
    /// resume that resolves this request is dispatched on that same thread —
    /// check your glue crate (e.g. `android_activity`).
    pub fn wait(self) -> RequestResult {
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
impl std::future::Future for PermissionRequestLifecycle {
    type Output = Result<RequestResult, futures_channel::oneshot::Canceled>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use futures_lite::FutureExt;
        self.receiver.poll(cx)
    }
}

impl Drop for PermissionRequestLifecycle {
    fn drop(&mut self) {
        if let Some(proxy) = self.proxy.take() {
            let _ = jni_with_env(|env| unregister(env, &proxy));
            // Clear any still-pending guard belonging to this request.
            let _ = MUTEX_PERM_REQ.lock().unwrap().take();
        }
    }
}

/// `Application.unregisterActivityLifecycleCallbacks(proxy)`.
fn unregister(env: &mut jni::Env, proxy: &DynamicProxy) -> Result<(), Error> {
    env.call_method(
        android_context(),
        jni_str!("unregisterActivityLifecycleCallbacks"),
        jni_sig!("(Landroid/app/Application$ActivityLifecycleCallbacks;)V"),
        &[JValue::Object(proxy.as_ref())],
    )?;
    Ok(())
}
