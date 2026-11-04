//! Native fallback for platform color-font formats unavailable in Swash.
use cosmic_text::{CacheKey, FontSystem, SwashImage};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

/// One already-shaped glyph. No application text is passed to the platform.
pub struct ColorGlyph {
    /// Font database file, including its collection index.
    pub file: PathBuf,
    /// Face within the font collection.
    pub index: u32,
    /// Shaped glyph identifier.
    pub glyph: u16,
    /// Physical raster size.
    pub size: f32,
    /// Fractional pixel offset.
    pub x: f32,
    /// Fractional pixel offset.
    pub y: f32,
}

type Rasterizer = Arc<dyn Fn(ColorGlyph) -> Option<SwashImage> + Send + Sync>;
static RASTERIZER: OnceLock<RwLock<Option<Rasterizer>>> = OnceLock::new();

/// Installs the host's native color-glyph provider. Providers must hold a weak
/// host reference so the process-wide registry cannot retain a closed activity.
pub fn set_color_glyph_rasterizer(rasterizer: Rasterizer) {
    *RASTERIZER
        .get_or_init(RwLock::default)
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(rasterizer);
}

/// Resolves only missing system emoji glyphs. Ordinary outlines stay on Swash;
/// successful results are cached in the bounded GPU atlas.
pub fn rasterize_color_glyph(system: &mut FontSystem, key: CacheKey) -> Option<SwashImage> {
    let face = system.db().face(key.font_id)?;
    if !face
        .families
        .iter()
        .any(|(family, _)| family.contains("Emoji"))
    {
        return None;
    }
    let file = match &face.source {
        cosmic_text::fontdb::Source::File(path)
        | cosmic_text::fontdb::Source::SharedFile(path, _) => path.clone(),
        cosmic_text::fontdb::Source::Binary(_) => return None,
    };
    let request = ColorGlyph {
        file,
        index: face.index,
        glyph: key.glyph_id,
        size: f32::from_bits(key.font_size_bits),
        x: key.x_bin.as_float(),
        y: key.y_bin.as_float(),
    };
    if !request.size.is_finite() || request.size <= 0.0 || request.size > 512.0 {
        return None;
    }
    let rasterizer = RASTERIZER
        .get()?
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()?;
    rasterizer(request)
}
