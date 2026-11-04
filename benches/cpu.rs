#![allow(missing_docs)]
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use taim::border;
use taim::mouse;
use taim::{Color, Point, Rectangle, Size, Theme};
use taim_core::Alignment::Center;
use taim_core::Length;
use taim_core::Length::Fill;
use taim_core::Never;
use taim_futures::futures::executor;
use taim_renderer::Renderer;
use taim_renderer::core::renderer::{self, Headless as _};
use taim_runtime::{UserInterface, user_interface};
use taim_widget::Element;
use taim_widget::{canvas, center_y, column, container, row, scrollable, space, text};

criterion_main!(taim_cpu_benches);
criterion_group!(taim_cpu_benches, measure_software_frames);

const LOGICAL_SIZE: Size<f32> = Size::new(512.0, 512.0);
const OUTPUT_SCALE: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
enum Scene {
    Paragraph,
    Application,
}

impl Scene {
    const ALL: [Self; 2] = [Self::Paragraph, Self::Application];

    const fn label(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Application => "application",
        }
    }

    fn view(self) -> Element<'static, Never> {
        match self {
            Self::Paragraph => paragraph(),
            Self::Application => application(),
        }
    }
}

pub fn measure_software_frames(criterion: &mut Criterion) {
    let mut renderer = executor::block_on(Renderer::new(
        renderer::Settings::default(),
        Some("software"),
    ))
    .expect("TAIM software renderer must be available");
    let mut group = criterion.benchmark_group("taim/software/frame_and_readback");

    for scene in Scene::ALL {
        let _ = group.bench_with_input(
            BenchmarkId::from_parameter(scene.label()),
            &scene,
            |bencher, scene| {
                let mut interface = UserInterface::build(
                    scene.view(),
                    LOGICAL_SIZE,
                    user_interface::Cache::new(),
                    &mut renderer,
                );
                bencher.iter(|| render_and_readback(&mut interface, &mut renderer));
            },
        );
    }

    group.finish();
}

fn render_and_readback(
    interface: &mut UserInterface<'static, Never, Theme, Renderer>,
    renderer: &mut Renderer,
) {
    interface.draw(
        renderer,
        &Theme::Dark,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );
    let _ = black_box(renderer.screenshot(
        Size::new(
            (LOGICAL_SIZE.width * OUTPUT_SCALE) as u32,
            (LOGICAL_SIZE.height * OUTPUT_SCALE) as u32,
        ),
        OUTPUT_SCALE,
        Color::WHITE,
    ));
}

fn paragraph() -> Element<'static, Never> {
    text(include_str!("ipsum.txt"))
        .ellipsis(text::Ellipsis::End)
        .into()
}

fn application() -> Element<'static, Never> {
    let tile = |size| color_tile(size);
    let header = container(
        row![
            tile(40),
            space::horizontal(),
            "TAIM",
            space::horizontal(),
            tile(40)
        ]
        .padding(10)
        .align_y(Center),
    )
    .style(|theme| {
        container::Style::default()
            .border(border::color(theme.palette().background.strong.color).width(1))
    });
    let sidebar = center_y(
        column!["Controls", tile(50), tile(50)]
            .spacing(40)
            .padding(10)
            .width(200)
            .align_x(Center),
    )
    .style(container::rounded_box);
    let content = container(
        scrollable(
            column![
                "Content",
                row((1..10).map(|index| tile(if index % 2 == 0 { 80 } else { 160 })))
                    .spacing(20)
                    .align_y(Center)
                    .wrap(),
                "Complete"
            ]
            .spacing(40)
            .align_x(Center)
            .width(Fill),
        )
        .height(Fill),
    )
    .padding(10);

    column![header, row![sidebar, content]].into()
}

fn color_tile<'a>(size: impl Into<Length> + Copy) -> Element<'a, Never> {
    struct Tile;

    impl canvas::Program<Never> for Tile {
        type State = ();

        fn draw(
            &self,
            _state: &Self::State,
            renderer: &Renderer,
            theme: &Theme,
            bounds: Rectangle,
            _cursor: mouse::Cursor,
        ) -> Vec<canvas::Geometry> {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                theme.palette().background.strong.color,
            );
            vec![frame.into_geometry()]
        }
    }

    canvas(Tile).width(size).height(size).into()
}
