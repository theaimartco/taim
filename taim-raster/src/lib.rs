//! Flat, GPU-uploadable scene encoding for Taim's tiled compute rasterizer.
//!
//! This crate owns the data contract between retained UI paint and the four
//! compute stages described by Pillar I. It deliberately has no dependency on
//! a graphics API: the same byte stream can be consumed by `wgpu`, inspected
//! by benchmarks, and binned by the deterministic CPU reference below.

#![forbid(unsafe_code)]

pub mod allocator;
pub use allocator::{Allocation, FreeListAllocator};

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

/// Kinematic velocity attached to a scene draw object for predictive damage and frame re-timing.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct ObjectVelocity {
    /// Linear velocity in scene pixels per second (dx/dt, dy/dt).
    pub linear: [f32; 2],
    /// Angular velocity in radians per second (dtheta/dt).
    pub angular: f32,
    /// Rate of scale change (ds/dt).
    pub scale_rate: f32,
}

impl Default for ObjectVelocity {
    fn default() -> Self {
        Self {
            linear: [0.0, 0.0],
            angular: 0.0,
            scale_rate: 0.0,
        }
    }
}

/// A granular delta patch to a GPU-resident scene buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum SceneEdit {
    /// Replace or update a contiguous slice of segments.
    UpdateSegments {
        /// Starting segment index in the scene.
        start_index: u32,
        /// New segments to overwrite with.
        segments: Vec<Segment>,
    },
    /// Update a specific draw object's properties in place.
    UpdateDraw {
        /// Target draw object index.
        draw_index: u32,
        /// New draw object data.
        draw: DrawObject,
    },
    /// Update an object's velocity in place.
    UpdateVelocity {
        /// Target draw object index.
        draw_index: u32,
        /// New velocity data.
        velocity: ObjectVelocity,
    },
    /// Append new draw objects to the end of the scene.
    AppendDraws(Vec<DrawObject>),
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
    velocities: Vec<ObjectVelocity>,
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

    /// Returns the per-object velocity stream.
    pub fn velocities(&self) -> &[ObjectVelocity] {
        &self.velocities
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

    /// Returns velocities as their exact GPU-upload bytes.
    pub fn velocity_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.velocities)
    }

    /// Applies a granular delta patch to the scene without rebuilding.
    pub fn apply_edit(&mut self, edit: &SceneEdit) {
        match edit {
            SceneEdit::UpdateSegments {
                start_index,
                segments,
            } => {
                let start = *start_index as usize;
                let end = start + segments.len();
                if end <= self.segments.len() {
                    self.segments[start..end].copy_from_slice(segments);
                }
            }
            SceneEdit::UpdateDraw { draw_index, draw } => {
                let idx = *draw_index as usize;
                if idx < self.draws.len() {
                    self.draws[idx] = *draw;
                }
            }
            SceneEdit::UpdateVelocity {
                draw_index,
                velocity,
            } => {
                let idx = *draw_index as usize;
                if idx < self.velocities.len() {
                    self.velocities[idx] = *velocity;
                }
            }
            SceneEdit::AppendDraws(new_draws) => {
                self.draws.extend_from_slice(new_draws);
                self.velocities
                    .resize(self.draws.len(), ObjectVelocity::default());
            }
        }
    }

    /// Calculates the exact byte count required to upload a delta patch to the GPU.
    pub fn delta_upload_bytes(&self, edit: &SceneEdit) -> usize {
        match edit {
            SceneEdit::UpdateSegments { segments, .. } => {
                segments.len() * std::mem::size_of::<Segment>()
            }
            SceneEdit::UpdateDraw { .. } => std::mem::size_of::<DrawObject>(),
            SceneEdit::UpdateVelocity { .. } => std::mem::size_of::<ObjectVelocity>(),
            SceneEdit::AppendDraws(draws) => draws.len() * std::mem::size_of::<DrawObject>(),
        }
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

    /// Appends an axis-aligned rectangle path directly with zero dynamic allocation.
    pub fn push_rect(&mut self, rect: Rect) -> Result<PathId, EncodeError> {
        let min_x = rect.min_x;
        let min_y = rect.min_y;
        let max_x = rect.max_x;
        let max_y = rect.max_y;

        if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
            return Err(EncodeError::NonFinite);
        }

        let first_segment =
            u32::try_from(self.scene.segments.len()).map_err(|_| EncodeError::Capacity)?;
        let id = PathId(u32::try_from(self.scene.paths.len()).map_err(|_| EncodeError::Capacity)?);

        self.scene.segments.reserve(4);
        self.scene.segments.push(Segment {
            from: [min_x, min_y],
            to: [max_x, min_y],
        });
        self.scene.segments.push(Segment {
            from: [max_x, min_y],
            to: [max_x, max_y],
        });
        self.scene.segments.push(Segment {
            from: [max_x, max_y],
            to: [min_x, max_y],
        });
        self.scene.segments.push(Segment {
            from: [min_x, max_y],
            to: [min_x, min_y],
        });

        self.scene.paths.push(Path {
            first_segment,
            segment_count: 4,
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
        self.scene.velocities.push(ObjectVelocity::default());
        Ok(())
    }

    /// Appends a painter-ordered draw object with kinematic velocity.
    pub fn push_draw_with_velocity(
        &mut self,
        path: PathId,
        clip: Option<ClipId>,
        material: MaterialId,
        opacity: f32,
        velocity: ObjectVelocity,
    ) -> Result<(), EncodeError> {
        self.push_draw(path, clip, material, opacity)?;
        if let Some(last) = self.scene.velocities.last_mut() {
            *last = velocity;
        }
        Ok(())
    }

    /// Flattens and appends a quadratic Bézier curve.
    pub fn push_quadratic_bezier(
        &mut self,
        p0: [f32; 2],
        p1: [f32; 2],
        p2: [f32; 2],
        policy: Option<CurveFlatteningPolicy>,
    ) -> Result<PathId, EncodeError> {
        let policy = policy.unwrap_or_default();
        let mut points = vec![p0];
        flatten_quadratic(p0, p1, p2, policy, 0, &mut points);
        self.push_path(&points, false)
    }

    /// Flattens and appends a cubic Bézier curve.
    pub fn push_cubic_bezier(
        &mut self,
        p0: [f32; 2],
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        policy: Option<CurveFlatteningPolicy>,
    ) -> Result<PathId, EncodeError> {
        let policy = policy.unwrap_or_default();
        let mut points = vec![p0];
        flatten_cubic(p0, p1, p2, p3, policy, 0, &mut points);
        self.push_path(&points, false)
    }

    /// Appends a closed rounded rectangle path with per-corner radii [top-left, top-right, bottom-right, bottom-left].
    pub fn push_rounded_rect(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        policy: Option<CurveFlatteningPolicy>,
    ) -> Result<PathId, EncodeError> {
        let policy = policy.unwrap_or_default();
        let w = (rect.max_x - rect.min_x).abs();
        let h = (rect.max_y - rect.min_y).abs();
        let max_r = (w.min(h) * 0.5).max(0.0);

        let tl = radii[0].clamp(0.0, max_r);
        let tr = radii[1].clamp(0.0, max_r);
        let br = radii[2].clamp(0.0, max_r);
        let bl = radii[3].clamp(0.0, max_r);

        const KAPPA: f32 = 0.552_284_8; // 4 * (sqrt(2) - 1) / 3

        let mut points = Vec::with_capacity(32);
        // Start after top-left corner
        points.push([rect.min_x + tl, rect.min_y]);
        // Top edge
        points.push([rect.max_x - tr, rect.min_y]);
        // Top-right corner
        if tr > 0.0 {
            let p0 = [rect.max_x - tr, rect.min_y];
            let p1 = [rect.max_x - tr + tr * KAPPA, rect.min_y];
            let p2 = [rect.max_x, rect.min_y + tr - tr * KAPPA];
            let p3 = [rect.max_x, rect.min_y + tr];
            flatten_cubic(p0, p1, p2, p3, policy, 0, &mut points);
        }
        // Right edge
        points.push([rect.max_x, rect.max_y - br]);
        // Bottom-right corner
        if br > 0.0 {
            let p0 = [rect.max_x, rect.max_y - br];
            let p1 = [rect.max_x, rect.max_y - br + br * KAPPA];
            let p2 = [rect.max_x - br + br * KAPPA, rect.max_y];
            let p3 = [rect.max_x - br, rect.max_y];
            flatten_cubic(p0, p1, p2, p3, policy, 0, &mut points);
        }
        // Bottom edge
        points.push([rect.min_x + bl, rect.max_y]);
        // Bottom-left corner
        if bl > 0.0 {
            let p0 = [rect.min_x + bl, rect.max_y];
            let p1 = [rect.min_x + bl - bl * KAPPA, rect.max_y];
            let p2 = [rect.min_x, rect.max_y - bl + bl * KAPPA];
            let p3 = [rect.min_x, rect.max_y - bl];
            flatten_cubic(p0, p1, p2, p3, policy, 0, &mut points);
        }
        // Left edge
        points.push([rect.min_x, rect.min_y + tl]);
        // Top-left corner
        if tl > 0.0 {
            let p0 = [rect.min_x, rect.min_y + tl];
            let p1 = [rect.min_x, rect.min_y + tl - tl * KAPPA];
            let p2 = [rect.min_x + tl - tl * KAPPA, rect.min_y];
            let p3 = [rect.min_x + tl, rect.min_y];
            flatten_cubic(p0, p1, p2, p3, policy, 0, &mut points);
        }

        self.push_path(&points, true)
    }

    /// Appends a closed circle path approximated by cubic Béziers.
    pub fn push_circle(
        &mut self,
        center: [f32; 2],
        radius: f32,
        policy: Option<CurveFlatteningPolicy>,
    ) -> Result<PathId, EncodeError> {
        let rect = Rect::new(
            [center[0] - radius, center[1] - radius],
            [center[0] + radius, center[1] + radius],
        );
        self.push_rounded_rect(rect, [radius; 4], policy)
    }

    /// Finishes encoding without reordering any stream.
    pub fn finish(self) -> Scene {
        self.scene
    }
}

/// Configurable policy for flattening curves into line segments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveFlatteningPolicy {
    /// Maximum allowable deviation in pixels between the curve and its flattened segments.
    pub tolerance: f32,
    /// Maximum recursive subdivision depth.
    pub max_depth: u32,
}

impl Default for CurveFlatteningPolicy {
    fn default() -> Self {
        Self {
            tolerance: 0.25,
            max_depth: 10,
        }
    }
}

fn flatten_quadratic(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    policy: CurveFlatteningPolicy,
    depth: u32,
    points: &mut Vec<[f32; 2]>,
) {
    let tol_sq = policy.tolerance * policy.tolerance;
    let mut stack: [([f32; 2], [f32; 2], [f32; 2], u32); 32] =
        [([0.0, 0.0], [0.0, 0.0], [0.0, 0.0], 0); 32];
    let mut stack_len = 0;
    let mut curr = (p0, p1, p2, depth);

    loop {
        let (c0, c1, c2, d) = curr;
        let dx = 0.5 * c1[0] - 0.25 * (c0[0] + c2[0]);
        let dy = 0.5 * c1[1] - 0.25 * (c0[1] + c2[1]);
        let d2 = dx * dx + dy * dy;

        if d2 <= tol_sq || d >= policy.max_depth || stack_len >= 31 {
            points.push(c2);
            if stack_len > 0 {
                stack_len -= 1;
                curr = stack[stack_len];
            } else {
                break;
            }
        } else {
            let p01 = [0.5 * (c0[0] + c1[0]), 0.5 * (c0[1] + c1[1])];
            let p12 = [0.5 * (c1[0] + c2[0]), 0.5 * (c1[1] + c2[1])];
            let mid = [0.5 * (p01[0] + p12[0]), 0.5 * (p01[1] + p12[1])];

            stack[stack_len] = (mid, p12, c2, d + 1);
            stack_len += 1;
            curr = (c0, p01, mid, d + 1);
        }
    }
}

fn flatten_cubic(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    policy: CurveFlatteningPolicy,
    depth: u32,
    points: &mut Vec<[f32; 2]>,
) {
    let tol_sq = policy.tolerance * policy.tolerance;
    let mut stack: [([f32; 2], [f32; 2], [f32; 2], [f32; 2], u32); 32] =
        [([0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0], 0); 32];
    let mut stack_len = 0;
    let mut curr = (p0, p1, p2, p3, depth);

    loop {
        let (c0, c1, c2, c3, d) = curr;
        let dx = c3[0] - c0[0];
        let dy = c3[1] - c0[1];
        let len_sq = dx * dx + dy * dy;

        let is_flat = if len_sq > 1e-6 {
            let tol_sq_len = tol_sq * len_sq;
            let cross1 = (c1[0] - c0[0]) * dy - (c1[1] - c0[1]) * dx;
            if cross1 * cross1 <= tol_sq_len {
                let cross2 = (c2[0] - c0[0]) * dy - (c2[1] - c0[1]) * dx;
                cross2 * cross2 <= tol_sq_len
            } else {
                false
            }
        } else {
            let dev1 = (c1[0] - c0[0]) * (c1[0] - c0[0]) + (c1[1] - c0[1]) * (c1[1] - c0[1]);
            let dev2 = (c2[0] - c0[0]) * (c2[0] - c0[0]) + (c2[1] - c0[1]) * (c2[1] - c0[1]);
            dev1 <= tol_sq && dev2 <= tol_sq
        };

        if is_flat || d >= policy.max_depth || stack_len >= 31 {
            points.push(c3);
            if stack_len > 0 {
                stack_len -= 1;
                curr = stack[stack_len];
            } else {
                break;
            }
        } else {
            let p01 = [0.5 * (c0[0] + c1[0]), 0.5 * (c0[1] + c1[1])];
            let p12 = [0.5 * (c1[0] + c2[0]), 0.5 * (c1[1] + c2[1])];
            let p23 = [0.5 * (c2[0] + c3[0]), 0.5 * (c2[1] + c3[1])];
            let p012 = [0.5 * (p01[0] + p12[0]), 0.5 * (p01[1] + p12[1])];
            let p123 = [0.5 * (p12[0] + p23[0]), 0.5 * (p12[1] + p23[1])];
            let mid = [0.5 * (p012[0] + p123[0]), 0.5 * (p012[1] + p123[1])];

            stack[stack_len] = (mid, p123, p23, c3, d + 1);
            stack_len += 1;
            curr = (c0, p01, p012, mid, d + 1);
        }
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

    /// Workgroup dispatch counts for stage 3 and 4 compute passes.
    pub fn dispatch_workgroups(self) -> (u32, u32, u32) {
        (self.columns, self.rows, 1)
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

/// WGSL shader sources for the four GPU compute rasterizer stages.
pub mod shader {
    /// WGSL source for Stage 2 `path_coarse` segment binning.
    pub const PATH_COARSE: &str = include_str!("../shader/path_coarse.wgsl");
    /// WGSL source for Stage 3 `coarse` clip resolution and damage culling.
    pub const COARSE: &str = include_str!("../shader/coarse.wgsl");
    /// WGSL source for Stage 4 `fine` analytic coverage and linear HDR blending.
    pub const FINE: &str = include_str!("../shader/fine.wgsl");
}

/// GPU representation of a material in the material storage buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct ComputeMaterial {
    /// RGBA linear-float color.
    pub color: [f32; 4],
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

    #[test]
    fn compute_shaders_are_embedded_and_valid() {
        assert!(shader::PATH_COARSE.contains("@compute"));
        assert!(shader::COARSE.contains("@compute"));
        assert!(shader::FINE.contains("@compute"));
        assert!(shader::FINE.contains("rgba16float"));
    }

    #[test]
    fn tile_grid_dispatch_workgroups() {
        let grid = TileGrid::new(1920, 1080);
        assert_eq!(grid.columns(), 120);
        assert_eq!(grid.rows(), 68);
        assert_eq!(grid.dispatch_workgroups(), (120, 68, 1));
        assert_eq!(grid.len(), 120 * 68);
    }

    #[test]
    fn quadratic_and_cubic_bezier_flattening() {
        let mut encoder = Encoder::new();
        let quad_path = encoder
            .push_quadratic_bezier([0.0, 0.0], [50.0, 100.0], [100.0, 0.0], None)
            .expect("quadratic bezier");
        let cubic_path = encoder
            .push_cubic_bezier(
                [0.0, 0.0],
                [30.0, 90.0],
                [70.0, 90.0],
                [100.0, 0.0],
                Some(CurveFlatteningPolicy {
                    tolerance: 0.1,
                    max_depth: 8,
                }),
            )
            .expect("cubic bezier");

        let scene = encoder.finish();
        assert!(scene.paths()[quad_path.0 as usize].segment_count >= 4);
        assert!(scene.paths()[cubic_path.0 as usize].segment_count >= 8);
    }

    #[test]
    fn rounded_rect_and_circle_geometry() {
        let mut encoder = Encoder::new();
        let rect = Rect::new([0.0, 0.0], [120.0, 80.0]);
        let rr_path = encoder
            .push_rounded_rect(rect, [10.0, 10.0, 10.0, 10.0], None)
            .expect("rounded rect");
        let circle_path = encoder
            .push_circle([50.0, 50.0], 25.0, None)
            .expect("circle");

        let scene = encoder.finish();
        assert!(scene.paths()[rr_path.0 as usize].segment_count >= 16);
        assert!(scene.paths()[circle_path.0 as usize].segment_count >= 16);
    }

    #[test]
    fn scene_delta_edits_and_upload_bytes() {
        let mut encoder = Encoder::new();
        let path = square(&mut encoder);
        encoder
            .push_draw_with_velocity(
                path,
                None,
                MaterialId(1),
                1.0,
                ObjectVelocity {
                    linear: [100.0, 0.0],
                    angular: 0.0,
                    scale_rate: 0.0,
                },
            )
            .expect("draw with velocity");
        let mut scene = encoder.finish();
        assert_eq!(scene.velocities().len(), 1);
        assert_eq!(scene.velocities()[0].linear, [100.0, 0.0]);

        // Test delta patch
        let edit = SceneEdit::UpdateVelocity {
            draw_index: 0,
            velocity: ObjectVelocity {
                linear: [250.0, -50.0],
                angular: 1.5,
                scale_rate: 0.0,
            },
        };
        assert_eq!(
            scene.delta_upload_bytes(&edit),
            std::mem::size_of::<ObjectVelocity>()
        );
        scene.apply_edit(&edit);
        assert_eq!(scene.velocities()[0].linear, [250.0, -50.0]);
        assert_eq!(scene.velocities()[0].angular, 1.5);
    }

    #[test]
    fn push_rect_fast_path() {
        let mut encoder = Encoder::new();
        let rect = Rect::new([10.0, 20.0], [110.0, 220.0]);
        let path_id = encoder.push_rect(rect).expect("push_rect");
        let scene = encoder.finish();

        let path = scene.paths()[path_id.0 as usize];
        assert_eq!(path.segment_count, 4);
        let segs = &scene.segments()
            [path.first_segment as usize..(path.first_segment + path.segment_count) as usize];
        assert_eq!(segs[0].from, [10.0, 20.0]);
        assert_eq!(segs[0].to, [110.0, 20.0]);
        assert_eq!(segs[1].from, [110.0, 20.0]);
        assert_eq!(segs[1].to, [110.0, 220.0]);
        assert_eq!(segs[2].from, [110.0, 220.0]);
        assert_eq!(segs[2].to, [10.0, 220.0]);
        assert_eq!(segs[3].from, [10.0, 220.0]);
        assert_eq!(segs[3].to, [10.0, 20.0]);
    }
}
