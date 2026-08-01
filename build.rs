// For the Android target, new source files and even .jar dependencies can be added easily:
// add the jar in `class_paths` of `compile_java_source` and `jar_dependencies` of `build_dex_file`.
// Note: Newer JDK versions (including JDK 21 and above) may not work with Android D8
// if there are anonymous classes in the Java code, which produces files like `Cls$1.class`
// (fixed in build tools 35.0.0). Currently `jni-min-helper` doesn't use anonymous classes.

use std::{env, fs, path::PathBuf};

use android_build::{Dexer, JavaBuild};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let src_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("java");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    #[cfg(windows)]
    unsafe {
        // TODO: fix the possible unicode output problem.
        // Safety: this sets the variable for the current single-thread process
        // of the executable compiled from this build script.
        env::set_var("JAVA_TOOL_OPTIONS", "-Duser.language=en");
    }

    if target_os == "android" {
        let sources = [
            src_dir.join("InvocHdl.java"),
            src_dir.join("BroadcastRec.java"),
            src_dir.join("RunOnce.java"),
        ];
        let android_jar = android_build::android_jar(None);

        let out_cls_dir = out_dir.join("classes");
        if out_cls_dir.try_exists().unwrap() {
            fs::remove_dir_all(&out_cls_dir).unwrap();
        }
        fs::create_dir(&out_cls_dir).unwrap();

        let mut err_string = None;
        if android_jar.is_none() {
            err_string.replace("Failed to find android.jar.".to_string());
        } else if let Err(s) =
            compile_java_source(sources, [android_jar.clone().unwrap()], out_cls_dir.clone())
        {
            err_string.replace(s);
        } else if let Err(s) = build_dex_file(out_cls_dir.clone(), android_jar, [], out_dir.clone())
        {
            err_string.replace(s);
        };

        if let Some(s) = err_string {
            for line in s.lines() {
                println!("cargo::warning={line}");
            }
            println!("cargo::warning=Falling back to the unmanaged prebuilt dex.");
            let prebuilt_dex_path = src_dir.join("classes.dex");
            let out_dex_path = out_dir.join("classes.dex");
            fs::copy(prebuilt_dex_path, out_dex_path)
                .expect("Failed to access the prebuilt dex file");
        }
    } else {
        println!("Building for PC platform...");
        if let Err(s) = compile_java_source([src_dir.join("InvocHdl.java")], [], out_dir.clone()) {
            for line in s.lines() {
                println!("cargo::warning={line}");
            }
            println!("cargo::warning=Falling back to the unmanaged prebuilt class.");
            let out_class_file_dir = out_dir.join("rust").join("jniminhelper");
            if !out_class_file_dir.try_exists().unwrap() {
                fs::DirBuilder::new()
                    .recursive(true)
                    .create(&out_class_file_dir)
                    .expect("Failed to create output directories");
            }
            let prebuilt_class_path = src_dir.join("InvocHdl.class");
            let out_class_path = out_class_file_dir.join("InvocHdl.class");
            fs::copy(prebuilt_class_path, out_class_path)
                .expect("Failed to access the prebuilt class file");
        }
    }
}

fn compile_java_source(
    source_paths: impl IntoIterator<Item = PathBuf>,
    class_paths: impl IntoIterator<Item = PathBuf>,
    output_dir: PathBuf,
) -> Result<(), String> {
    let mut java_build = JavaBuild::new();

    for java_src in source_paths {
        println!("cargo:rerun-if-changed={}", java_src.to_string_lossy());
        java_build.file(java_src);
    }

    for class_path in class_paths {
        println!("cargo:rerun-if-changed={}", class_path.to_string_lossy());
        java_build.class_path(class_path);
    }

    java_build.java_source_version(8).java_target_version(8);
    java_build.classes_out_dir(output_dir);

    // Execute the command
    let result = java_build
        .command()
        .map_err(|e| e.to_string())?
        .output()
        .map_err(|e| format!("Failed to execute javac: {e:?}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Java compilation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ))
    }
}

fn build_dex_file(
    compiled_classes_path: PathBuf,
    android_jar: Option<PathBuf>,
    jar_dependencies: impl IntoIterator<Item = PathBuf>,
    output_dir: PathBuf,
) -> Result<(), String> {
    let mut dexer = Dexer::new();
    if let Some(android_jar) = android_jar {
        dexer.android_jar(&android_jar);
    }
    let dependencies: Vec<_> = jar_dependencies.into_iter().collect();
    for dependency in dependencies.iter() {
        println!("cargo:rerun-if-changed={}", dependency.to_string_lossy());
        dexer.class_path(dependency);
    }
    dexer
        .android_min_api(20)
        .release(env::var("PROFILE").as_ref().map(|s| s.as_str()) == Ok("release"))
        .class_path(&compiled_classes_path)
        .no_desugaring(true)
        .out_dir(output_dir)
        .files(dependencies.iter())
        .collect_classes(&compiled_classes_path)
        .map_err(|e| e.to_string())?;

    // Execute the command
    let result = dexer
        .run()
        .map_err(|e| format!("Failed to execute d8.jar: {e:?}"))?;
    if result.success() {
        Ok(())
    } else {
        Err(format!("Dexer invocation failed: {result}"))
    }
}
