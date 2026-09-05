//! Flat, GPU-uploadable scene encoding for Taim's tiled compute rasterizer.
//!
//! This crate owns the data contract between retained UI paint and the four
//! compute stages described by Pillar I. It deliberately has no dependency on
//! a graphics API: the same byte stream can be consumed by `wgpu`, inspected
//! by benchmarks, and binned by the deterministic CPU reference below.

#![forbid(unsafe_code)]

use bytemuck::{Pod, Zeroable};

/// Width and height, in pixels, of one compute-raster tile.
pub const TILE_SIZE: u32 = 16;

/// Sentinel stored in GPU records when an optional index is absent.
pub const NONE_INDEX: u32 = u32::MAX;

/// A stable index into [`Scene::paths`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct PathId(pub u32);

/// A stable index into [`Scene::clips`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct ClipId(pub u32);

/// An application-defined material-table index.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct MaterialId(pub u32);

/// One straight path segment in scene coordinates.
///
/// Curves are flattened before this boundary for the first implementation.
/// A later encoder revision may add typed curve records without changing draw
/// or clip ordering.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Segment {
    /// Segment start point.
    pub from: [f32; 2],
    /// Segment end point.
    pub to: [f32; 2],
}

/// A contiguous range in the scene's segment stream.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct Path {
    /// Index of the first segment.
    pub first_segment: u32,
    /// Number of segments in the path.
    pub segment_count: u32,
}

/// One entry in the append-only clip stack.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct Clip {
    /// Path defining the clip boundary.
    pub path: u32,
    /// Parent clip index, or [`NONE_INDEX`] for the root.
    pub parent: u32,
}

/// One painter-ordered draw command.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct DrawObject {
    /// Path to cover.
    pub path: u32,
    /// Active clip-stack entry, or [`NONE_INDEX`].
    pub clip: u32,
    /// Material-table index used by the fine stage.
    pub material: u32,
    /// Object opacity, clamped to `0..=1` by [`Encoder`].
    pub opacity: f32,
}

/// A normalized axis-aligned rectangle in scene coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Minimum x coordinate.
    pub min_x: f32,
    /// Minimum y coordinate.
    pub min_y: f32,
    /// Maximum x coordinate.
    pub max_x: f32,
    /// Maximum y coordinate.
    pub max_y: f32,
}

impl Rect {
    /// Creates a normalized rectangle from two corners.
    pub fn new(a: [f32; 2], b: [f32; 2]) -> Self {
        Self {
            min_x: a[0].min(b[0]),
            min_y: a[1].min(b[1]),
            max_x: a[0].max(b[0]),
            max_y: a[1].max(b[1]),
        }
    }

    fn intersect(self, other: Self) -> Option<Self> {
        let intersection = Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        };

        (intersection.max_x >= intersection.min_x && intersection.max_y >= intersection.min_y)
            .then_some(intersection)
    }
}

/// Fully encoded, append-only raster scene.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    segments: Vec<Segment>,
    paths: Vec<Path>,
    clips: Vec<Clip>,
    draws: Vec<DrawObject>,
}

impl Scene {
    /// Returns the flat segment stream.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Returns the path range table.
    pub fn paths(&self) -> &[Path] {
        &self.paths
    }

    /// Returns the append-only clip stack.
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    /// Returns painter-ordered draw objects.
    pub fn draws(&self) -> &[DrawObject] {
        &self.draws
    }

    /// Returns the segment stream as its exact GPU-upload bytes.
    pub fn segment_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.segments)
    }

    /// Returns the path table as its exact GPU-upload bytes.
    pub fn path_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.paths)
    }

    /// Returns the clip table as its exact GPU-upload bytes.
    pub fn clip_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.clips)
    }

    /// Returns draw objects as their exact GPU-upload bytes.
    pub fn draw_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.draws)
    }
}

/// Invalid input rejected by [`Encoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// A coordinate or opacity was NaN or infinite.
    NonFinite,
    /// A path contained fewer than two points.
    EmptyPath,
    /// A referenced path does not exist in this scene.
    UnknownPath,
    /// A referenced clip does not exist in this scene.
    UnknownClip,
    /// A stream exceeded the `u32` indexing limit used by the GPU ABI.
    Capacity,
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NonFinite => "raster input must be finite",
            Self::EmptyPath => "a path needs at least two points",
            Self::UnknownPath => "draw or clip references an unknown path",
            Self::UnknownClip => "clip stack references an unknown parent",
            Self::Capacity => "raster stream exceeds its u32 index space",
        };

        formatter.write_str(message)
    }
}

impl std::error::Error for EncodeError {}

/// Builder for a validated [`Scene`].
#[derive(Debug, Default)]
pub struct Encoder {
    scene: Scene,
}

impl Encoder {
    /// Creates an empty encoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a polyline or closed polygon and returns its stable path ID.
    pub fn push_path(&mut self, points: &[[f32; 2]], closed: bool) -> Result<PathId, EncodeError> {
        if points.len() < 2 {
            return Err(EncodeError::EmptyPath);
        }
        if points
            .iter()
            .flatten()
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err(EncodeError::NonFinite);
        }

        let extra = usize::from(closed);
        let segment_count = points
            .len()
            .checked_sub(1)
            .and_then(|count| count.checked_add(extra))
            .ok_or(EncodeError::Capacity)?;
        let first_segment =
            u32::try_from(self.scene.segments.len()).map_err(|_| EncodeError::Capacity)?;
        let segment_count = u32::try_from(segment_count).map_err(|_| EncodeError::Capacity)?;
        let _ = first_segment
            .checked_add(segment_count)
            .ok_or(EncodeError::Capacity)?;
        let id = PathId(u32::try_from(self.scene.paths.len()).map_err(|_| EncodeError::Capacity)?);

        self.scene
            .segments
            .extend(points.windows(2).map(|pair| Segment {
                from: pair[0],
                to: pair[1],
            }));
        if closed {
            self.scene.segments.push(Segment {
                from: points[points.len() - 1],
                to: points[0],
            });
        }
        self.scene.paths.push(Path {
            first_segment,
            segment_count,
        });

        Ok(id)
    }

    /// Appends a clip-stack entry and returns its stable ID.
    pub fn push_clip(
        &mut self,
        path: PathId,
        parent: Option<ClipId>,
    ) -> Result<ClipId, EncodeError> {
        if path.0 as usize >= self.scene.paths.len() {
            return Err(EncodeError::UnknownPath);
        }
        if parent.is_some_and(|parent| parent.0 as usize >= self.scene.clips.len()) {
            return Err(EncodeError::UnknownClip);
        }

        let id = ClipId(u32::try_from(self.scene.clips.len()).map_err(|_| EncodeError::Capacity)?);
        self.scene.clips.push(Clip {
            path: path.0,
            parent: parent.map_or(NONE_INDEX, |parent| parent.0),
        });
        Ok(id)
    }

    /// Appends a painter-ordered draw object.
    pub fn push_draw(
        &mut self,
        path: PathId,
        clip: Option<ClipId>,
        material: MaterialId,
        opacity: f32,
    ) -> Result<(), EncodeError> {
        if path.0 as usize >= self.scene.paths.len() {
            return Err(EncodeError::UnknownPath);
        }
        if clip.is_some_and(|clip| clip.0 as usize >= self.scene.clips.len()) {
            return Err(EncodeError::UnknownClip);
        }
        if !opacity.is_finite() {
            return Err(EncodeError::NonFinite);
        }

        self.scene.draws.push(DrawObject {
            path: path.0,
            clip: clip.map_or(NONE_INDEX, |clip| clip.0),
            material: material.0,
            opacity: opacity.clamp(0.0, 1.0),
        });
        Ok(())
    }

    /// Finishes encoding without reordering any stream.
    pub fn finish(self) -> Scene {
        self.scene
    }
}

/// Dimensions of a row-major tile grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileGrid {
    columns: u32,
    rows: u32,
}

impl TileGrid {
    /// Creates the grid covering a pixel viewport.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            columns: width.div_ceil(TILE_SIZE),
            rows: height.div_ceil(TILE_SIZE),
        }
    }

    /// Returns the number of tile columns.
    pub fn columns(self) -> u32 {
        self.columns
    }

    /// Returns the number of tile rows.
    pub fn rows(self) -> u32 {
        self.rows
    }

    /// Returns the total tile count.
    pub fn len(self) -> usize {
        self.columns as usize * self.rows as usize
    }

    /// Returns whether the grid contains no tiles.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// Compressed row storage of segment indices for every tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileBins {
    grid: TileGrid,
    offsets: Vec<u32>,
    segment_indices: Vec<u32>,
}

impl TileBins {
    /// Returns the grid these bins cover.
    pub fn grid(&self) -> TileGrid {
        self.grid
    }

    /// Returns the segment indices touching `tile` in source order.
    pub fn segments_for(&self, tile: usize) -> &[u32] {
        let Some(next) = tile.checked_add(1) else {
            return &[];
        };
        let Some(range) = self.offsets.get(tile..=next) else {
            return &[];
        };
        &self.segment_indices[range[0] as usize..range[1] as usize]
    }

    /// Returns the CSR offset table for GPU upload.
    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    /// Returns the flattened segment-index stream for GPU upload.
    pub fn segment_indices(&self) -> &[u32] {
        &self.segment_indices
    }
}

/// Deterministic CPU reference for the `path_coarse` tile-binning stage.
pub fn bin_segments(scene: &Scene, grid: TileGrid, damage: Option<Rect>) -> TileBins {
    let mut tiles = vec![Vec::<u32>::new(); grid.len()];
    let viewport = Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: (grid.columns * TILE_SIZE) as f32,
        max_y: (grid.rows * TILE_SIZE) as f32,
    };

    for (index, segment) in scene.segments.iter().enumerate() {
        let Some(mut bounds) = Rect::new(segment.from, segment.to).intersect(viewport) else {
            continue;
        };
        if let Some(damage) = damage {
            let Some(clipped) = bounds.intersect(damage) else {
                continue;
            };
            bounds = clipped;
        }
        if grid.is_empty() {
            continue;
        }

        let min_x =
            ((bounds.min_x / TILE_SIZE as f32).floor().max(0.0) as u32).min(grid.columns - 1);
        let min_y = ((bounds.min_y / TILE_SIZE as f32).floor().max(0.0) as u32).min(grid.rows - 1);
        let max_x =
            ((bounds.max_x / TILE_SIZE as f32).floor().max(0.0) as u32).min(grid.columns - 1);
        let max_y = ((bounds.max_y / TILE_SIZE as f32).floor().max(0.0) as u32).min(grid.rows - 1);
        let Ok(index) = u32::try_from(index) else {
            break;
        };

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                tiles[(y * grid.columns + x) as usize].push(index);
            }
        }
    }

    let mut offsets = Vec::with_capacity(tiles.len() + 1);
    let mut segment_indices = Vec::new();
    offsets.push(0);
    for tile in tiles {
        segment_indices.extend(tile);
        offsets.push(u32::try_from(segment_indices.len()).unwrap_or(u32::MAX));
    }

    TileBins {
        grid,
        offsets,
        segment_indices,
    }
}

/// Failure to evaluate reference path coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageError {
    /// The requested path is not present in the scene.
    UnknownPath,
    /// The segment range stored for the path is invalid.
    InvalidRange,
    /// Reference fill coverage requires a closed path.
    OpenPath,
    /// Consecutive segments do not form one continuous contour.
    DiscontinuousPath,
}

impl std::fmt::Display for CoverageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::UnknownPath => "coverage references an unknown path",
            Self::InvalidRange => "path segment range is invalid",
            Self::OpenPath => "fill coverage requires a closed path",
            Self::DiscontinuousPath => "path segments are not continuous",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CoverageError {}

/// Computes exact area coverage of a closed polygon in one unit pixel.
///
/// The polygon is clipped against the pixel square and evaluated with the
/// shoelace formula. This is deliberately a slow CPU reference for validating
/// the compute fine stage, not the production rasterization path. Curves must
/// first be flattened to the accuracy required by their proof workload.
pub fn reference_coverage(
    scene: &Scene,
    path: PathId,
    pixel_x: u32,
    pixel_y: u32,
) -> Result<f32, CoverageError> {
    let path = scene
        .paths
        .get(path.0 as usize)
        .ok_or(CoverageError::UnknownPath)?;
    let start = path.first_segment as usize;
    let end = start
        .checked_add(path.segment_count as usize)
        .ok_or(CoverageError::InvalidRange)?;
    let segments = scene
        .segments
        .get(start..end)
        .ok_or(CoverageError::InvalidRange)?;
    let Some(first) = segments.first() else {
        return Err(CoverageError::OpenPath);
    };

    let mut polygon = Vec::with_capacity(segments.len() + 1);
    polygon.push(first.from);
    let mut previous = first.from;
    for segment in segments {
        if segment.from != previous {
            return Err(CoverageError::DiscontinuousPath);
        }
        polygon.push(segment.to);
        previous = segment.to;
    }
    if previous != first.from {
        return Err(CoverageError::OpenPath);
    }

    let min_x = pixel_x as f32;
    let min_y = pixel_y as f32;
    let max_x = min_x + 1.0;
    let max_y = min_y + 1.0;
    polygon = clip_polygon(&polygon, Edge::Left(min_x));
    polygon = clip_polygon(&polygon, Edge::Right(max_x));
    polygon = clip_polygon(&polygon, Edge::Top(min_y));
    polygon = clip_polygon(&polygon, Edge::Bottom(max_y));

    if polygon.len() < 3 {
        return Ok(0.0);
    }

    let twice_area = polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f32>();

    Ok((twice_area.abs() * 0.5).clamp(0.0, 1.0))
}

/// Rasterizes one reference path into row-major 8-bit coverage.
pub fn reference_mask(
    scene: &Scene,
    path: PathId,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, CoverageError> {
    let mut mask = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let coverage = reference_coverage(scene, path, x, y)?;
            mask.push((coverage * 255.0).round() as u8);
        }
    }
    Ok(mask)
}

#[derive(Debug, Clone, Copy)]
enum Edge {
    Left(f32),
    Right(f32),
    Top(f32),
    Bottom(f32),
}

impl Edge {
    fn contains(self, point: [f32; 2]) -> bool {
        match self {
            Self::Left(x) => point[0] >= x,
            Self::Right(x) => point[0] <= x,
            Self::Top(y) => point[1] >= y,
            Self::Bottom(y) => point[1] <= y,
        }
    }

    fn intersection(self, from: [f32; 2], to: [f32; 2]) -> [f32; 2] {
        match self {
            Self::Left(x) | Self::Right(x) => {
                let delta = to[0] - from[0];
                let t = if delta == 0.0 {
                    0.0
                } else {
                    (x - from[0]) / delta
                };
                [x, from[1] + t * (to[1] - from[1])]
            }
            Self::Top(y) | Self::Bottom(y) => {
                let delta = to[1] - from[1];
                let t = if delta == 0.0 {
                    0.0
                } else {
                    (y - from[1]) / delta
                };
                [from[0] + t * (to[0] - from[0]), y]
            }
        }
    }
}

fn clip_polygon(polygon: &[[f32; 2]], edge: Edge) -> Vec<[f32; 2]> {
    let Some(mut previous) = polygon.last().copied() else {
        return Vec::new();
    };
    let mut previous_inside = edge.contains(previous);
    let mut output = Vec::with_capacity(polygon.len() + 1);

    for &current in polygon {
        let current_inside = edge.contains(current);
        match (previous_inside, current_inside) {
            (true, true) => output.push(current),
            (true, false) => output.push(edge.intersection(previous, current)),
            (false, true) => {
                output.push(edge.intersection(previous, current));
                output.push(current);
            }
            (false, false) => {}
        }
        previous = current;
        previous_inside = current_inside;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(encoder: &mut Encoder) -> PathId {
        encoder
            .push_path(&[[0.0, 0.0], [32.0, 0.0], [32.0, 32.0], [0.0, 32.0]], true)
            .expect("square must encode")
    }

    #[test]
    fn scene_streams_preserve_append_and_painter_order() {
        let mut encoder = Encoder::new();
        let path = square(&mut encoder);
        let root = encoder.push_clip(path, None).expect("root clip");
        let child = encoder.push_clip(path, Some(root)).expect("child clip");
        encoder
            .push_draw(path, Some(child), MaterialId(7), 2.0)
            .expect("draw");
        let scene = encoder.finish();

        assert_eq!(scene.paths()[0].segment_count, 4);
        assert_eq!(scene.clips()[1].parent, root.0);
        assert_eq!(scene.draws()[0].material, 7);
        assert_eq!(scene.draws()[0].opacity, 1.0);
        assert_eq!(
            scene.segment_bytes().len(),
            4 * std::mem::size_of::<Segment>()
        );
    }

    #[test]
    fn invalid_or_non_finite_input_never_enters_the_stream() {
        let mut encoder = Encoder::new();
        assert_eq!(
            encoder.push_path(&[[0.0, 0.0]], false),
            Err(EncodeError::EmptyPath)
        );
        assert_eq!(
            encoder.push_path(&[[0.0, 0.0], [f32::NAN, 1.0]], false),
            Err(EncodeError::NonFinite)
        );
        assert_eq!(
            encoder.push_draw(PathId(4), None, MaterialId(0), 1.0),
            Err(EncodeError::UnknownPath)
        );
        assert!(encoder.finish().segments().is_empty());
    }

    #[test]
    fn binning_is_deterministic_and_source_ordered() {
        let mut encoder = Encoder::new();
        let _ = encoder
            .push_path(&[[1.0, 1.0], [31.0, 31.0], [2.0, 2.0]], false)
            .expect("path");
        let scene = encoder.finish();
        let grid = TileGrid::new(32, 32);
        let first = bin_segments(&scene, grid, None);
        let second = bin_segments(&scene, grid, None);

        assert_eq!(first, second);
        assert_eq!(first.segments_for(0), &[0, 1]);
        assert_eq!(first.segments_for(3), &[0, 1]);
    }

    #[test]
    fn damage_culls_untouched_tiles_and_segments() {
        let mut encoder = Encoder::new();
        let _ = encoder
            .push_path(&[[1.0, 1.0], [7.0, 7.0], [20.0, 20.0], [30.0, 30.0]], false)
            .expect("path");
        let bins = bin_segments(
            &encoder.finish(),
            TileGrid::new(32, 32),
            Some(Rect::new([0.0, 0.0], [15.0, 15.0])),
        );

        assert_eq!(bins.segments_for(0), &[0, 1]);
        assert!(bins.segments_for(3).is_empty());
    }

    #[test]
    fn reference_coverage_is_exact_for_axis_aligned_edges() {
        let mut encoder = Encoder::new();
        let path = encoder
            .push_path(&[[0.0, 0.0], [1.5, 0.0], [1.5, 1.0], [0.0, 1.0]], true)
            .expect("rectangle");
        let scene = encoder.finish();

        assert_eq!(reference_coverage(&scene, path, 0, 0), Ok(1.0));
        assert_eq!(reference_coverage(&scene, path, 1, 0), Ok(0.5));
        assert_eq!(reference_coverage(&scene, path, 2, 0), Ok(0.0));
        assert_eq!(reference_mask(&scene, path, 3, 1), Ok(vec![255, 128, 0]));
    }

    #[test]
    fn reference_coverage_rejects_open_paths() {
        let mut encoder = Encoder::new();
        let path = encoder
            .push_path(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false)
            .expect("open path");

        assert_eq!(
            reference_coverage(&encoder.finish(), path, 0, 0),
            Err(CoverageError::OpenPath)
        );
    }
}
