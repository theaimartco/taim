//! `taim-new`: creates a TAIM application that runs on the desktop and on
//! Android, like `flutter create`.
//!
//! ```text
//! cargo run -p taim-new -- my_notes
//! cargo run -p taim-new -- my_notes --name "My Notes" --package com.example.notes
//! cargo run -p taim-new -- my_notes --taim-path ../taim
//! ```
//!
//! The project it writes builds with `cargo run`, and its `android/`
//! folder is a Gradle project that builds the Rust library with
//! `cargo-ndk` and packages it as an APK or Play Store bundle.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The template, file by file: where each goes and what it holds.
const TEMPLATE: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../template/Cargo.toml")),
    ("README.md", include_str!("../template/README.md")),
    (".gitignore", include_str!("../template/gitignore")),
    (
        "rust-toolchain.toml",
        include_str!("../template/rust-toolchain.toml"),
    ),
    ("src/lib.rs", include_str!("../template/src/lib.rs")),
    ("src/main.rs", include_str!("../template/src/main.rs")),
    (
        "android/settings.gradle.kts",
        include_str!("../template/android/settings.gradle.kts"),
    ),
    (
        "android/build.gradle.kts",
        include_str!("../template/android/build.gradle.kts"),
    ),
    (
        "android/gradle.properties",
        include_str!("../template/android/gradle.properties"),
    ),
    (
        "android/app/build.gradle.kts",
        include_str!("../template/android/app/build.gradle.kts"),
    ),
    (
        "android/app/src/main/AndroidManifest.xml",
        include_str!("../template/android/app/src/main/AndroidManifest.xml"),
    ),
];

/// Template files copied byte for byte: the Gradle wrapper, so a project
/// builds without Gradle installed. `true` marks an executable.
const BINARY: &[(&str, &[u8], bool)] = &[
    // TAIM's Android host (`TaimActivity`): native text input, insets,
    // TalkBack, back navigation and platform services. Copied verbatim.
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimAccessibility.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimAccessibility.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimActivity.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimActivity.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimBack.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimBack.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimBiometric.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimBiometric.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimBluetooth.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimBluetooth.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimCameraView.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimCameraView.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimExtension.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimExtension.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimForeground.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimForeground.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimForegroundService.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimForegroundService.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimGeofence.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimGeofence.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimGeofenceReceiver.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimGeofenceReceiver.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimLocation.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimLocation.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimMedia.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimMedia.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimMediaProvider.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimMediaProvider.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimNativeViewFactory.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimNativeViewFactory.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimNativeViews.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimNativeViews.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimNfc.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimNfc.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimNotifications.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimNotifications.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimPermissions.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimPermissions.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimPlugin.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimPlugin.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimSensors.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimSensors.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimServices.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimServices.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimTextureView.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimTextureView.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimVideoView.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimVideoView.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimWebView.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimWebView.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimWork.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimWork.java"),
        false,
    ),
    (
        "android/app/src/main/java/com/theaimartco/taim/TaimWorkService.java",
        include_bytes!("../../../winit/android/com/theaimartco/taim/TaimWorkService.java"),
        false,
    ),
    (
        "android/gradlew",
        include_bytes!("../template/android/gradlew"),
        true,
    ),
    (
        "android/gradlew.bat",
        include_bytes!("../template/android/gradlew.bat"),
        false,
    ),
    (
        "android/gradle/wrapper/gradle-wrapper.jar",
        include_bytes!("../template/android/gradle/wrapper/gradle-wrapper.jar"),
        false,
    ),
    (
        "android/gradle/wrapper/gradle-wrapper.properties",
        include_bytes!("../template/android/gradle/wrapper/gradle-wrapper.properties"),
        false,
    ),
];

const USAGE: &str = "\
usage: taim-new <directory> [--name <display name>] [--package <android package>]
                            [--taim-path <path to a TAIM checkout> | --taim-git <url>]";

/// What to create.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Project {
    directory: PathBuf,
    /// The Cargo package (and binary) name.
    krate: String,
    /// The name people see.
    name: String,
    /// The Android application id.
    package: String,
    /// Where Cargo finds TAIM, as a dependency's source fields.
    taim_source: String,
}

impl Project {
    fn from_args(args: &[String]) -> Result<Self, String> {
        let mut directory = None;
        let mut name = None;
        let mut package = None;
        let mut taim_source = None;
        let mut args = args.iter();

        while let Some(arg) = args.next() {
            let mut value = |flag: &str| {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value"))
            };
            match arg.as_str() {
                "--name" => name = Some(value("--name")?),
                "--package" => package = Some(value("--package")?),
                "--taim-path" => {
                    let path = value("--taim-path")?;
                    let absolute = std::fs::canonicalize(&path)
                        .map_err(|error| format!("--taim-path {path}: {error}"))?;
                    taim_source = Some(format!(
                        "path = {}",
                        toml_string(&absolute.to_string_lossy())
                    ));
                }
                "--taim-git" => {
                    taim_source = Some(format!("git = {}", toml_string(&value("--taim-git")?)));
                }
                "-h" | "--help" => return Err(USAGE.to_owned()),
                flag if flag.starts_with('-') => {
                    return Err(format!("unknown option {flag}\n{USAGE}"));
                }
                path if directory.is_none() => directory = Some(PathBuf::from(path)),
                extra => return Err(format!("unexpected argument {extra}\n{USAGE}")),
            }
        }

        let directory = directory.ok_or_else(|| USAGE.to_owned())?;
        let stem = directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| "the directory needs a name".to_owned())?;
        let krate = crate_name(&stem).ok_or_else(|| {
            format!("{stem:?} cannot become a crate name: use letters, digits and `_`")
        })?;
        let name = name.unwrap_or_else(|| display_name(&krate));
        let package = package.unwrap_or_else(|| format!("com.example.{}", krate.replace('_', "")));
        if !valid_package(&package) {
            return Err(format!(
                "{package:?} is not an Android package name (like com.example.app)"
            ));
        }
        if name
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '<' | '>' | '&' | '\\'))
        {
            return Err("the display name cannot contain quotes, <, >, & or \\".to_owned());
        }

        Ok(Self {
            directory,
            krate,
            name,
            package,
            taim_source: taim_source.unwrap_or_else(|| {
                format!(
                    "git = {}",
                    toml_string("https://github.com/theaimartco/taim")
                )
            }),
        })
    }

    /// `text` with the template's placeholders filled in.
    fn fill(&self, text: &str) -> String {
        let gradle_name: String = self
            .name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();

        text.replace("{{crate}}", &self.krate)
            .replace("{{lib}}", &self.krate)
            .replace("{{name}}", &self.name)
            .replace("{{package}}", &self.package)
            .replace("{{taim_source}}", &self.taim_source)
            .replace(
                "{{gradle_name}}",
                if gradle_name.is_empty() {
                    "App"
                } else {
                    &gradle_name
                },
            )
    }

    /// Writes the project. Refuses to touch a directory that has files.
    fn create(&self) -> Result<(), String> {
        if let Ok(mut entries) = std::fs::read_dir(&self.directory)
            && entries.next().is_some()
        {
            return Err(format!(
                "{} already exists and is not empty",
                self.directory.display()
            ));
        }

        for (path, contents) in TEMPLATE {
            let target = self.directory.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            std::fs::write(&target, self.fill(contents))
                .map_err(|error| format!("{}: {error}", target.display()))?;
        }

        for (path, bytes, executable) in BINARY {
            let target = self.directory.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            std::fs::write(&target, bytes)
                .map_err(|error| format!("{}: {error}", target.display()))?;

            #[cfg(unix)]
            if *executable {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
                    .map_err(|error| format!("{}: {error}", target.display()))?;
            }
            #[cfg(not(unix))]
            let _ = executable;
        }
        Ok(())
    }
}

/// A Cargo package name from a directory name: lowercase, `-` as `_`,
/// starting with a letter.
fn crate_name(stem: &str) -> Option<String> {
    let name: String = stem
        .chars()
        .map(|c| {
            if c == '-' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();

    let valid = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    valid.then_some(name)
}

/// "my_notes" becomes "My Notes".
fn display_name(krate: &str) -> String {
    krate
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Android application ids: two or more dot-separated segments, each a
/// letter followed by letters, digits or `_`.
fn valid_package(package: &str) -> bool {
    let segments: Vec<_> = package.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

fn toml_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let project = match Project::from_args(&args) {
        Ok(project) => project,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(message) = project.create() {
        eprintln!("taim-new: {message}");
        return ExitCode::FAILURE;
    }

    println!(
        "Created {} ({}, Android package {}).\n\n  cd {}\n  cargo run                       # on the desktop\n  cd android && gradle installDebug   # on a phone; see README.md",
        project.name,
        project.krate,
        project.package,
        display(&project.directory),
    );
    ExitCode::SUCCESS
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn names_come_from_the_directory() {
        let project = Project::from_args(&args(&["apps/My-Notes"])).expect("valid");

        assert_eq!(project.krate, "my_notes");
        assert_eq!(project.name, "My Notes");
        assert_eq!(project.package, "com.example.mynotes");
        assert!(project.taim_source.starts_with("git = "));
    }

    #[test]
    fn bad_names_and_packages_are_refused() {
        assert!(Project::from_args(&args(&["9lives"])).is_err());
        assert!(Project::from_args(&args(&["ok", "--package", "nodots"])).is_err());
        assert!(Project::from_args(&args(&["ok", "--package", "com.1bad"])).is_err());
        assert!(Project::from_args(&args(&["ok", "--name", "a\"b"])).is_err());
        assert!(Project::from_args(&args(&[])).is_err());
    }

    #[test]
    fn every_placeholder_is_filled() {
        let project = Project::from_args(&args(&[
            "notes",
            "--name",
            "Notes",
            "--package",
            "dev.taim.notes",
        ]))
        .expect("valid");

        for (path, contents) in TEMPLATE {
            let filled = project.fill(contents);
            assert!(!filled.contains("{{"), "{path} still has a placeholder");
        }

        let file = |name: &str| {
            let (_, contents) = TEMPLATE
                .iter()
                .find(|(path, _)| *path == name)
                .expect("in the template");
            project.fill(contents)
        };
        let manifest = file("android/app/src/main/AndroidManifest.xml");
        assert!(manifest.contains(r#"android:value="notes""#));
        let gradle = file("android/app/build.gradle.kts");
        assert!(gradle.contains(r#"applicationId = "dev.taim.notes""#));
    }

    #[test]
    fn creating_writes_the_project_and_never_overwrites() {
        let directory =
            std::env::temp_dir().join(format!("taim-new-test-{}/demo_app", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let project = Project::from_args(&args(&[&directory.to_string_lossy()])).expect("valid");

        project.create().expect("creates");
        assert!(directory.join("src/lib.rs").is_file());
        assert!(
            directory
                .join("android/app/src/main/AndroidManifest.xml")
                .is_file()
        );
        assert!(
            directory
                .join("android/gradle/wrapper/gradle-wrapper.jar")
                .is_file()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(directory.join("android/gradlew"))
                .expect("written")
                .permissions()
                .mode();
            assert!(mode & 0o111 != 0, "gradlew is executable");
        }
        assert!(
            std::fs::read_to_string(directory.join("Cargo.toml"))
                .expect("written")
                .contains(r#"name = "demo_app""#)
        );

        assert!(
            project.create().is_err(),
            "an existing project is left alone"
        );
        let _ = std::fs::remove_dir_all(directory.parent().expect("parent"));
    }
}
