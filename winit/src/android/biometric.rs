//! System biometric/device-credential prompt (Android 10, API 29, and newer).
//!
//! A successful prompt proves that the local user passed Android's system UI. It
//! is not a server credential: anything that unlocks secrets or authorizes a
//! remote action must bind a Keystore key to user authentication instead.
use serde_json::{Value, json};

/// Android biometric class accepted by the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BiometricStrength {
    /// Class 3: suitable for Keystore-bound keys. The default.
    #[default]
    Strong,
    /// Class 2 or better; includes many face-unlock implementations.
    Weak,
}
impl BiometricStrength {
    fn wire(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Weak => "weak",
        }
    }
}

/// Which authenticators a status query or prompt accepts.
///
/// On API 29 the class cannot be enforced or queried separately; Android's own
/// biometric policy applies. API 30+ enforces `strength` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BiometricPolicy {
    /// Minimum biometric class.
    pub strength: BiometricStrength,
    /// Also allow the device PIN/pattern/password as an alternative.
    pub device_credential: bool,
}

/// Availability for a policy, without showing UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricAvailability {
    /// A prompt with this policy can be shown.
    Available,
    /// Hardware exists but nothing satisfying the policy is enrolled.
    NotEnrolled,
    /// No hardware for this policy.
    NoHardware,
    /// Hardware is temporarily unavailable.
    HardwareUnavailable,
    /// A security patch is required before the sensor can be used.
    SecurityUpdateRequired,
    /// Android 9 or older: the system prompt API is not supported.
    UnsupportedApi,
    /// Android reported a state this adapter does not recognise.
    Unsupported,
}

/// Status plus hardware hints. Hints describe features, not enrolment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiometricStatus {
    /// Whether a prompt with the queried policy can succeed now.
    pub availability: BiometricAvailability,
    /// A screen lock (PIN/pattern/password) is set.
    pub device_secure: bool,
    /// Device declares a fingerprint sensor.
    pub fingerprint: bool,
    /// Device declares face authentication.
    pub face: bool,
    /// Device declares iris authentication.
    pub iris: bool,
}
impl BiometricStatus {
    /// Decode a Request::BiometricStatus result, rejecting malformed native data.
    pub fn from_value(value: &Value) -> Option<Self> {
        let availability = match value.get("status")?.as_str()? {
            "available" => BiometricAvailability::Available,
            "notEnrolled" => BiometricAvailability::NotEnrolled,
            "noHardware" => BiometricAvailability::NoHardware,
            "hardwareUnavailable" => BiometricAvailability::HardwareUnavailable,
            "securityUpdateRequired" => BiometricAvailability::SecurityUpdateRequired,
            "unsupportedApi" => BiometricAvailability::UnsupportedApi,
            "unsupported" => BiometricAvailability::Unsupported,
            _ => return None,
        };
        let flag = |key: &str| value.get(key)?.as_bool();
        Some(Self {
            availability,
            device_secure: flag("deviceSecure")?,
            fingerprint: flag("fingerprint")?,
            face: flag("face")?,
            iris: flag("iris")?,
        })
    }
}

/// Text and policy for the system prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiometricPrompt {
    /// Required, 1–128 UTF-16 units.
    pub title: String,
    /// Optional, 1–256 units.
    pub subtitle: Option<String>,
    /// Optional, 1–512 units.
    pub description: Option<String>,
    /// Negative button label, 1–64 units. Required unless `policy.device_credential`
    /// is set, and forbidden with it: Android then shows its own credential button.
    pub negative: Option<String>,
    /// Accepted authenticators.
    pub policy: BiometricPolicy,
    /// For passive (face) biometrics, require an explicit confirm tap. Default true.
    pub confirmation_required: bool,
}
impl BiometricPrompt {
    /// Biometric-only prompt with a "Cancel"-style negative button.
    pub fn new(title: impl Into<String>, negative: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            description: None,
            negative: Some(negative.into()),
            policy: BiometricPolicy::default(),
            confirmation_required: true,
        }
    }
    pub(super) fn wire(&self) -> Option<Value> {
        let units = |text: &str| text.encode_utf16().count();
        let bounded = |text: &Option<String>, max| {
            text.as_deref()
                .is_none_or(|text| (1..=max).contains(&units(text)))
        };
        let negative_ok = if self.policy.device_credential {
            self.negative.is_none()
        } else {
            self.negative.is_some()
        };
        ((1..=128).contains(&units(&self.title))
            && bounded(&self.subtitle, 256)
            && bounded(&self.description, 512)
            && bounded(&self.negative, 64)
            && negative_ok)
            .then(|| {
                json!({
                    "op": "biometricAuthenticate",
                    "title": self.title,
                    "subtitle": self.subtitle,
                    "description": self.description,
                    "negative": self.negative,
                    "strength": self.policy.strength.wire(),
                    "deviceCredential": self.policy.device_credential,
                    "confirmationRequired": self.confirmation_required,
                })
            })
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(super) fn status_wire(policy: BiometricPolicy) -> Value {
    json!({"op":"biometricStatus","strength":policy.strength.wire(),"deviceCredential":policy.device_credential})
}

/// How the user satisfied a successful prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricAuthenticator {
    /// A biometric matched.
    Biometric,
    /// The device PIN/pattern/password was entered.
    DeviceCredential,
    /// API 29 does not report the method.
    Unknown,
}
impl BiometricAuthenticator {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        Some(match value.get("authenticator")?.as_str()? {
            "biometric" => Self::Biometric,
            "deviceCredential" => Self::DeviceCredential,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

/// Purpose of a biometric-bound Keystore key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricKeyKind {
    /// EC P-256 / SHA-256 signing key: register the public key with your server, then sign
    /// server challenges for login or transaction approval.
    Sign,
    /// AES-256-GCM key for local secrets (tokens, vault keys).
    Encrypt,
}

/// Options for a new biometric-bound key (Android 11, API 30, and newer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiometricKey {
    /// 1–64 characters `[A-Za-z0-9._-]`; creating an existing alias replaces the key.
    pub alias: String,
    /// Key purpose.
    pub kind: BiometricKeyKind,
    /// Also allow the device PIN/pattern/password to unlock it.
    pub device_credential: bool,
    /// Invalidate the key when a new biometric is enrolled (recommended; default true).
    pub invalidate_on_enrollment: bool,
}

/// AES-GCM output of `Services::biometric_encrypt`; store both parts, they are not secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedData {
    /// 12-byte nonce.
    pub iv: Vec<u8>,
    /// Ciphertext with the 16-byte authentication tag.
    pub ciphertext: Vec<u8>,
}

pub(super) fn valid_alias(alias: &str) -> bool {
    super::plugin_support::identifier(alias)
}

impl BiometricPrompt {
    /// Prompt wire data for a key operation. Keys require `BiometricStrength::Strong`, and the
    /// prompt's device-credential choice must match the key's.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(super) fn crypto_wire(&self, op: &str, alias: &str, fields: Value) -> Option<Value> {
        if !valid_alias(alias) || self.policy.strength != BiometricStrength::Strong {
            return None;
        }
        let mut wire = self.wire()?;
        wire["op"] = json!(op);
        wire["alias"] = json!(alias);
        for (key, value) in fields.as_object()? {
            wire[key] = value.clone();
        }
        Some(wire)
    }
}

#[cfg(target_os = "android")]
mod client {
    use super::super::plugin_support::{decode, encode};
    use super::super::{Request, ServiceError, Services};
    use super::*;

    impl Services {
        /// Create a biometric-bound key. Returns the DER (X.509 SubjectPublicKeyInfo) public key
        /// for `Sign`, None for `Encrypt`. `NotEnrolled` if no strong biometric is enrolled.
        pub async fn biometric_key_create(
            &self,
            key: &BiometricKey,
        ) -> Result<Option<Vec<u8>>, ServiceError> {
            if !valid_alias(&key.alias) {
                return Err(ServiceError::Invalid);
            }
            let kind = match key.kind {
                BiometricKeyKind::Sign => "sign",
                BiometricKeyKind::Encrypt => "encrypt",
            };
            let value = self
                .request(Request::BiometricKey(json!({"op":"biometricKeyCreate","alias":key.alias,"kind":kind,
                    "deviceCredential":key.device_credential,"invalidateOnEnrollment":key.invalidate_on_enrollment})))
                .await?;
            match key.kind {
                BiometricKeyKind::Sign => decode(&value).map(Some).ok_or(ServiceError::Failed),
                BiometricKeyKind::Encrypt => Ok(None),
            }
        }
        /// Delete a key, idempotently.
        pub async fn biometric_key_delete(&self, alias: &str) -> Result<(), ServiceError> {
            if !valid_alias(alias) {
                return Err(ServiceError::Invalid);
            }
            self.request(Request::BiometricKey(
                json!({"op":"biometricKeyDelete","alias":alias}),
            ))
            .await
            .map(drop)
        }
        /// DER public key of an existing signing key (no prompt), or None.
        pub async fn biometric_public_key(
            &self,
            alias: &str,
        ) -> Result<Option<Vec<u8>>, ServiceError> {
            if !valid_alias(alias) {
                return Err(ServiceError::Invalid);
            }
            let value = self
                .request(Request::BiometricKey(
                    json!({"op":"biometricPublicKey","alias":alias}),
                ))
                .await?;
            if value.is_null() {
                Ok(None)
            } else {
                decode(&value).map(Some).ok_or(ServiceError::Failed)
            }
        }
        /// Prompt, then sign `challenge` (1–4096 bytes) with SHA256withECDSA (DER signature).
        /// `KeyInvalidated` after a new enrollment: recreate the key and re-register it.
        pub async fn biometric_sign(
            &self,
            alias: &str,
            challenge: &[u8],
            prompt: &BiometricPrompt,
        ) -> Result<Vec<u8>, ServiceError> {
            if challenge.is_empty() || challenge.len() > 4096 {
                return Err(ServiceError::Invalid);
            }
            let wire = prompt
                .crypto_wire("biometricSign", alias, json!({"data":encode(challenge)}))
                .ok_or(ServiceError::Invalid)?;
            decode(&self.request(Request::BiometricKey(wire)).await?["signature"])
                .ok_or(ServiceError::Failed)
        }
        /// Prompt, then encrypt 1–65,536 bytes.
        pub async fn biometric_encrypt(
            &self,
            alias: &str,
            data: &[u8],
            prompt: &BiometricPrompt,
        ) -> Result<SealedData, ServiceError> {
            if data.is_empty() || data.len() > 65_536 {
                return Err(ServiceError::Invalid);
            }
            let wire = prompt
                .crypto_wire("biometricEncrypt", alias, json!({"data":encode(data)}))
                .ok_or(ServiceError::Invalid)?;
            let value = self.request(Request::BiometricKey(wire)).await?;
            Ok(SealedData {
                iv: decode(&value["iv"]).ok_or(ServiceError::Failed)?,
                ciphertext: decode(&value["ciphertext"]).ok_or(ServiceError::Failed)?,
            })
        }
        /// Prompt, then decrypt. A tampered ciphertext fails with `Failed`.
        pub async fn biometric_decrypt(
            &self,
            alias: &str,
            sealed: &SealedData,
            prompt: &BiometricPrompt,
        ) -> Result<Vec<u8>, ServiceError> {
            if sealed.iv.is_empty()
                || sealed.iv.len() > 32
                || sealed.ciphertext.is_empty()
                || sealed.ciphertext.len() > 65_552
            {
                return Err(ServiceError::Invalid);
            }
            let wire = prompt
                .crypto_wire(
                    "biometricDecrypt",
                    alias,
                    json!({"iv":encode(&sealed.iv),"ciphertext":encode(&sealed.ciphertext)}),
                )
                .ok_or(ServiceError::Invalid)?;
            decode(&self.request(Request::BiometricKey(wire)).await?["data"])
                .ok_or(ServiceError::Failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_text_and_negative_button_rules_are_enforced() {
        let prompt = BiometricPrompt::new("Unlock", "Cancel");
        let wire = prompt.wire().unwrap();
        assert_eq!(wire["strength"], "strong");
        assert_eq!(wire["deviceCredential"], false);

        let mut credential = prompt.clone();
        credential.policy.device_credential = true;
        assert!(
            credential.wire().is_none(),
            "negative forbidden with credential"
        );
        credential.negative = None;
        assert!(credential.wire().is_some());

        let mut missing = prompt.clone();
        missing.negative = None;
        assert!(
            missing.wire().is_none(),
            "negative required without credential"
        );

        for title in [String::new(), "x".repeat(129)] {
            let mut bad = prompt.clone();
            bad.title = title;
            assert!(bad.wire().is_none());
        }
        let mut empty_subtitle = prompt.clone();
        empty_subtitle.subtitle = Some(String::new());
        assert!(empty_subtitle.wire().is_none());
        let mut long_description = prompt;
        long_description.description = Some("x".repeat(513));
        assert!(long_description.wire().is_none());
    }

    #[test]
    fn crypto_prompts_require_strong_class_and_valid_alias() {
        let prompt = BiometricPrompt::new("Approve", "Cancel");
        let wire = prompt
            .crypto_wire("biometricSign", "login", json!({"data":"AQ=="}))
            .unwrap();
        assert_eq!(wire["op"], "biometricSign");
        assert_eq!(wire["alias"], "login");
        assert_eq!(wire["data"], "AQ==");
        assert!(
            prompt
                .crypto_wire("biometricSign", "bad alias", json!({}))
                .is_none()
        );
        let mut weak = prompt;
        weak.policy.strength = BiometricStrength::Weak;
        assert!(
            weak.crypto_wire("biometricSign", "login", json!({}))
                .is_none()
        );
    }

    #[test]
    fn status_query_carries_policy() {
        let wire = status_wire(BiometricPolicy {
            strength: BiometricStrength::Weak,
            device_credential: true,
        });
        assert_eq!(
            wire,
            json!({"op":"biometricStatus","strength":"weak","deviceCredential":true})
        );
    }

    #[test]
    fn status_decoding_fails_closed() {
        let ok = json!({"status":"available","deviceSecure":true,"fingerprint":true,"face":false,"iris":false});
        let status = BiometricStatus::from_value(&ok).unwrap();
        assert_eq!(status.availability, BiometricAvailability::Available);
        assert!(status.fingerprint && status.device_secure);
        assert!(BiometricStatus::from_value(&json!({"status":"maybe","deviceSecure":true,"fingerprint":true,"face":false,"iris":false})).is_none());
        assert!(BiometricStatus::from_value(&json!({"status":"available"})).is_none());
    }

    #[test]
    fn authenticator_decoding_fails_closed() {
        assert_eq!(
            BiometricAuthenticator::from_value(&json!({"authenticator":"biometric"})),
            Some(BiometricAuthenticator::Biometric)
        );
        assert!(BiometricAuthenticator::from_value(&json!({"authenticator":"face"})).is_none());
        assert!(BiometricAuthenticator::from_value(&json!({})).is_none());
    }
}
