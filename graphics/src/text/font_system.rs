//! Process-wide font registry with lock-free generation observation.

use rustc_hash::FxHashSet;

use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Bundled Fira Sans regular font data.
#[cfg(feature = "fira-sans")]
pub const FIRA_SANS_REGULAR: &[u8] = include_bytes!("../../fonts/FiraSans-Regular.ttf").as_slice();

/// Placeholder export when the bundled font feature is disabled.
#[cfg(not(feature = "fira-sans"))]
pub const FIRA_SANS_REGULAR: &[u8] = &[];

/// Returns TAIM's shared font registry.
pub fn font_system() -> &'static RwLock<FontSystem> {
    static INSTANCE: OnceLock<RwLock<FontSystem>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(FontSystem::new()))
}

/// A font database and its observable content generation.
pub struct FontSystem {
    raw: cosmic_text::FontSystem,
    borrowed_fonts: FxHashSet<(usize, usize)>,
    version: Version,
}

impl FontSystem {
    fn new() -> Self {
        #[allow(unused_mut)]
        let mut raw = cosmic_text::FontSystem::new_with_fonts([
            cosmic_text::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../fonts/Taim-Icons.ttf").as_slice(),
            )),
            #[cfg(feature = "fira-sans")]
            cosmic_text::fontdb::Source::Binary(Arc::new(FIRA_SANS_REGULAR)),
        ]);

        #[cfg(feature = "fira-sans")]
        raw.db_mut().set_sans_serif_family("Fira Sans");

        #[cfg(target_os = "macos")]
        {
            #[cfg(not(feature = "fira-sans"))]
            raw.db_mut().set_sans_serif_family(".SF NS");
            raw.db_mut().set_serif_family("Times New Roman");
            raw.db_mut().set_monospace_family("Menlo");
        }

        #[cfg(target_os = "windows")]
        {
            #[cfg(not(feature = "fira-sans"))]
            raw.db_mut().set_sans_serif_family("Segoe UI");
            raw.db_mut().set_serif_family("Times New Roman");
            raw.db_mut().set_monospace_family("Consolas");
        }

        Self {
            raw,
            borrowed_fonts: FxHashSet::default(),
            version: Version::default(),
        }
    }

    /// Returns mutable access to the shaping backend.
    pub fn raw(&mut self) -> &mut cosmic_text::FontSystem {
        &mut self.raw
    }

    /// Adds a font without copying borrowed static byte slices.
    pub fn load_font(&mut self, bytes: Cow<'static, [u8]>) {
        let borrowed_key = match &bytes {
            Cow::Borrowed(data) => {
                let key = (data.as_ptr() as usize, data.len());
                if !self.borrowed_fonts.insert(key) {
                    return;
                }
                Some(key)
            }
            Cow::Owned(_) => None,
        };

        let loaded = self
            .raw
            .db_mut()
            .load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(bytes)));

        if loaded.is_empty() {
            if let Some(key) = borrowed_key {
                let _ = self.borrowed_fonts.remove(&key);
            }
            return;
        }

        self.version = Version(self.version.0.wrapping_add(1));
        GENERATION.store(self.version.0, Ordering::Release);
    }

    /// Iterates the first family name of each registered face.
    pub fn families(&self) -> impl Iterator<Item = &str> {
        self.raw
            .db()
            .faces()
            .filter_map(|face| face.families.first())
            .map(|(name, _)| name.as_str())
    }

    /// Returns the current font content generation.
    pub const fn version(&self) -> Version {
        self.version
    }
}

/// Monotonic font-registry generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version(u64);

pub(crate) fn font_version() -> Version {
    Version(GENERATION.load(Ordering::Acquire))
}
