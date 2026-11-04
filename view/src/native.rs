//! Explicit Android hierarchy slots, separate from GPU texture import/composition.
use crate::{
    Axis, Constraints, DescribeCx, IntoProp, MeasureCx, PaintCx, Prop, Rectangle, Size, Sizing,
    View, semantics,
};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

/// Invalid native factory name or property data.
#[derive(Debug, thiserror::Error)]
pub enum NativeViewError {
    /// Factory names use 1–128 ASCII letters/digits/dots/dashes/underscores.
    #[error("invalid Android view factory")]
    Factory,
    /// Properties must be a JSON object of at most 16 KiB.
    #[error("invalid Android view properties")]
    Properties,
}
/// A native slot's immutable identity and current property snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeViewDescriptor {
    /// Stable slot identity; new view instances receive new IDs.
    pub id: u64,
    /// Explicitly registered Java/Kotlin factory name.
    pub factory: String,
    /// Current JSON object, or null when an invalid reactive update fails closed.
    pub properties: Value,
    /// Whether native interaction is enabled.
    pub enabled: bool,
}
/// Window-logical placement of an Android hierarchy slot.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeViewPlacement {
    /// Native descriptor.
    pub view: NativeViewDescriptor,
    /// Full unclipped logical frame.
    pub bounds: Rectangle,
    /// Visible ancestor-clipped frame; None means hidden (including modal occlusion).
    pub visible: Option<Rectangle>,
}
/// Reserve layout space for an explicitly registered Android native view.
/// Other platform shells leave the slot empty; this is not a cross-platform widget.
pub fn android_view(
    factory: impl Into<String>,
    properties: impl IntoProp<Value>,
) -> Result<AndroidView, NativeViewError> {
    let factory = factory.into();
    if factory.is_empty()
        || factory.len() > 128
        || !factory
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(NativeViewError::Factory);
    }
    let properties = properties.into_prop();
    if !valid(&properties.get()) {
        return Err(NativeViewError::Properties);
    }
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| {
            (id < i64::MAX as u64).then_some(id + 1)
        })
        .expect("native view identities exhausted");
    Ok(AndroidView {
        descriptor: NativeViewDescriptor {
            id,
            factory,
            properties: Value::Null,
            enabled: true,
        },
        properties,
        enabled: Prop::fixed(true),
        sizing: Sizing {
            width: Axis::Fill(1),
            height: Axis::Fixed(160.0),
        },
    })
}
fn valid(value: &Value) -> bool {
    value.is_object() && value.to_string().len() <= 16 * 1024
}
impl IntoProp<Value> for Value {
    fn into_prop(self) -> Prop<Value> {
        Prop::fixed(self)
    }
}
/// See android_view. Native state survives property updates and suspend/resume while this view is retained.
pub struct AndroidView {
    descriptor: NativeViewDescriptor,
    properties: Prop<Value>,
    enabled: Prop<bool>,
    sizing: Sizing,
}
impl AndroidView {
    /// Slot identity for filtering PlatformEvent::NativeView events.
    pub fn id(&self) -> u64 {
        self.descriptor.id
    }
    /// Fixed logical width, normalized to 0 for invalid/oversized values.
    pub fn width(mut self, width: f32) -> Self {
        self.sizing.width = Axis::Fixed(if width.is_finite() && (0.0..=8192.0).contains(&width) {
            width
        } else {
            0.0
        });
        self
    }
    /// Fixed logical height.
    pub fn height(mut self, height: f32) -> Self {
        self.sizing.height =
            Axis::Fixed(if height.is_finite() && (0.0..=8192.0).contains(&height) {
                height
            } else {
                0.0
            });
        self
    }
    /// Gate native interaction. Use factory properties for fine-grained child-control states.
    pub fn enabled(mut self, enabled: impl IntoProp<bool>) -> Self {
        self.enabled = enabled.into_prop();
        self
    }
}
impl View for AndroidView {
    fn sizing(&self) -> Sizing {
        self.sizing
    }
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.with_sizing(self.sizing).min
    }
    fn paint(&self, _: &mut PaintCx<'_>) {}
    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
    fn native_view(&self) -> Option<NativeViewDescriptor> {
        let mut descriptor = self.descriptor.clone();
        let properties = self.properties.get();
        descriptor.properties = if valid(&properties) {
            properties
        } else {
            Value::Null
        };
        descriptor.enabled = self.enabled.get();
        Some(descriptor)
    }
}
