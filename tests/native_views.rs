//! Exercise the current application contract through a real CPU renderer.
use taim::prelude::*;
use taim::view::Root;
use taim::{Color, Point, Rectangle, Size};
use taim_core::input_method::{self, InputMethod, Purpose};
use taim_core::renderer::Headless as _;
use taim_core::time::Instant;
use taim_core::{Event, Renderer as _, keyboard, mouse};

fn renderer() -> taim::Renderer {
    taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("CPU renderer")
}

#[test]
fn glass_fallback_draws_content_and_reduced_transparency_is_opaque() {
    // A software-rendered glass panel must remain usable without GPU refraction.
    let mut renderer = renderer();
    let mut glass = Root::new(|_| {
        container(text("Readable glass"))
            .glass()
            .width(240.0)
            .height(80.0)
    });
    let glass_pixels = screenshot(&mut glass, &mut renderer);
    let mut flat = Root::new(|_| {
        container(text("Readable glass"))
            .glass()
            .reduce_transparency(true)
            .width(240.0)
            .height(80.0)
    });
    let flat_pixels = screenshot(&mut flat, &mut renderer);
    assert!(flat_pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
    assert!(
        glass_pixels
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [255, 255, 255])
    );
    let mut empty = Root::new(|_| {
        container(text(""))
            .glass()
            .reduce_transparency(true)
            .width(240.0)
            .height(80.0)
    });
    assert_ne!(
        flat_pixels,
        screenshot(&mut empty, &mut renderer),
        "glass must keep child text visible"
    );
}

#[test]
fn native_glass_captures_the_prior_layer_and_preserves_child_paint() {
    let Some(mut renderer) = taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("wgpu"),
    )) else {
        eprintln!("skipped: no GPU adapter for native glass integration");
        return;
    };
    let scene = |reduce| {
        Root::new(move |_| {
            layers(views![
                row(views![
                    container(text(""))
                        .background(Color::from_rgb(0.1, 0.3, 0.9))
                        .width(160.0)
                        .height(120.0),
                    container(text(""))
                        .background(Color::from_rgb(0.9, 0.2, 0.4))
                        .width(160.0)
                        .height(120.0),
                ]),
                container(
                    container(text("Taim Glass").size(28))
                        .padding(20.0)
                        .frosted_glass()
                        .reduce_transparency(reduce)
                        .width(280.0)
                        .height(80.0)
                )
                .padding(20.0),
            ])
        })
    };
    let mut glass = scene(false);
    let pixels = screenshot(&mut glass, &mut renderer);
    let mut opaque = scene(true);
    let fallback = screenshot(&mut opaque, &mut renderer);
    if cfg!(feature = "web-colors") {
        assert_eq!(
            pixels, fallback,
            "legacy gamma blending must use the opaque material fallback"
        );
        return;
    }
    // Interior samples must retain different backdrop colours. An absent
    // layer capture would produce the same tint on both sides of the pane.
    let at = |x: usize, y: usize| &pixels[(y * 320 + x) * 4..(y * 320 + x) * 4 + 3];
    assert_ne!(
        at(70, 85),
        at(250, 85),
        "glass must transmit the prior layer"
    );
    assert_ne!(
        pixels, fallback,
        "reduced transparency must disable transmission"
    );
    if let Ok(path) = std::env::var("TAIM_GLASS_PREVIEW_PPM") {
        let mut ppm = b"P6\n320 120\n255\n".to_vec();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(path, ppm).expect("write requested glass preview");
    }
}

fn focus(root: &mut Root, renderer: &taim::Renderer) {
    root.layout(renderer, Size::new(320.0, 120.0));
    let at = Point::new(15.0, 15.0);
    for event in [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ] {
        let _ = root.handle(&event, Some(at), Instant::now());
    }
    assert!(root.input_method().is_enabled());
}

fn ime(root: &mut Root, event: input_method::Event) -> taim::view::Response {
    root.handle(&Event::InputMethod(event), None, Instant::now())
}

fn screenshot(root: &mut Root, renderer: &mut taim::Renderer) -> Vec<u8> {
    let size = Size::new(320.0, 120.0);
    root.layout(renderer, size);
    renderer.reset(Rectangle::with_size(size));
    root.paint(renderer, Color::BLACK, None, Instant::now());
    renderer.screenshot(Size::new(320, 120), 1.0, Color::WHITE)
}

#[test]
fn composition_is_visible_without_modifying_state_and_commit_inserts_once() {
    let mut value = None;
    let mut root = Root::new(|cx| {
        let state = cx.signal(String::new());
        value = Some(state);
        text_field("", state)
    });
    let value = value.unwrap();
    let mut renderer = renderer();
    assert_eq!(root.input_method(), InputMethod::Disabled);
    focus(&mut root, &renderer);
    let before = screenshot(&mut root, &mut renderer);
    assert!(
        ime(
            &mut root,
            input_method::Event::Preedit("日本".into(), Some(0..6))
        )
        .paint
    );
    assert_eq!(value.get(), "");
    let composing = screenshot(&mut root, &mut renderer);
    assert_ne!(
        composing, before,
        "composition must draw pixels before commit"
    );

    // Raw keys from the composing sequence must not edit the buffer twice.
    let key = keyboard::Key::Character("x".into());
    let event = Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: Some("x".into()),
        repeat: false,
    });
    assert!(root.handle(&event, None, Instant::now()).captured);
    assert_eq!(value.get(), "");
    assert!(ime(&mut root, input_method::Event::Commit("日本".into())).paint);
    assert_eq!(value.get(), "日本");
    assert!(matches!(
        root.input_method(),
        InputMethod::Enabled { preedit: None, .. }
    ));
}

#[test]
fn cancel_repaints_and_malformed_selection_never_reaches_rendering() {
    let mut root = Root::new(|cx| {
        let value = cx.signal(String::new());
        text_field("", value)
    });
    let mut renderer = renderer();
    focus(&mut root, &renderer);
    let before = screenshot(&mut root, &mut renderer);
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("é".into(), Some(1..99)),
    );
    assert!(matches!(
        root.input_method(),
        InputMethod::Enabled {
            preedit: Some(input_method::Preedit {
                selection: None,
                ..
            }),
            ..
        }
    ));
    assert!(ime(&mut root, input_method::Event::Closed).paint);
    assert_eq!(screenshot(&mut root, &mut renderer), before);
}

#[test]
fn password_composition_masks_different_secrets_identically() {
    let mut root = Root::new(|cx| {
        let value = cx.signal(String::new());
        text_field("", value).secret()
    });
    let mut renderer = renderer();
    focus(&mut root, &renderer);
    assert!(matches!(
        root.input_method(),
        InputMethod::Enabled {
            purpose: Purpose::Secure,
            ..
        }
    ));
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("secret".into(), None),
    );
    let first = screenshot(&mut root, &mut renderer);
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("hidden".into(), None),
    );
    assert_eq!(screenshot(&mut root, &mut renderer), first);
}

#[test]
fn multiline_editor_accepts_composed_text_through_the_same_contract() {
    let mut buffer = None;
    let mut root = Root::new(|cx| {
        let text = Buffer::new(cx, "");
        buffer = Some(text);
        text_editor(&text)
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let _ = ime(&mut root, input_method::Event::Preedit("नमस्ते".into(), None));
    assert!(buffer.unwrap().text().is_empty());
    let _ = ime(&mut root, input_method::Event::Commit("नमस्ते".into()));
    assert_eq!(buffer.unwrap().text(), "नमस्ते");
}

#[test]
fn composition_does_not_survive_window_blur_or_clicking_outside_the_editor() {
    let mut root = Root::new(|cx| {
        let value = cx.signal(String::new());
        text_field("", value)
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("compose".into(), None),
    );
    assert!(
        root.handle(
            &Event::Window(taim_core::window::Event::Unfocused),
            None,
            Instant::now()
        )
        .paint
    );
    assert!(matches!(
        root.input_method(),
        InputMethod::Enabled { preedit: None, .. }
    ));
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("compose".into(), None),
    );
    let _ = root.handle(
        &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Some(Point::new(319.0, 119.0)),
        Instant::now(),
    );
    assert_eq!(root.input_method(), InputMethod::Disabled);
}

#[test]
fn button_does_not_activate_after_focus_loss_or_a_disabled_release() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let clicks = cx.signal(0);
        let enabled = cx.signal(true);
        state = Some((clicks, enabled));
        button("Run")
            .enabled(enabled)
            .on_press(move || clicks.update(|n| *n += 1))
    });
    let (clicks, enabled) = state.unwrap();
    root.layout(&renderer(), Size::new(320.0, 120.0));
    let at = Some(Point::new(15.0, 15.0));
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    let _ = root.handle(&press, at, Instant::now());
    let _ = root.handle(
        &Event::Window(taim_core::window::Event::Unfocused),
        at,
        Instant::now(),
    );
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 0);

    let _ = root.handle(&press, at, Instant::now());
    let _ = enabled.set(false);
    let _ = root.handle(&release, at, Instant::now());
    let _ = enabled.set(true);
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 0);
    let _ = root.handle(&press, at, Instant::now());
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn cancelled_touch_never_commits_a_pull_to_refresh() {
    use taim_core::{pointer, touch};
    let mut count = None;
    let mut root = Root::new(|cx| {
        let refreshing = cx.signal(false);
        let calls = cx.signal(0);
        count = Some(calls);
        pull_to_refresh(space(), refreshing).on_refresh(move || calls.update(|n| *n += 1))
    });
    root.layout(&renderer(), Size::new(320.0, 300.0));
    let id = touch::Finger(1);
    let metadata = pointer::Metadata::NONE;
    let start = Point::new(15.0, 15.0);
    let end = Point::new(15.0, 250.0);
    let down = Event::Touch(touch::Event::FingerPressed {
        id,
        position: start,
        metadata,
    });
    let moved = Event::Touch(touch::Event::FingerMoved {
        id,
        position: end,
        metadata,
    });
    let up = Event::Touch(touch::Event::FingerLifted {
        id,
        position: end,
        metadata,
    });
    for event in [
        &down,
        &moved,
        &Event::Touch(touch::Event::FingerLost {
            id,
            position: end,
            metadata,
        }),
        &up,
    ] {
        let _ = root.handle(event, None, Instant::now());
    }
    assert_eq!(count.unwrap().get(), 0);
    for event in [&down, &moved, &up] {
        let _ = root.handle(event, None, Instant::now());
    }
    assert_eq!(count.unwrap().get(), 1);
}

fn named_key(named: keyboard::key::Named, modifiers: keyboard::Modifiers) -> Event {
    let key = keyboard::Key::Named(named);
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    })
}

#[test]
fn clicking_a_button_takes_keyboard_focus_and_layout_preserves_it() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let clicks = cx.signal(0);
        state = Some(clicks);
        button("Run").on_press(move || clicks.update(|n| *n += 1))
    });
    let renderer = renderer();
    root.layout(&renderer, Size::new(320.0, 120.0));
    let at = Some(Point::new(15.0, 15.0));
    for event in [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE),
    ] {
        assert!(root.handle(&event, at, Instant::now()).captured);
    }
    assert_eq!(state.unwrap().get(), 2);
    root.layout(&renderer, Size::new(640.0, 240.0));
    assert!(
        root.handle(
            &named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE),
            None,
            Instant::now()
        )
        .captured
    );
    assert_eq!(state.unwrap().get(), 3);
}

#[test]
fn shortcut_tab_is_not_stolen_by_focus_navigation() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let clicks = cx.signal(0);
        state = Some(clicks);
        row(views![
            button("First").on_press(move || clicks.update(|n| *n += 1)),
            button("Second").on_press(move || clicks.update(|n| *n += 10)),
        ])
    });
    root.layout(&renderer(), Size::new(320.0, 120.0));
    let tab = |modifiers| named_key(keyboard::key::Named::Tab, modifiers);
    let enter = named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE);
    assert!(
        root.handle(&tab(keyboard::Modifiers::NONE), None, Instant::now())
            .captured
    );
    for modifiers in [
        keyboard::Modifiers::CTRL,
        keyboard::Modifiers::ALT,
        keyboard::Modifiers::LOGO,
    ] {
        assert!(!root.handle(&tab(modifiers), None, Instant::now()).captured);
        assert!(root.handle(&enter, None, Instant::now()).captured);
    }
    assert_eq!(state.unwrap().get(), 3);
    let _ = root.handle(&tab(keyboard::Modifiers::NONE), None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(state.unwrap().get(), 13);
    let _ = root.handle(&tab(keyboard::Modifiers::SHIFT), None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(state.unwrap().get(), 14);
}

#[test]
fn replacing_a_focused_control_does_not_transfer_keyboard_activation() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let key = cx.signal(0);
        let clicks = cx.signal(0);
        state = Some((key, clicks));
        rebuild(key, move |key| {
            Some(
                button(format!("Action {key}"))
                    .on_press(move || clicks.update(|n| *n += 1))
                    .into(),
            )
        })
    });
    let (key, clicks) = state.unwrap();
    let renderer = renderer();
    root.layout(&renderer, Size::new(320.0, 120.0));
    let tab = named_key(keyboard::key::Named::Tab, keyboard::Modifiers::NONE);
    let enter = named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE);
    let _ = root.handle(&tab, None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 1);
    let _ = key.set(1);
    root.layout(&renderer, Size::new(320.0, 120.0));
    assert!(!root.handle(&enter, None, Instant::now()).captured);
    assert_eq!(clicks.get(), 1);
    let _ = root.handle(&tab, None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 2);
}

#[test]
fn disabling_a_focused_control_does_not_restore_focus_when_reenabled() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let enabled = cx.signal(true);
        let clicks = cx.signal(0);
        state = Some((enabled, clicks));
        button("Run")
            .enabled(enabled)
            .on_press(move || clicks.update(|n| *n += 1))
    });
    let (enabled, clicks) = state.unwrap();
    let renderer = renderer();
    root.layout(&renderer, Size::new(320.0, 120.0));
    let _ = root.handle(
        &named_key(keyboard::key::Named::Tab, keyboard::Modifiers::NONE),
        None,
        Instant::now(),
    );
    let _ = enabled.set(false);
    root.layout(&renderer, Size::new(320.0, 120.0));
    let _ = enabled.set(true);
    root.layout(&renderer, Size::new(320.0, 120.0));
    assert!(
        !root
            .handle(
                &named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE),
                None,
                Instant::now()
            )
            .captured
    );
    assert_eq!(clicks.get(), 0);
}

#[test]
fn touch_button_tracks_its_own_finger_and_does_not_accept_a_mouse_release() {
    use taim_core::{pointer, touch};
    let mut count = None;
    let mut root = Root::new(|cx| {
        let clicks = cx.signal(0);
        count = Some(clicks);
        button("Tap").on_press(move || clicks.update(|n| *n += 1))
    });
    root.layout(&renderer(), Size::new(320.0, 120.0));
    let at = Point::new(15.0, 15.0);
    let metadata = pointer::Metadata::NONE;
    let first = touch::Finger(1);
    let other = touch::Finger(2);
    let down = Event::Touch(touch::Event::FingerPressed {
        id: first,
        position: at,
        metadata,
    });
    let up = Event::Touch(touch::Event::FingerLifted {
        id: first,
        position: at,
        metadata,
    });
    assert!(root.handle(&down, None, Instant::now()).captured);
    for event in [
        Event::Touch(touch::Event::FingerLost {
            id: other,
            position: at,
            metadata,
        }),
        Event::Touch(touch::Event::FingerLifted {
            id: other,
            position: at,
            metadata,
        }),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ] {
        let _ = root.handle(&event, Some(at), Instant::now());
    }
    assert_eq!(count.unwrap().get(), 0);
    assert!(root.handle(&up, None, Instant::now()).captured);
    assert_eq!(count.unwrap().get(), 1);
    let _ = root.handle(&down, None, Instant::now());
    let _ = root.handle(
        &Event::Touch(touch::Event::FingerLost {
            id: first,
            position: at,
            metadata,
        }),
        None,
        Instant::now(),
    );
    let _ = root.handle(&up, None, Instant::now());
    assert_eq!(count.unwrap().get(), 1);
}

#[test]
fn clipped_touch_content_cannot_be_pressed_or_activated_outside_the_viewport() {
    use taim_core::{pointer, touch};
    let mut count = None;
    let mut root = Root::new(|cx| {
        let clicks = cx.signal(0);
        count = Some(clicks);
        scroll(
            button("Tall action")
                .label_view(space().width(120.0).height(200.0))
                .on_press(move || clicks.update(|n| *n += 1)),
        )
    });
    root.layout(&renderer(), Size::new(320.0, 50.0));
    let event = |pressed, y| {
        let id = touch::Finger(1);
        let position = Point::new(15.0, y);
        let metadata = pointer::Metadata::NONE;
        Event::Touch(if pressed {
            touch::Event::FingerPressed {
                id,
                position,
                metadata,
            }
        } else {
            touch::Event::FingerLifted {
                id,
                position,
                metadata,
            }
        })
    };
    for (pressed, y) in [(true, 15.0), (false, 100.0), (true, 100.0), (false, 15.0)] {
        let _ = root.handle(&event(pressed, y), None, Instant::now());
    }
    assert_eq!(count.unwrap().get(), 0);
    for pressed in [true, false] {
        let _ = root.handle(&event(pressed, 15.0), None, Instant::now());
    }
    assert_eq!(count.unwrap().get(), 1);
}

#[test]
fn replacing_a_composing_text_field_clears_focus_and_pending_ime_input() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let key = cx.signal(0);
        let value = cx.signal(String::new());
        state = Some((key, value));
        rebuild(key, move |_| Some(text_field("", value).into()))
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("日本".into(), Some(0..6)),
    );
    let (key, value) = state.unwrap();
    let _ = key.set(1);
    root.layout(&renderer, Size::new(320.0, 120.0));
    assert_eq!(root.input_method(), InputMethod::Disabled);
    assert!(!ime(&mut root, input_method::Event::Commit("日本".into())).captured);
    assert_eq!(value.get(), "");
}

#[test]
fn focus_follows_a_retained_child_when_its_parent_reorders_it() {
    use taim::view::{Child, Constraints, DescribeCx, MeasureCx, PaintCx, State, View};
    struct SwappingRow {
        children: [Child; 2],
        reverse: State<bool>,
        reversed: bool,
    }
    impl View for SwappingRow {
        fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
            let reverse = self.reverse.get();
            if reverse != self.reversed {
                self.children.swap(0, 1);
                self.reversed = reverse;
            }
            for (index, child) in self.children.iter_mut().enumerate() {
                let _ = child.measure(cx, Constraints::loose(Size::new(120.0, 40.0)));
                child.place(Point::new(index as f32 * 120.0, 0.0));
            }
            constraints.clamp(Size::new(240.0, 40.0))
        }
        fn paint(&self, _: &mut PaintCx<'_>) {}
        fn describe(&self, _: &DescribeCx<'_>) -> Option<taim_core::semantics::Node> {
            None
        }
        fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
            for child in &mut self.children {
                visit(child);
            }
        }
    }
    let mut state = None;
    let mut root = Root::new(|cx| {
        let reverse = cx.signal(false);
        let clicks = cx.signal(0);
        state = Some((reverse, clicks));
        SwappingRow {
            children: [
                Child::new(button("A").on_press(move || clicks.update(|n| *n += 1))),
                Child::new(button("B").on_press(move || clicks.update(|n| *n += 10))),
            ],
            reverse,
            reversed: false,
        }
    });
    let (reverse, clicks) = state.unwrap();
    let renderer = renderer();
    root.layout(&renderer, Size::new(320.0, 120.0));
    let tab = named_key(keyboard::key::Named::Tab, keyboard::Modifiers::NONE);
    let enter = named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE);
    let _ = root.handle(&tab, None, Instant::now());
    let _ = reverse.set(true);
    root.layout(&renderer, Size::new(320.0, 120.0));
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(
        clicks.get(),
        1,
        "A retained focus when moved to the second slot"
    );
    let _ = root.handle(&tab, None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 11, "Tab follows the updated visual order");
}

#[test]
fn disabling_during_a_press_cancels_it_even_without_an_intervening_release() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let enabled = cx.signal(true);
        let clicks = cx.signal(0);
        state = Some((enabled, clicks));
        button("Run")
            .enabled(enabled)
            .on_press(move || clicks.update(|n| *n += 1))
    });
    let (enabled, clicks) = state.unwrap();
    let renderer = renderer();
    root.layout(&renderer, Size::new(320.0, 120.0));
    let at = Some(Point::new(15.0, 15.0));
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    let _ = root.handle(&press, at, Instant::now());
    let _ = enabled.set(false);
    root.layout(&renderer, Size::new(320.0, 120.0));
    let _ = enabled.set(true);
    root.layout(&renderer, Size::new(320.0, 120.0));
    assert!(!root.handle(&release, at, Instant::now()).captured);
    assert_eq!(clicks.get(), 0);
    let _ = root.handle(&press, at, Instant::now());
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn native_toggle_handles_touch_ownership_cancellation_and_disabled_state() {
    use taim_core::{pointer, touch};
    for use_switch in [false, true] {
        let mut state = None;
        let mut root = Root::new(|cx| {
            let value = cx.signal(false);
            let enabled = cx.signal(true);
            state = Some((value, enabled));
            if use_switch {
                switch("Choice", value)
            } else {
                checkbox("Choice", value)
            }
            .enabled(enabled)
        });
        let (value, enabled) = state.unwrap();
        let renderer = renderer();
        let size = Size::new(320.0, 120.0);
        root.layout(&renderer, size);
        let at = Point::new(10.0, 10.0);
        let metadata = pointer::Metadata::NONE;
        let first = touch::Finger(1);
        let down = Event::Touch(touch::Event::FingerPressed {
            id: first,
            position: at,
            metadata,
        });
        let up = Event::Touch(touch::Event::FingerLifted {
            id: first,
            position: at,
            metadata,
        });
        let _ = root.handle(&down, None, Instant::now());
        let _ = root.handle(
            &Event::Touch(touch::Event::FingerLost {
                id: touch::Finger(2),
                position: at,
                metadata,
            }),
            None,
            Instant::now(),
        );
        let _ = root.handle(&up, None, Instant::now());
        assert!(value.get());
        let _ = root.handle(&down, None, Instant::now());
        let _ = root.handle(
            &Event::Touch(touch::Event::FingerLost {
                id: first,
                position: at,
                metadata,
            }),
            None,
            Instant::now(),
        );
        let _ = root.handle(&up, None, Instant::now());
        assert!(value.get(), "cancelled touch must not toggle");
        let _ = root.handle(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Some(at),
            Instant::now(),
        );
        let _ = root.handle(
            &Event::Window(taim_core::window::Event::Unfocused),
            None,
            Instant::now(),
        );
        let _ = root.handle(
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Some(at),
            Instant::now(),
        );
        assert!(value.get(), "focus loss must cancel a mouse press");
        let _ = root.handle(&down, None, Instant::now());
        let _ = enabled.set(false);
        root.layout(&renderer, size);
        let semantics = root.describe(size);
        assert!(semantics.children[0].actions.is_empty());
        let _ = enabled.set(true);
        root.layout(&renderer, size);
        let _ = root.handle(&up, None, Instant::now());
        assert!(
            value.get(),
            "disable/enable must not resurrect the old press"
        );
    }
}

#[test]
fn native_slider_drag_only_accepts_its_owner_and_ignores_invalid_motion() {
    use taim_core::{pointer, touch};
    let mut state = None;
    let mut root = Root::new(|cx| {
        let value = cx.signal(0.0_f32);
        state = Some(value);
        slider(0.0..=10.0, value)
    });
    let value = state.unwrap();
    root.layout(&renderer(), Size::new(200.0, 60.0));
    let metadata = pointer::Metadata::NONE;
    let owner = touch::Finger(1);
    let event = |id, x| {
        Event::Touch(touch::Event::FingerMoved {
            id,
            position: Point::new(x, 8.0),
            metadata,
        })
    };
    let down = Event::Touch(touch::Event::FingerPressed {
        id: owner,
        position: Point::new(8.0, 8.0),
        metadata,
    });
    let _ = root.handle(&down, None, Instant::now());
    let _ = root.handle(&event(owner, 192.0), None, Instant::now());
    assert_eq!(value.get(), 10.0);
    let _ = root.handle(&event(touch::Finger(2), 8.0), None, Instant::now());
    let _ = root.handle(&event(owner, f32::NAN), None, Instant::now());
    assert_eq!(value.get(), 10.0);
    let _ = root.handle(&event(owner, 100.0), None, Instant::now());
    assert_eq!(value.get(), 5.0);
    let _ = root.handle(
        &Event::Touch(touch::Event::FingerLost {
            id: owner,
            position: Point::new(100.0, 8.0),
            metadata,
        }),
        None,
        Instant::now(),
    );
    let _ = root.handle(&event(owner, 192.0), None, Instant::now());
    assert_eq!(value.get(), 5.0, "cancelled contact cannot keep dragging");
}

#[test]
fn touch_focuses_native_text_controls_and_blur_cancels_selection_drag() {
    use taim_core::{pointer, touch};
    for multiline in [false, true] {
        let mut state = None;
        let mut buffer = None;
        let mut root = Root::new(|cx| {
            let view: taim::view::AnyView = if multiline {
                let value = Buffer::new(cx, "text with several words");
                buffer = Some(value);
                text_editor(&value).into()
            } else {
                let value = cx.signal("text with several words".to_string());
                state = Some(value);
                text_field("", value).into()
            };
            view
        });
        root.layout(&renderer(), Size::new(320.0, 120.0));
        let metadata = pointer::Metadata::NONE;
        let owner = touch::Finger(1);
        let down = Event::Touch(touch::Event::FingerPressed {
            id: owner,
            position: Point::new(15.0, 15.0),
            metadata,
        });
        assert!(root.handle(&down, None, Instant::now()).captured);
        assert!(root.input_method().is_enabled());
        let initial = root.input_method();
        let wrong_move = Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(2),
            position: Point::new(250.0, 15.0),
            metadata,
        });
        let _ = root.handle(&wrong_move, None, Instant::now());
        assert_eq!(root.input_method(), initial);
        let _ = root.handle(
            &Event::Window(taim_core::window::Event::Unfocused),
            None,
            Instant::now(),
        );
        let _ = root.handle(
            &Event::Touch(touch::Event::FingerMoved {
                id: owner,
                position: Point::new(250.0, 15.0),
                metadata,
            }),
            None,
            Instant::now(),
        );
        assert_eq!(
            root.input_method(),
            initial,
            "old contact must not extend selection after blur"
        );
        assert!(ime(&mut root, input_method::Event::Commit("日".into())).captured);
        let text = buffer.map_or_else(|| state.unwrap().get(), |value| value.text());
        assert!(text.contains('日'));
    }
}

#[test]
fn modal_dialog_traps_keyboard_focus_and_restores_the_previous_control() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let open = cx.signal(false);
        let clicks = cx.signal(0);
        state = Some((open, clicks));
        column(views![
            button("Background").on_press(move || clicks.update(|n| *n += 1)),
            dialog(
                open,
                button("Inside").on_press(move || clicks.update(|n| *n += 10))
            ),
        ])
    });
    let (open, clicks) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(640.0, 480.0);
    root.layout(&renderer, size);
    let tab = named_key(keyboard::key::Named::Tab, keyboard::Modifiers::NONE);
    let enter = named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE);
    let _ = root.handle(&tab, None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 1);
    let _ = open.set(true);
    root.layout(&renderer, size);
    let _ = root.handle(&enter, None, Instant::now());
    let _ = root.handle(&tab, None, Instant::now());
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 21, "Tab must remain inside the modal subtree");
    assert!(
        root.handle(
            &named_key(keyboard::key::Named::Escape, keyboard::Modifiers::NONE),
            None,
            Instant::now()
        )
        .captured
    );
    assert!(!open.get());
    root.layout(&renderer, size);
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(
        clicks.get(),
        22,
        "closing restores the background control's identity"
    );
}

#[test]
fn nested_modal_focus_restores_one_scope_at_a_time() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let outer = cx.signal(false);
        let inner = cx.signal(false);
        let clicks = cx.signal(0);
        state = Some((outer, inner, clicks));
        column(views![
            button("Background").on_press(move || clicks.update(|n| *n += 1)),
            dialog(
                outer,
                column(views![
                    button("Outer").on_press(move || clicks.update(|n| *n += 10)),
                    dialog(
                        inner,
                        button("Inner").on_press(move || clicks.update(|n| *n += 100))
                    ),
                ])
            ),
        ])
    });
    let (outer, inner, clicks) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(800.0, 800.0);
    root.layout(&renderer, size);
    let enter = named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE);
    let _ = root.handle(
        &named_key(keyboard::key::Named::Tab, keyboard::Modifiers::NONE),
        None,
        Instant::now(),
    );
    let _ = outer.set(true);
    root.layout(&renderer, size);
    let _ = inner.set(true);
    root.layout(&renderer, size);
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 100);
    let _ = inner.set(false);
    root.layout(&renderer, size);
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 110);
    let _ = outer.set(false);
    root.layout(&renderer, size);
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 111);
}

#[test]
fn modal_without_focusable_children_still_blocks_background_and_accepts_escape() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let open = cx.signal(false);
        let clicks = cx.signal(0);
        state = Some((open, clicks));
        column(views![
            button("Background").on_press(move || clicks.update(|n| *n += 1)),
            dialog(open, text("Notice"))
        ])
    });
    let (open, clicks) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(640.0, 480.0);
    root.layout(&renderer, size);
    let _ = root.handle(
        &named_key(keyboard::key::Named::Tab, keyboard::Modifiers::NONE),
        None,
        Instant::now(),
    );
    let _ = open.set(true);
    root.layout(&renderer, size);
    let enter = named_key(keyboard::key::Named::Enter, keyboard::Modifiers::NONE);
    assert!(!root.handle(&enter, None, Instant::now()).captured);
    assert_eq!(clicks.get(), 0);
    assert!(
        root.handle(
            &named_key(keyboard::key::Named::Escape, keyboard::Modifiers::NONE),
            None,
            Instant::now()
        )
        .captured
    );
    root.layout(&renderer, size);
    let _ = root.handle(&enter, None, Instant::now());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn semantic_actions_revalidate_live_controls_and_obey_the_active_modal() {
    use taim_core::semantics::{Action, Request};
    let mut state = None;
    let mut root = Root::new(|cx| {
        let open = cx.signal(false);
        let enabled = cx.signal(true);
        let clicks = cx.signal(0);
        state = Some((open, enabled, clicks));
        column(views![
            button("Background")
                .enabled(enabled)
                .on_press(move || clicks.update(|n| *n += 1)),
            dialog(open, text("Notice"))
        ])
    });
    let (open, enabled, clicks) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(640.0, 480.0);
    root.layout(&renderer, size);
    let background = root.describe(size).children[0].id.clone();
    let activate = Action {
        target: background.clone(),
        request: Request::Activate,
    };
    assert!(
        root.accessibility_action(&activate, Instant::now())
            .captured
    );
    assert_eq!(clicks.get(), 1);
    let _ = enabled.set(false);
    assert!(
        !root
            .accessibility_action(&activate, Instant::now())
            .captured,
        "a stale enabled snapshot cannot authorize activation"
    );
    let _ = enabled.set(true);
    let _ = open.set(true);
    root.layout(&renderer, size);
    let visible = root.describe(size);
    assert_ne!(visible.children[0].id, background);
    assert!(
        !root
            .accessibility_action(&activate, Instant::now())
            .captured
    );
    let dismiss = Action {
        target: visible.children[0].id.clone(),
        request: Request::Dismiss,
    };
    assert!(root.accessibility_action(&dismiss, Instant::now()).captured);
    root.layout(&renderer, size);
    assert!(
        !root.accessibility_action(&dismiss, Instant::now()).captured,
        "retired modal identity must be rejected"
    );
    assert!(
        root.accessibility_action(&activate, Instant::now())
            .captured
    );
    assert_eq!(clicks.get(), 2);
}

fn named_node<'a>(
    node: &'a taim_core::semantics::Node,
    name: &str,
) -> Option<&'a taim_core::semantics::Node> {
    if node.name.as_deref() == Some(name) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| named_node(child, name))
}

#[test]
fn closing_and_reopening_a_retained_modal_cannot_resurrect_a_pending_click() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let open = cx.signal(true);
        let clicks = cx.signal(0);
        state = Some((open, clicks));
        dialog(
            open,
            button("Inside").on_press(move || clicks.update(|n| *n += 1)),
        )
    });
    let (open, clicks) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(640.0, 480.0);
    root.layout(&renderer, size);
    let bounds = named_node(&root.describe(size), "Inside").unwrap().bounds;
    let at = Some(bounds.center());
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    let _ = root.handle(&press, at, Instant::now());
    let _ = open.set(false);
    root.layout(&renderer, size);
    let _ = open.set(true);
    root.layout(&renderer, size);
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 0);
    let _ = root.handle(&press, at, Instant::now());
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn opening_a_modal_cancels_pending_background_clicks() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let open = cx.signal(false);
        let clicks = cx.signal(0);
        state = Some((open, clicks));
        column(views![
            button("Background").on_press(move || clicks.update(|n| *n += 1)),
            dialog(open, text("Notice"))
        ])
    });
    let (open, clicks) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(640.0, 480.0);
    root.layout(&renderer, size);
    let at = Some(
        named_node(&root.describe(size), "Background")
            .unwrap()
            .bounds
            .center(),
    );
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    let _ = root.handle(&press, at, Instant::now());
    let _ = open.set(true);
    root.layout(&renderer, size);
    let _ = open.set(false);
    root.layout(&renderer, size);
    let _ = root.handle(&release, at, Instant::now());
    assert_eq!(clicks.get(), 0);
}

#[test]
fn native_semantic_identities_remain_stable_across_repeated_publication() {
    let mut root = Root::new(|cx| {
        let selected = cx.signal(0_usize);
        let opened = cx.signal(Some(0_usize));
        let refreshing = cx.signal(false);
        column(views![
            tabs(["One", "Two"], selected),
            avatar("Taim User"),
            badge(text("Messages"), 3_u32),
            skeleton(100.0, 20.0),
            tooltip(text("Tip base"), "Help"),
            accordion([("Section", text("Details").into())], opened),
            carousel([text("Page").into()], selected),
            pull_to_refresh(text("Refresh content"), refreshing),
        ])
    });
    let size = Size::new(640.0, 1200.0);
    root.layout(&renderer(), size);
    let at = named_node(&root.describe(size), "Tip base")
        .unwrap()
        .bounds
        .center();
    let _ = root.handle(
        &Event::Mouse(mouse::Event::CursorMoved { position: at }),
        Some(at),
        Instant::now(),
    );
    assert_eq!(
        root.describe(size),
        root.describe(size),
        "unchanged controls must not look removed/re-added to a screen reader"
    );
}

#[test]
fn semantic_activation_executes_toggle_radio_and_dropdown_behaviors() {
    use taim_core::semantics::{Action, Request, State};
    let mut state = None;
    let mut root = Root::new(|cx| {
        let checked = cx.signal(false);
        let selected = cx.signal(0_usize);
        let option = cx.signal(Some(0_usize));
        state = Some((checked, selected));
        column(views![
            checkbox("Toggle", checked),
            radio("Choice", 1_usize, selected),
            dropdown(["One", "Two"], option)
        ])
    });
    let (checked, selected) = state.unwrap();
    let renderer = renderer();
    let size = Size::new(640.0, 480.0);
    root.layout(&renderer, size);
    for name in ["Toggle", "Choice", "One"] {
        let id = named_node(&root.describe(size), name).unwrap().id.clone();
        assert!(
            root.accessibility_action(
                &Action {
                    target: id,
                    request: Request::Activate
                },
                Instant::now()
            )
            .captured
        );
    }
    assert!(checked.get());
    assert_eq!(selected.get(), 1);
    assert!(
        named_node(&root.describe(size), "One")
            .unwrap()
            .state
            .contains(State::EXPANDED)
    );
}

#[test]
fn native_editor_snapshot_keeps_composition_uncommitted() {
    let mut signal = None;
    let mut root = Root::new(|cx| {
        let state = cx.signal(String::from("a"));
        signal = Some(state);
        text_field("", state)
    });
    let value = signal.unwrap();
    let renderer = renderer();
    focus(&mut root, &renderer);
    let snapshot = input_method::EditorState {
        content: "aनमस्ते🌍".into(),
        selection: 23..23,
        composing: Some(1..19),
        multiline: false,
    }
    .normalized();
    let _ = ime(&mut root, input_method::Event::Edit(snapshot.clone()));
    assert_eq!(value.get(), "a");
    let _ = ime(
        &mut root,
        input_method::Event::Edit(input_method::EditorState {
            composing: None,
            ..snapshot
        }),
    );
    assert_eq!(value.get(), "aनमस्ते🌍");
    let (_, state) = root.editor_state().expect("focused editor");
    assert_eq!(state.content, value.get());
    assert_eq!(state.selection, state.content.len()..state.content.len());
}

#[test]
fn native_multiline_snapshot_preserves_utf8_selection_across_lines() {
    let mut handle = None;
    let mut root = Root::new(|cx| {
        let buffer = Buffer::new(cx, "old");
        handle = Some(buffer);
        text_editor(&buffer)
    });
    focus(&mut root, &renderer());
    let content = "a🌍\nनमस्ते";
    let _ = ime(
        &mut root,
        input_method::Event::Edit(input_method::EditorState {
            content: content.into(),
            selection: 6..content.len(),
            composing: None,
            multiline: true,
        }),
    );
    let buffer = handle.unwrap();
    assert_eq!(buffer.text(), content);
    assert_eq!(buffer.selected_text().as_deref(), Some("नमस्ते"));
    let (_, snapshot) = root.editor_state().unwrap();
    assert_eq!(snapshot.selection, 6..content.len());
}

#[test]
fn native_password_context_exposes_identity_without_content_or_length() {
    let mut root = Root::new(|cx| {
        let value = cx.signal(String::from("private🌍"));
        text_field("", value).secret()
    });
    focus(&mut root, &renderer());
    let identity = root
        .focused_identity()
        .expect("password has stable focus identity");
    assert!(root.editor_state().is_none());
    let _ = ime(&mut root, input_method::Event::Commit("new".into()));
    assert_eq!(root.focused_identity(), Some(identity));
    assert!(root.editor_state().is_none());
}

#[test]
fn redacted_native_deletion_distinguishes_utf16_and_unicode_scalars() {
    for (content, before, codepoints, expected) in [
        ("a🌍", 2, false, "a"),
        ("a🌍", 2, true, ""),
        ("a🌍", 1, false, "a"),
        ("a\u{301}", 1, true, "a"),
    ] {
        let mut signal = None;
        let mut root = Root::new(|cx| {
            let value = cx.signal(String::new());
            signal = Some(value);
            text_field("", value).secret()
        });
        focus(&mut root, &renderer());
        let _ = ime(&mut root, input_method::Event::Commit(content.into()));
        let _ = ime(
            &mut root,
            input_method::Event::DeleteSurrounding {
                before,
                after: 0,
                codepoints,
            },
        );
        assert_eq!(signal.unwrap().get(), expected);
        assert!(root.editor_state().is_none());
    }
}

#[test]
fn native_editor_focus_targets_the_semantic_node_identity() {
    let mut root = Root::new(|cx| text_field("Name", cx.signal(String::new())));
    let renderer = renderer();
    focus(&mut root, &renderer);
    let tree = root.describe(Size::new(320.0, 120.0));
    fn editor(node: &taim_core::semantics::Node) -> Option<taim_core::widget::Id> {
        if node.role == taim_core::semantics::Role::TextField {
            Some(node.id.clone())
        } else {
            node.children.iter().find_map(editor)
        }
    }
    let identity = editor(&tree).expect("editor semantics");
    assert_eq!(root.focused_semantic_identity(), Some(identity.clone()));
    assert_ne!(root.focused_identity(), Some(identity));
}

#[test]
fn versioned_restoration_survives_new_root_and_excludes_unregistered_secrets() {
    let mut count = None;
    let root = Root::new(|cx| {
        let value = cx.restorable("count", 2, 0_u64).unwrap();
        count = Some(value);
        let secret = cx.signal("not-a-checkpoint-secret".to_owned());
        column(views![
            text(move || value.get().to_string()),
            text_field("Password", secret).secret()
        ])
    });
    let _ = count.unwrap().set(37);
    let snapshot = root.restoration_snapshot().unwrap();
    assert!(!snapshot.contains("not-a-checkpoint-secret"));
    drop(root);
    let mut restored = None;
    let _root = Root::new_restored(&snapshot, |cx| {
        let value = cx.restorable("count", 2, 9_u64).unwrap();
        restored = Some(value);
        text(move || value.get().to_string())
    });
    assert_eq!(restored.unwrap().get(), 37);
}

#[test]
fn restoration_schema_changes_and_malformed_checkpoints_use_initial_values() {
    for snapshot in [
        "not-json",
        r#"{"count":{"version":1,"value":777}}"#,
        r#"{"count":{"version":2,"value":"wrong-type"}}"#,
    ] {
        let mut value = None;
        let _root = Root::new_restored(snapshot, |cx| {
            let state = cx.restorable("count", 2, 5_u64).unwrap();
            value = Some(state);
            assert!(cx.restorable("count", 2, 0_u64).is_err());
            text(move || state.get().to_string())
        });
        assert_eq!(value.unwrap().get(), 5);
    }
}

#[test]
fn back_navigation_revalidates_enabled_state_before_commit() {
    let mut handle = None;
    let mut root = Root::new(|cx| {
        let enabled = cx.signal(true);
        let calls = cx.signal(0_u32);
        handle = Some((enabled, calls));
        cx.on_back(enabled, move || {
            calls.update(|value| *value += 1);
            let _ = enabled.set(false);
        });
        text("Page")
    });
    assert!(root.handles_back());
    assert!(root.back(Instant::now()).captured);
    assert!(!root.handles_back());
    assert!(!root.back(Instant::now()).captured);
    assert_eq!(handle.unwrap().1.get(), 1);
}

#[test]
fn android_slots_keep_identity_update_properties_and_hide_under_modals() {
    use serde_json::json;
    let mut signals = None;
    let mut root = Root::new(|cx| {
        let open = cx.signal(false);
        let enabled = cx.signal(true);
        let label = cx.signal(String::from("first"));
        signals = Some((open, enabled, label));
        column(views![
            android_view("fixture.controls", move || json!({"label":label.get()}))
                .unwrap()
                .width(100.0)
                .height(60.0)
                .enabled(enabled),
            dialog(open, text("Modal")),
        ])
        .padding(10.0)
    });
    let renderer = renderer();
    root.layout(&renderer, Size::new(320.0, 240.0));
    let first = root.native_views().remove(0);
    assert_eq!(
        first.bounds,
        Rectangle::new(Point::new(10.0, 10.0), Size::new(100.0, 60.0))
    );
    assert_eq!(first.visible, Some(first.bounds));
    let (open, enabled, label) = signals.unwrap();
    let _ = enabled.set(false);
    let _ = label.set("updated".into());
    root.layout(&renderer, Size::new(320.0, 240.0));
    let changed = root.native_views().remove(0);
    assert_eq!(changed.view.id, first.view.id);
    assert!(!changed.view.enabled);
    assert_eq!(changed.view.properties, json!({"label":"updated"}));
    let _ = open.set(true);
    root.layout(&renderer, Size::new(320.0, 240.0));
    assert!(root.native_views()[0].visible.is_none());
    let _ = open.set(false);
    root.layout(&renderer, Size::new(320.0, 240.0));
    assert!(root.native_views()[0].visible.is_some());
}

#[test]
fn native_input_handoff_discards_preedit_and_releases_taim_editor() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let value = cx.signal(String::new());
        state = Some(value);
        text_field("", value)
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let _ = ime(
        &mut root,
        input_method::Event::Preedit("日本".into(), Some(0..6)),
    );
    assert!(root.release_focus(Instant::now()).paint);
    assert_eq!(root.input_method(), InputMethod::Disabled);
    assert_eq!(state.unwrap().get(), "");
}

#[test]
fn android_factory_names_and_properties_fail_closed() {
    use serde_json::json;
    assert!(android_view("", json!({})).is_err());
    assert!(android_view("bad/name", json!({})).is_err());
    assert!(android_view("valid.name", json!([])).is_err());
    assert!(android_view("valid.name", json!({"data":"x".repeat(16384)})).is_err());
    let mut state = None;
    let mut root = Root::new(|cx| {
        let props = cx.signal(json!({}));
        state = Some(props);
        android_view("valid.name", props).unwrap()
    });
    let _ = state.unwrap().set(json!(null));
    assert!(root.native_views()[0].view.properties.is_null());
}

#[test]
fn android_slot_bounds_preserve_full_frame_and_inherit_scroll_clip() {
    let mut root = Root::new(|_| {
        scroll(
            android_view("fixture.controls", serde_json::json!({}))
                .unwrap()
                .width(100.0)
                .height(200.0),
        )
    });
    root.layout(&renderer(), Size::new(320.0, 50.0));
    let slot = root.native_views().remove(0);
    assert_eq!(slot.bounds.height, 200.0);
    assert_eq!(slot.visible.unwrap().height, 50.0);
    assert_eq!(slot.visible.unwrap().width, 100.0);
}

#[test]
fn semantic_scroll_bounds_hide_fully_clipped_descendants() {
    let mut root = Root::new(|_| {
        scroll(column(views![
            button("Partly visible").label_view(space().width(100.0).height(80.0)),
            button("Below viewport")
        ]))
    });
    let size = Size::new(320.0, 50.0);
    root.layout(&renderer(), size);
    let tree = taim_accessibility::Tree::from_forest(&[root.describe(size)]);
    let entries = tree.in_order();
    let visible = entries
        .iter()
        .find(|e| e.name.as_deref() == Some("Partly visible"))
        .unwrap();
    assert!(visible.bounds.height <= 50.0);
    let clipped = entries
        .iter()
        .find(|e| e.name.as_deref() == Some("Below viewport"))
        .unwrap();
    assert!(clipped.state.contains(taim::semantics::State::HIDDEN));
}

fn request_paste(root: &mut Root) -> taim::view::PasteToken {
    let key = keyboard::Key::Character("v".into());
    let response = root.handle(
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        }),
        None,
        Instant::now(),
    );
    assert!(response.paste);
    root.begin_paste().expect("focused paste destination")
}
fn clipboard_text(root: &mut Root, text: &str) {
    let _ = root.handle(
        &Event::Clipboard(taim_core::clipboard::Event::Read(Ok(std::sync::Arc::new(
            taim_core::clipboard::Content::Text(text.into()),
        )))),
        None,
        Instant::now(),
    );
}

#[test]
fn delayed_clipboard_cannot_satisfy_a_new_editors_pending_paste() {
    let mut states = None;
    let mut root = Root::new(|cx| {
        let a = cx.signal(String::new());
        let b = cx.signal(String::new());
        states = Some((a, b));
        column(views![text_field("First", a), text_field("Second", b)])
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let old = request_paste(&mut root);
    let tree = taim_accessibility::Tree::from_forest(&[root.describe(Size::new(320.0, 120.0))]);
    let second = tree
        .in_order()
        .into_iter()
        .find(|e| e.name.as_deref() == Some("Second"))
        .unwrap()
        .id
        .clone();
    assert!(
        root.accessibility_action_with_renderer(
            &renderer,
            &taim::semantics::Action {
                target: second,
                request: taim::semantics::Request::Focus
            },
            Instant::now()
        )
        .captured
    );
    let current = request_paste(&mut root);
    assert!(!root.accept_paste(&old));
    assert!(root.accept_paste(&current));
    clipboard_text(&mut root, "current");
    let (a, b) = states.unwrap();
    assert_eq!(a.get(), "");
    assert_eq!(b.get(), "current");
    assert!(!root.accept_paste(&current));
}

#[test]
fn clipboard_lease_expires_on_typing_external_edits_and_modal_round_trip() {
    let mut signals = None;
    let mut root = Root::new(|cx| {
        let value = cx.signal(String::new());
        let open = cx.signal(false);
        signals = Some((value, open));
        column(views![text_field("", value), dialog(open, text("Modal"))])
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let (value, open) = signals.unwrap();
    let old = request_paste(&mut root);
    let _ = ime(&mut root, input_method::Event::Commit("typed".into()));
    assert!(!root.accept_paste(&old));
    let external = request_paste(&mut root);
    let _ = value.set("external".into());
    assert!(!root.accept_paste(&external));
    let modal = request_paste(&mut root);
    let _ = open.set(true);
    root.layout(&renderer, Size::new(320.0, 120.0));
    let _ = open.set(false);
    root.layout(&renderer, Size::new(320.0, 120.0));
    assert!(!root.accept_paste(&modal));
    let lost = request_paste(&mut root);
    let _ = root.release_focus(Instant::now());
    focus(&mut root, &renderer);
    assert!(!root.accept_paste(&lost));
}

#[test]
fn paste_tokens_are_root_scoped_and_superseded_reads_do_not_cancel_latest() {
    let mut a = Root::new(|cx| text_field("", cx.signal(String::new())));
    let mut b = Root::new(|cx| text_field("", cx.signal(String::new())));
    let renderer = renderer();
    focus(&mut a, &renderer);
    focus(&mut b, &renderer);
    let a_token = request_paste(&mut a);
    let b_old = request_paste(&mut b);
    let b_current = request_paste(&mut b);
    assert!(!b.accept_paste(&a_token));
    assert!(!b.accept_paste(&b_old));
    assert!(b.accept_paste(&b_current));
    assert!(!b.accept_paste(&b_current));
}

#[test]
fn clipboard_error_ends_textfield_and_multiline_waiting_state() {
    let mut text = None;
    let mut root = Root::new(|cx| {
        let state = cx.signal(String::new());
        text = Some(state);
        text_field("", state)
    });
    let renderer = renderer();
    focus(&mut root, &renderer);
    let token = request_paste(&mut root);
    assert!(root.accept_paste(&token));
    let _ = root.handle(
        &Event::Clipboard(taim_core::clipboard::Event::Read(Err(
            taim_core::clipboard::Error::ClipboardUnavailable,
        ))),
        None,
        Instant::now(),
    );
    clipboard_text(&mut root, "unsolicited");
    assert_eq!(text.unwrap().get(), "");
    let mut buffer = None;
    let mut editor = Root::new(|cx| {
        let value = Buffer::new(cx, "");
        buffer = Some(value);
        text_editor(&value)
    });
    focus(&mut editor, &renderer);
    let token = request_paste(&mut editor);
    assert!(editor.accept_paste(&token));
    let _ = editor.handle(
        &Event::Clipboard(taim_core::clipboard::Event::Read(Err(
            taim_core::clipboard::Error::ClipboardUnavailable,
        ))),
        None,
        Instant::now(),
    );
    clipboard_text(&mut editor, "unsolicited");
    assert_eq!(buffer.unwrap().text(), "");
}

#[test]
fn secure_paste_tokens_do_not_contain_secret_and_expire_on_secret_edits() {
    let mut root =
        Root::new(|cx| text_field("Secret", cx.signal("private-fixture".to_owned())).secret());
    let renderer = renderer();
    focus(&mut root, &renderer);
    let token = request_paste(&mut root);
    assert!(!format!("{token:?}").contains("private-fixture"));
    let _ = ime(&mut root, input_method::Event::Commit("new".into()));
    assert!(!root.accept_paste(&token));
}
