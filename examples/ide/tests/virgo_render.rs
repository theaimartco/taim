use std::time::Instant;
use taim::advanced::renderer::Headless;
use taim::{Color, Rectangle, Size};
use taim_core::renderer::Renderer as _;
use taim_renderer::Renderer;
use taim_runtime::user_interface::{Cache, UserInterface};

#[test]
fn workspace_first_frame_contains_visible_ui() {
    let (studio, _) = taim_ide::Studio::with_workspace(std::env::current_dir().unwrap()).unwrap();
    let _subscription = studio.subscription();
    let theme = studio.theme();
    let base = taim_core::theme::Base::base(&theme);
    let size = Size::new(1200_u32, 800_u32);
    let logical = Size::new(size.width as f32, size.height as f32);
    let backend = std::env::var("VIRGO_RENDER_BACKEND").unwrap_or_else(|_| "tiny-skia".to_string());
    let mut renderer = taim_futures::futures::executor::block_on(<Renderer as Headless>::new(
        taim_core::renderer::Settings::default(),
        Some(&backend),
    ))
    .unwrap();
    renderer.reset(Rectangle::with_size(logical));
    let mut ui = UserInterface::build(studio.view(), logical, Cache::new(), &mut renderer);
    ui.draw(
        &mut renderer,
        &theme,
        &taim_core::renderer::Style {
            text_color: base.text_color,
        },
        taim_core::mouse::Cursor::Unavailable,
    );
    let pixels = renderer.screenshot(size, 1.0, Color::BLACK);
    if let Ok(path) = std::env::var("VIRGO_RENDER_DUMP") {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, size.width, size.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    }
    let lit = pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 70 || pixel[1] > 70 || pixel[2] > 70)
        .count();
    assert!(
        lit > 500,
        "workspace first frame has only {lit} visible pixels"
    );
    let bright_region = |left: usize, top: usize, right: usize, bottom: usize| {
        (top..bottom)
            .flat_map(|y| (left..right).map(move |x| (y * size.width as usize + x) * 4))
            .filter(|&index| pixels[index] > 70)
            .count()
    };
    assert!(
        bright_region(302, 98, 346, 545) > 60,
        "editor line numbers are missing"
    );
    assert!(
        bright_region(1186, 95, 1197, 250) > 100,
        "editor scrollbar is missing"
    );
    assert!(
        bright_region(1133, 98, 1180, 545) > 80,
        "editor minimap is missing"
    );
}

#[test]
fn editor_scrollbar_moves_a_long_document() {
    let root = std::env::temp_dir().join(format!("virgo-editor-scroll-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let content = (1..=300)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    std::fs::write(root.join("long.rs"), content).unwrap();
    let (mut studio, _) = taim_ide::Studio::with_workspace(root.clone()).unwrap();
    let theme = studio.theme();
    let base = taim_core::theme::Base::base(&theme);
    let logical = Size::new(1200.0, 800.0);
    let mut renderer = taim_futures::futures::executor::block_on(<Renderer as Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .unwrap();
    renderer.reset(Rectangle::with_size(logical));
    let mut ui = UserInterface::build(studio.view(), logical, Cache::new(), &mut renderer);
    ui.draw(
        &mut renderer,
        &theme,
        &taim_core::renderer::Style {
            text_color: base.text_color,
        },
        taim_core::mouse::Cursor::Unavailable,
    );
    let cache = ui.into_cache();
    let _ = studio.update(taim_ide::Message::EditorScrollToLine(0, 140));
    let (first_line, _) = studio.docs.documents[0].content.scroll_position();
    assert!(
        first_line >= 100,
        "scrollbar action did not move the viewport: {first_line}"
    );
    renderer.reset(Rectangle::with_size(logical));
    let mut ui = UserInterface::build(studio.view(), logical, cache, &mut renderer);
    ui.draw(
        &mut renderer,
        &theme,
        &taim_core::renderer::Style {
            text_color: base.text_color,
        },
        taim_core::mouse::Cursor::Unavailable,
    );
    let pixels = renderer.screenshot(Size::new(1200, 800), 1.0, Color::BLACK);
    let lit = |left: usize, right: usize| {
        (98..120)
            .flat_map(|y| (left..right).map(move |x| (y * 1200 + x) * 4))
            .filter(|&i| pixels[i] > 70 || pixels[i + 1] > 70 || pixels[i + 2] > 70)
            .count()
    };
    assert!(
        lit(325, 345) > 5,
        "scrolled gutter lost its first visible row"
    );
    assert!(lit(355, 440) > 20, "scrolled text left its gutter row");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn short_document_stays_at_gutter_origin_after_pane_changes() {
    let root = std::env::temp_dir().join(format!("virgo-editor-origin-{}", std::process::id()));
    std::fs::create_dir_all(root.join(".cargo")).unwrap();
    std::fs::write(root.join("a.rs"), "fn main() {}\n".repeat(100)).unwrap();
    std::fs::write(
        root.join(".cargo/audit.toml"),
        "[advisories]\nignore = []\n",
    )
    .unwrap();
    let (mut studio, _) = taim_ide::Studio::with_workspace(root.clone()).unwrap();
    let short_id = studio
        .tree
        .root
        .find_by_path(".cargo/audit.toml")
        .unwrap()
        .id;
    let long_id = studio.tree.root.find_by_path("a.rs").unwrap().id;
    let theme = studio.theme();
    let base = taim_core::theme::Base::base(&theme);
    let size = Size::new(1200_u32, 800_u32);
    let logical = Size::new(size.width as f32, size.height as f32);
    let mut renderer = taim_futures::futures::executor::block_on(<Renderer as Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .unwrap();
    let mut cache = Cache::new();
    let mut terminal_expanded = false;
    for (case_index, (panel, split, expanded)) in [
        (true, false, false),
        (false, false, false),
        (true, true, false),
        (true, true, true),
    ]
    .into_iter()
    .enumerate()
    {
        let _ = studio.update(taim_ide::Message::FileSelected(long_id));
        if studio.show_terminal != panel {
            let _ = studio.update(taim_ide::Message::ToggleTerminal);
        }
        if studio.docs.split_tab.is_some() != split {
            let _ = studio.update(taim_ide::Message::ToggleSplitEditor);
        }
        if terminal_expanded != expanded {
            let _ = studio.update(taim_ide::Message::ToggleTerminalMaximize);
            terminal_expanded = expanded;
        }
        let ui = UserInterface::build(studio.view(), logical, cache, &mut renderer);
        cache = ui.into_cache();
        let _ = studio.update(taim_ide::Message::FileSelected(short_id));
        renderer.reset(Rectangle::with_size(logical));
        let mut ui = UserInterface::build(studio.view(), logical, cache, &mut renderer);
        ui.draw(
            &mut renderer,
            &theme,
            &taim_core::renderer::Style {
                text_color: base.text_color,
            },
            taim_core::mouse::Cursor::Unavailable,
        );
        let pixels = renderer.screenshot(size, 1.0, Color::BLACK);
        let lit = (96..116)
            .flat_map(|y| (355..475).map(move |x| (y * size.width as usize + x) * 4))
            .filter(|&i| pixels[i] > 70 || pixels[i + 1] > 70 || pixels[i + 2] > 70)
            .count();
        assert!(
            lit > 25,
            "short file line 1 is not at editor top: panel={panel}, split={split}, expanded={expanded}, pixels={lit}"
        );
        let mut messages = Vec::new();
        let _ = ui.update(
            &taim_core::window::Headless,
            &taim_core::shell::Waker::noop(),
            &[taim_core::Event::Mouse(
                taim_core::mouse::Event::ButtonPressed(taim_core::mouse::Button::Left),
            )],
            taim_core::mouse::Cursor::Available(taim::Point::new(
                358.0 + case_index as f32 * 50.0,
                104.0,
            )),
            &mut renderer,
            &mut messages,
        );
        assert!(
            messages.iter().any(|message| matches!(
                message,
                taim_ide::Message::EditorAction(taim::widget::text_editor::Action::Click(_))
            )),
            "clicking the drawn first line did not reach the editor: panel={panel}, split={split}, expanded={expanded}, messages={messages:?}"
        );
        cache = ui.into_cache();
        for message in messages {
            let _ = studio.update(message);
        }
        assert_eq!(
            studio.docs.documents[studio.docs.active_tab]
                .content
                .cursor()
                .position
                .line,
            0
        );
    }
    let _ = studio.update(taim_ide::Message::ToggleSplitEditor);
    let _ = studio.update(taim_ide::Message::TabClosed(studio.docs.active_tab));
    let _ = studio.update(taim_ide::Message::FileSelected(short_id));
    let resized = Size::new(980_u32, 720_u32);
    let resized_logical = Size::new(resized.width as f32, resized.height as f32);
    renderer.reset(Rectangle::with_size(resized_logical));
    let mut ui = UserInterface::build(studio.view(), resized_logical, cache, &mut renderer);
    ui.draw(
        &mut renderer,
        &theme,
        &taim_core::renderer::Style {
            text_color: base.text_color,
        },
        taim_core::mouse::Cursor::Unavailable,
    );
    let pixels = renderer.screenshot(resized, 1.0, Color::BLACK);
    let lit = (96..116)
        .flat_map(|y| (355..475).map(move |x| (y * resized.width as usize + x) * 4))
        .filter(|&i| pixels[i] > 70 || pixels[i + 1] > 70 || pixels[i + 2] > 70)
        .count();
    assert!(lit > 25, "reopened short file lost line 1 after resize");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "manual performance probe for workspace startup, search, and view"]
fn workspace_performance_probe() {
    let begin = Instant::now();
    let (mut studio, _) =
        taim_ide::Studio::with_workspace(std::env::current_dir().unwrap()).unwrap();
    eprintln!("workspace startup: {:?}", begin.elapsed());
    let start = Instant::now();
    for query in ["f", "fn", "fun", "func", "function"] {
        let _ = studio.update(taim_ide::Message::GlobalSearchQueryChanged(query.into()));
    }
    eprintln!("five search keystrokes: {:?}", start.elapsed());
    let start = Instant::now();
    for _ in 0..30 {
        std::hint::black_box(studio.view());
    }
    eprintln!("thirty view builds: {:?}", start.elapsed());

    let theme = studio.theme();
    let base = taim_core::theme::Base::base(&theme);
    let size = Size::new(1200_u32, 800_u32);
    let logical = Size::new(size.width as f32, size.height as f32);
    let backend = std::env::var("VIRGO_RENDER_BACKEND").unwrap_or_else(|_| "tiny-skia".to_string());
    let mut renderer = taim_futures::futures::executor::block_on(<Renderer as Headless>::new(
        taim_core::renderer::Settings::default(),
        Some(&backend),
    ))
    .unwrap();
    let mut cache = Cache::new();
    let mut frame_times = Vec::new();
    let mut layout_times = Vec::new();
    let mut paint_times = Vec::new();
    let mut present_times = Vec::new();
    for frame in 0..34 {
        let _ = studio.update(taim_ide::Message::EditorAction(
            taim::widget::text_editor::Action::Edit(taim::widget::text_editor::Edit::Insert('x')),
        ));
        let begin = Instant::now();
        let mut ui = UserInterface::build(studio.view(), logical, cache, &mut renderer);
        let laid_out = Instant::now();
        ui.draw(
            &mut renderer,
            &theme,
            &taim_core::renderer::Style {
                text_color: base.text_color,
            },
            taim_core::mouse::Cursor::Unavailable,
        );
        let painted = Instant::now();
        let _ = renderer.screenshot(size, 1.0, base.background_color);
        let presented = Instant::now();
        cache = ui.into_cache();
        if frame >= 4 {
            frame_times.push(begin.elapsed());
            layout_times.push(laid_out - begin);
            paint_times.push(painted - laid_out);
            present_times.push(presented - painted);
        }
    }
    frame_times.sort();
    eprintln!(
        "typing frame p50 {:?}, p95 {:?}, max {:?}",
        frame_times[frame_times.len() / 2],
        frame_times[frame_times.len() * 95 / 100],
        frame_times[frame_times.len() - 1]
    );
    layout_times.sort();
    paint_times.sort();
    present_times.sort();
    eprintln!(
        "phase p50: layout {:?}, paint {:?}, present {:?}",
        layout_times[6], paint_times[6], present_times[6]
    );
}
