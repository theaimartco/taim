//! Bounded clipboard offers.

use std::collections::BTreeMap;

/// Default maximum bytes accepted for a single clipboard representation.
pub const DEFAULT_MAX_OFFER_BYTES: usize = 16 * 1024 * 1024;

/// Disclosure class attached to transferred data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privacy {
    /// Unrestricted content.
    Public,
    /// Content withheld while the session is locked.
    Private,
    /// Content that must never leave its originating widget.
    Secret,
}

/// Clipboard validation failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("secure content may not enter the clipboard")]
    /// Secret content was rejected.
    Secure,
    #[error("clipboard offer exceeds the configured byte limit")]
    /// Byte limit was exceeded.
    TooLarge,
    #[error("clipboard MIME type is empty or malformed")]
    /// MIME syntax was rejected.
    InvalidMime,
}

/// One validated clipboard representation.
#[derive(Debug, Clone)]
pub struct Offer {
    mime: String,
    bytes: Vec<u8>,
    privacy: Privacy,
}

impl Offer {
    /// Validates and constructs an offer.
    pub fn new(
        mime: impl Into<String>,
        bytes: Vec<u8>,
        privacy: Privacy,
        max_bytes: usize,
    ) -> Result<Self, Error> {
        let mime = mime.into();
        if privacy == Privacy::Secret {
            return Err(Error::Secure);
        }
        if bytes.len() > max_bytes {
            return Err(Error::TooLarge);
        }
        if mime.is_empty()
            || !mime.contains('/')
            || mime.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(Error::InvalidMime);
        }
        Ok(Self {
            mime,
            bytes,
            privacy,
        })
    }

    /// Returns the representation MIME type.
    pub fn mime(&self) -> &str {
        &self.mime
    }

    /// Returns the immutable payload.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the disclosure class.
    pub const fn privacy(&self) -> Privacy {
        self.privacy
    }
}

/// Bounded set of clipboard representations.
#[derive(Debug)]
pub struct Clipboard {
    offers: BTreeMap<String, Offer>,
    max_bytes: usize,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_OFFER_BYTES)
    }
}

impl Clipboard {
    /// Creates a clipboard using the supplied per-offer byte limit.
    pub fn new(max_bytes: usize) -> Self {
        Self {
            offers: BTreeMap::new(),
            max_bytes,
        }
    }

    /// Adds or replaces one MIME representation.
    pub fn offer(
        &mut self,
        mime: impl Into<String>,
        bytes: Vec<u8>,
        privacy: Privacy,
    ) -> Result<(), Error> {
        let offer = Offer::new(mime, bytes, privacy, self.max_bytes)?;
        let _ = self.offers.insert(offer.mime.clone(), offer);
        Ok(())
    }

    /// Returns a representation by MIME type.
    pub fn get(&self, mime: &str) -> Option<&Offer> {
        self.offers.get(mime)
    }

    /// Cancels and removes every current offer.
    pub fn clear(&mut self) {
        self.offers.clear();
    }

    /// Returns total resident payload bytes.
    pub fn total_bytes(&self) -> usize {
        self.offers.values().map(|offer| offer.bytes.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_and_oversized_offers_fail_closed() {
        assert_eq!(
            Offer::new("text/plain", vec![1], Privacy::Secret, 8).unwrap_err(),
            Error::Secure
        );
        assert_eq!(
            Offer::new("text/plain", vec![1; 9], Privacy::Public, 8).unwrap_err(),
            Error::TooLarge
        );
    }
}
