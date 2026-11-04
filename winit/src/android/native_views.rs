//! Density/origin conversion for native hierarchy overlays.
use serde_json::{Value, json};
use taim_core::{Point, Rectangle};
pub(crate) fn scene(
    views: Vec<taim_view::NativeViewPlacement>,
    origin: Point,
    scale: f32,
    clip: Rectangle,
) -> Value {
    let error = |code| json!({"views":[],"error":code});
    if views.len() > 64 {
        return error("tooManyViews");
    }
    if !scale.is_finite() || scale <= 0.0 {
        return error("invalidGeometry");
    }
    let mut total = 0.0;
    let mut entries = Vec::new();
    for view in views {
        let shift = origin - Point::ORIGIN;
        let bounds = Rectangle::new(view.bounds.position() + shift, view.bounds.size()) * scale;
        if [bounds.x, bounds.y, bounds.width, bounds.height]
            .iter()
            .any(|n| !n.is_finite())
            || bounds.width < 0.0
            || bounds.height < 0.0
            || bounds.width > 8192.0
            || bounds.height > 8192.0
            || bounds.width * bounds.height > 8_388_608.0
        {
            return error("invalidGeometry");
        }
        total += bounds.width * bounds.height;
        if total > 33_554_432.0 {
            return error("layoutBudget");
        }
        let visible = view
            .visible
            .map(|r| Rectangle::new(r.position() + shift, r.size()))
            .and_then(|r| r.intersection(&clip));
        let visible = visible
            .map(|r| r * scale)
            .filter(|r| r.width > 0.0 && r.height > 0.0);
        let rect = |r: Rectangle| json!([r.x, r.y, r.width, r.height]);
        entries.push(json!({"id":view.view.id,"factory":view.view.factory,"properties":view.view.properties,"enabled":view.view.enabled,"bounds":rect(bounds),"clip":visible.map(rect)}));
    }
    let value = json!({"views":entries});
    if value.to_string().len() > 2 * 1024 * 1024 {
        error("sceneTooLarge")
    } else {
        value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use taim_core::Size;
    #[test]
    fn native_coordinates_apply_origin_density_and_viewport_clip_once() {
        let rect = Rectangle::new(Point::new(10.0, 20.0), Size::new(100.0, 80.0));
        let view = taim_view::NativeViewPlacement {
            view: taim_view::NativeViewDescriptor {
                id: 1,
                factory: "test".into(),
                properties: json!({}),
                enabled: true,
            },
            bounds: rect,
            visible: Some(rect),
        };
        let value = scene(
            vec![view],
            Point::new(5.0, -10.0),
            2.0,
            Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 50.0)),
        );
        assert_eq!(
            value["views"][0]["bounds"],
            json!([30.0, 20.0, 200.0, 160.0])
        );
        assert_eq!(value["views"][0]["clip"], json!([30.0, 20.0, 170.0, 80.0]));
    }
}
