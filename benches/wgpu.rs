#![allow(missing_docs)]
use criterion::{
    Bencher, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};

use taim::alignment;
use taim::mouse;
use taim_widget::{canvas, scrollable, stack, text};
type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::{Color, Font, Length, Pixels, Point, Rectangle, Size, Theme};
use taim_wgpu::Renderer;
use taim_wgpu::wgpu;

criterion_main!(taim_gpu_benches);
criterion_group!(taim_gpu_benches, measure_gpu_frames);

const TARGET_PIXELS: Size<u32> = Size::new(3840, 2160);
const TARGET_SCALE: f32 = 2.0;
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

#[derive(Debug, Clone, Copy)]
enum Scene {
    Canvas,
    Layers,
    Text { advanced: bool },
}

#[derive(Debug, Clone, Copy)]
struct Workload {
    label: &'static str,
    items: usize,
    scene: Scene,
}

const WORKLOADS: [Workload; 8] = [
    Workload {
        label: "canvas",
        items: 10,
        scene: Scene::Canvas,
    },
    Workload {
        label: "canvas",
        items: 1_000,
        scene: Scene::Canvas,
    },
    Workload {
        label: "layers",
        items: 10,
        scene: Scene::Layers,
    },
    Workload {
        label: "layers",
        items: 1_000,
        scene: Scene::Layers,
    },
    Workload {
        label: "basic_text",
        items: 1_000,
        scene: Scene::Text { advanced: false },
    },
    Workload {
        label: "basic_text",
        items: 100_000,
        scene: Scene::Text { advanced: false },
    },
    Workload {
        label: "advanced_text",
        items: 1_000,
        scene: Scene::Text { advanced: true },
    },
    Workload {
        label: "advanced_text",
        items: 100_000,
        scene: Scene::Text { advanced: true },
    },
];

pub fn measure_gpu_frames(criterion: &mut Criterion) {
    let (adapter, device, queue) = gpu_device();
    let mut group = criterion.benchmark_group("taim/wgpu/frame");

    for workload in WORKLOADS {
        let _ = group.throughput(Throughput::Elements(workload.items as u64));
        let _ = group.bench_with_input(
            BenchmarkId::new(workload.label, workload.items),
            &workload,
            |bencher, workload| {
                render_workload(bencher, &adapter, &device, &queue, *workload);
            },
        );
    }

    group.finish();
}

fn gpu_device() -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    use taim_futures::futures::executor;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("TAIM benchmark requires a GPU adapter");
    let (device, queue) = executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("taim-benchmark-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
    }))
    .expect("TAIM benchmark requires a WGPU device");

    (adapter, device, queue)
}

fn render_workload(
    bencher: &mut Bencher<'_>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    workload: Workload,
) {
    use taim_runtime as runtime;
    use taim_wgpu::graphics::{self, Antialiasing, Shell};
    use taim_winit::core;
    use taim_winit::core::renderer;

    let engine = taim_wgpu::Engine::new(
        adapter,
        device.clone(),
        queue.clone(),
        TARGET_FORMAT,
        Some(Antialiasing::MSAAx4),
        Shell::headless(),
    );
    let mut renderer = Renderer::new(engine, renderer::Settings::default());
    let viewport = graphics::Viewport::with_physical_size(TARGET_PIXELS, TARGET_SCALE);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("taim-benchmark-target"),
        size: wgpu::Extent3d {
            width: TARGET_PIXELS.width,
            height: TARGET_PIXELS.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut retained = Some(runtime::user_interface::Cache::default());
    let mut frame_number = 0_usize;

    bencher.iter(|| {
        let mut interface = runtime::UserInterface::build(
            workload.view(frame_number),
            viewport.logical_size(),
            retained
                .take()
                .expect("benchmark cache must be returned each frame"),
            &mut renderer,
        );
        interface.draw(
            &mut renderer,
            &Theme::Dark,
            &core::renderer::Style {
                text_color: Color::WHITE,
            },
            mouse::Cursor::Unavailable,
        );
        retained = Some(interface.into_cache());

        let submitted = renderer.present(
            Some(Color::BLACK),
            TARGET_FORMAT,
            &target_view,
            &target,
            &viewport,
        );
        let _ = black_box(device.poll(wgpu::PollType::Wait {
            submission_index: Some(submitted),
            timeout: None,
        }));
        frame_number = frame_number.wrapping_add(1);
    });
}

impl Workload {
    fn view(self, frame: usize) -> Element<'static, (), Theme, Renderer> {
        match self.scene {
            Scene::Canvas => canvas_scene(self.items),
            Scene::Layers => layered_text(self.items),
            Scene::Text { advanced } => dynamic_text(self.items, frame, advanced),
        }
    }
}

fn canvas_scene(n: usize) -> Element<'static, (), Theme, Renderer> {
    struct RectanglesAndLabels(usize);

    impl canvas::Program<(), Theme, Renderer> for RectanglesAndLabels {
        type State = canvas::Cache<Renderer>;

        fn draw(
            &self,
            cache: &Self::State,
            renderer: &Renderer,
            _theme: &Theme,
            bounds: Rectangle,
            _cursor: mouse::Cursor,
        ) -> Vec<canvas::Geometry<Renderer>> {
            vec![cache.draw_keyed(renderer, bounds.size(), &self.0, |frame| {
                for row in 0..self.0 {
                    frame.fill_rectangle(
                        Point::new(0.0, row as f32),
                        Size::new(10.0, 10.0),
                        Color::WHITE,
                    );
                    frame.fill_text(canvas::Text {
                        content: row.to_string(),
                        position: Point::new(0.0, row as f32),
                        color: Color::BLACK,
                        size: Pixels::from(16),
                        line_height: text::LineHeight::default(),
                        font: Font::DEFAULT,
                        align_x: text::Alignment::Left,
                        align_y: alignment::Vertical::Top,
                        shaping: text::Shaping::Basic,
                        wrapping: text::Wrapping::default(),
                        ellipsis: text::Ellipsis::default(),
                        max_width: f32::INFINITY,
                    });
                }
            })]
        }
    }

    canvas(RectanglesAndLabels(n))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn layered_text(n: usize) -> Element<'static, (), Theme, Renderer> {
    stack((0..n).map(|index| text(format!("TAIM layer {index}")).into()))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn dynamic_text(
    characters: usize,
    frame: usize,
    advanced: bool,
) -> Element<'static, (), Theme, Renderer> {
    const SAMPLE: &str = include_str!("ipsum.txt");
    let mut content = String::with_capacity(characters.saturating_add(40));
    content.extend(std::iter::repeat(SAMPLE.chars()).flatten().take(characters));
    use std::fmt::Write as _;
    let _ = write!(content, " · TAIM frame {frame} 😎");

    let label = text(content).size(10).shaping(if advanced {
        text::Shaping::Advanced
    } else {
        text::Shaping::Basic
    });

    scrollable(label).into()
}
