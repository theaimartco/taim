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

        // fontdb deliberately excludes Android from system discovery. Load the
        // platform directories before rebuilding cosmic-text's derived font
        // and monospace indices, rather than adding faces to stale indices.
        #[cfg(target_os = "android")]
        #[allow(unused_mut)]
        let mut raw = {
            let (locale, mut db) = raw.into_locale_and_db();
            for directory in ["/system/fonts", "/product/fonts", "/system_ext/fonts"] {
                if std::path::Path::new(directory).is_dir() {
                    db.load_fonts_dir(directory);
                }
            }
            db.set_sans_serif_family("Roboto");
            db.set_serif_family("Noto Serif");
            db.set_monospace_family("Noto Sans Mono");
            log::info!("Android font discovery: {} faces", db.len());
            cosmic_text::FontSystem::new_with_locale_and_db_and_fallback(
                locale,
                db,
                AndroidFallback,
            )
        };

        #[cfg(feature = "fira-sans")]
        raw.db_mut().set_sans_serif_family("Fira Sans");

        #[cfg(target_os = "macos")]
        {
            #[cfg(not(feature = "fira-sans"))]
            raw.db_mut().set_sans_serif_family(".SF NS");
            raw.db_mut().set_serif_family("Times New Roman");
            raw.db_mut().set_monospace_family("Menlo");
        }

        // fontdb does not look for system fonts on Android, which would
        // leave every view without text. The platform's fonts (Roboto and
        // the Noto families that cover other scripts, Devanagari included)
        // live in these directories.
        #[cfg(target_os = "android")]
        {
            for directory in ["/system/fonts", "/product/fonts", "/system_ext/fonts"] {
                raw.db_mut().load_fonts_dir(directory);
            }
            #[cfg(not(feature = "fira-sans"))]
            raw.db_mut().set_sans_serif_family("Roboto");
            raw.db_mut().set_serif_family("Noto Serif");
            raw.db_mut().set_monospace_family("Droid Sans Mono");
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

// cosmic-text intentionally supplies an empty fallback policy on Android.
// Discovering the files alone does not put color emoji into its fallback list.
#[cfg(target_os = "android")]
struct AndroidFallback;

#[cfg(target_os = "android")]
impl cosmic_text::Fallback for AndroidFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &[
            "Noto Color Emoji",
            "Noto Sans Symbols 2",
            "Noto Sans Symbols",
            "Roboto",
            "Noto Sans",
        ]
    }
    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }
    fn script_fallback(&self, script: unicode_script::Script, locale: &str) -> &[&'static str] {
        use unicode_script::Script;
        match script {
            Script::Devanagari => &["Noto Sans Devanagari"],
            Script::Arabic => &["Noto Naskh Arabic", "Noto Sans Arabic"],
            Script::Bengali => &["Noto Sans Bengali"],
            Script::Gujarati => &["Noto Sans Gujarati"],
            Script::Gurmukhi => &["Noto Sans Gurmukhi"],
            Script::Tamil => &["Noto Sans Tamil"],
            Script::Telugu => &["Noto Sans Telugu"],
            Script::Kannada => &["Noto Sans Kannada"],
            Script::Malayalam => &["Noto Sans Malayalam"],
            Script::Thai => &["Noto Sans Thai"],
            Script::Hebrew => &["Noto Sans Hebrew"],
            Script::Hiragana | Script::Katakana => &["Noto Sans CJK JP"],
            Script::Hangul => &["Noto Sans CJK KR"],
            Script::Han if locale.starts_with("ja") => &["Noto Sans CJK JP"],
            Script::Han if locale.starts_with("ko") => &["Noto Sans CJK KR"],
            Script::Han if locale.starts_with("zh-TW") || locale.starts_with("zh-HK") => {
                &["Noto Sans CJK TC"]
            }
            Script::Han => &["Noto Sans CJK SC", "Noto Sans CJK JP"],
            _ => &[],
        }
    }
}
