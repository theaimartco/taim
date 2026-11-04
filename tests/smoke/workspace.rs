//! Smoke checks for the current facade and renderer-neutral contracts.

#[test]
fn application_builder_and_view_prelude_compile() {
    use taim::prelude::*;
    let _app = taim::app("Smoke", |cx| {
        let value = cx.signal(String::new());
        column(views![text("Ready"), text_field("Name", value)])
    })
    .size(640.0, 480.0);
}

#[test]
fn core_types_are_backend_independent() {
    use taim_core::{Point, Rectangle, Size};
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(1.0, 1.0));
    assert_eq!(bounds.width, 1.0);
    assert_eq!(bounds.height, 1.0);
}

#[test]
fn custom_view_contract_is_public() {
    fn view<V: taim::view::View>(value: V) -> taim::view::AnyView {
        value.into()
    }
    let _ = view(taim::view::views::text("Custom view boundary"));
}
