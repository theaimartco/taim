//! Physical-device biometric acceptance fixture. Each result is logged as
//! `TAIM_BIOMETRIC <step> <result>` for scripts/android/biometric-probe.py.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::android::{
        BiometricPolicy, BiometricPrompt, BiometricStrength, ServiceError, services,
    };
    use taim::prelude::*;
    let result = taim::app("Taim biometric", |cx| {
        let status = cx.signal("Biometric: not checked".to_owned());
        let last = cx.signal("Result: none".to_owned());
        let tasks = cx.tasks();
        let report = move |step: &'static str, value: String| {
            eprintln!("TAIM_BIOMETRIC {step} {value}");
            let _ = last.set(format!("{step}: {value}"));
        };
        let check = {
            let tasks = tasks.clone();
            move || {
                let _ = tasks.spawn(async move {
                    let mut lines = Vec::new();
                    for (name, policy) in [
                        ("strong", BiometricPolicy::default()),
                        (
                            "weak",
                            BiometricPolicy {
                                strength: BiometricStrength::Weak,
                                device_credential: false,
                            },
                        ),
                        (
                            "strong+credential",
                            BiometricPolicy {
                                strength: BiometricStrength::Strong,
                                device_credential: true,
                            },
                        ),
                    ] {
                        let result = match services() {
                            Some(client) => client.biometric_status(policy).await,
                            None => Err(ServiceError::Unavailable),
                        };
                        let text = match result {
                            Ok(s) => format!(
                                "{:?} secure={} fingerprint={} face={}",
                                s.availability, s.device_secure, s.fingerprint, s.face
                            ),
                            Err(e) => format!("Err({e:?})"),
                        };
                        eprintln!("TAIM_BIOMETRIC status-{name} {text}");
                        lines.push(format!("{name}: {text}"));
                    }
                    let _ = status.set(lines.join("\n"));
                });
            }
        };
        let prompt = move |step: &'static str, prompt: BiometricPrompt| {
            let tasks = tasks.clone();
            move || {
                let prompt = prompt.clone();
                let _ = tasks.spawn(async move {
                    let result = match services() {
                        Some(client) => client.authenticate(prompt).await,
                        None => Err(ServiceError::Unavailable),
                    };
                    report(
                        step,
                        match result {
                            Ok(kind) => format!("Ok({kind:?})"),
                            Err(e) => format!("Err({e:?})"),
                        },
                    );
                });
            }
        };
        let mut weak = BiometricPrompt::new("Taim weak biometric", "Cancel");
        weak.policy.strength = BiometricStrength::Weak;
        weak.subtitle = Some("Face or fingerprint".into());
        let mut credential = BiometricPrompt::new("Taim biometric or PIN", "unused");
        credential.negative = None;
        credential.policy.device_credential = true;
        column(views![
            text("Taim biometric probe").size(28),
            button("Check status").on_press(check),
            button("Strong biometric").on_press(prompt(
                "auth-strong",
                BiometricPrompt::new("Taim strong biometric", "Cancel"),
            )),
            button("Weak biometric").on_press(prompt("auth-weak", weak)),
            button("Biometric or PIN").on_press(prompt("auth-credential", credential)),
            text(move || last.get()).size(20),
            text(move || status.get()),
        ])
        .gap(24.0)
        .padding(32.0)
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Taim Android failed: {error}");
    }
}
