//! Connect a window with a renderer.
use crate::core::Color;
use crate::core::backend;
use crate::core::color::Display;
use crate::core::renderer;
use crate::graphics::color;
use crate::graphics::compositor;
use crate::graphics::{self, Antialiasing, Shell, Viewport};
use crate::{Engine, Renderer};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// How the compositor should pick a surface format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorPolicy {
    /// Standard dynamic range: an 8-bit surface, the way every user interface
    /// framework has presented since the 1990s.
    #[default]
    Sdr,
    /// Prefer a high-dynamic-range surface when the platform offers one, and
    /// fall back to [`ColorPolicy::Sdr`] when it does not.
    ///
    /// This asks for [`wgpu::TextureFormat::Rgba16Float`] specifically. It is
    /// the only widely available surface format in which a component greater
    /// than `1.0` survives to the compositor at all: every `Unorm` format,
    /// including the 10-bit ones, clamps. A surface that clamps cannot show a
    /// highlight brighter than paper no matter how much precision it has.
    ///
    /// The rest of the pipeline needs no change to feed it. Taim's fragment
    /// shaders already emit linear light and rely on an sRGB surface to encode
    /// it in hardware, so pointing them at a half-float target simply stops
    /// discarding the range they were always producing.
    PreferHdr,
}

/// Chooses a surface format out of what the platform offers.
///
/// Split out of [`Compositor::request`] so that it can be tested without a
/// GPU. Format selection is a rule table, rule tables are where the bugs live,
/// and a rule that only executes on a machine with the right panel attached is
/// a rule that never gets tested.
fn select_format(
    available: &[wgpu::TextureFormat],
    policy: ColorPolicy,
) -> Option<wgpu::TextureFormat> {
    // Formats needing an opt-in feature cannot be assumed, and the packed
    // 10-bit formats are excluded because they are not a dependable surface
    // target across this crate's backends. `Rgb10a2Uint` in particular is not
    // blendable at all.
    const BLACKLIST: &[wgpu::TextureFormat] = &[
        wgpu::TextureFormat::Rgb10a2Unorm,
        wgpu::TextureFormat::Rgb10a2Uint,
    ];

    let usable = || {
        available.iter().copied().filter(|format| {
            format.required_features() == wgpu::Features::empty() && !BLACKLIST.contains(format)
        })
    };

    if policy == ColorPolicy::PreferHdr {
        if let Some(format) = usable().find(|format| *format == wgpu::TextureFormat::Rgba16Float) {
            return Some(format);
        }

        log::info!(
            "HDR was requested but no half-float surface is available; \
             falling back to standard dynamic range"
        );
    }

    if color::GAMMA_CORRECTION {
        usable().find(wgpu::TextureFormat::is_srgb)
    } else {
        usable().find(|format| !wgpu::TextureFormat::is_srgb(format))
    }
}

/// A window graphics backend for taim powered by `wgpu`.
pub struct Compositor {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    /// What presented frames can be used for: always rendered to, and
    /// copied from where the surface allows it (not WebGL).
    usage: wgpu::TextureUsages,
    engine: Engine,
    settings: Settings,
    device_lost: Arc<AtomicBool>,
}

/// A compositor error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// The adapter returned incorrect bytes in a required buffer roundtrip.
    #[error("graphics buffer integrity check failed: {0}")]
    BufferIntegrity(String),
    /// The native framebuffer cannot preserve the required channel precision.
    #[error("graphics surface precision check failed: {0}")]
    SurfacePrecision(String),
    /// Renderer pipelines could not be initialized on the adapter.
    #[error("graphics pipeline initialization failed: {0}")]
    PipelineInitialization(String),
    /// The surface creation failed.
    #[error("the surface creation failed: {0}")]
    SurfaceCreationFailed(#[from] wgpu::CreateSurfaceError),
    /// The surface is not compatible.
    #[error("the surface is not compatible")]
    IncompatibleSurface,
    /// No adapter was found for the options requested.
    #[error("no adapter was found for the options requested: {0:?}")]
    NoAdapterFound(String),
    /// No device request succeeded.
    #[error("no device request succeeded: {0:?}")]
    RequestDeviceFailed(Vec<(wgpu::Limits, wgpu::RequestDeviceError)>),
}

impl From<Error> for backend::Error {
    fn from(error: Error) -> Self {
        Self::GraphicsAdapterNotFound {
            backend: "wgpu",
            reason: backend::Reason::RequestFailed(error.to_string()),
        }
    }
}

impl Compositor {
    /// Requests a new [`Compositor`] with the given [`Settings`].
    ///
    /// Returns `None` if no compatible graphics adapter could be found.
    pub async fn request(
        settings: Settings,
        display: impl compositor::Display,
        compatible_window: impl compositor::Window,
        shell: Shell,
    ) -> Result<Self, Error> {
        #[cfg(all(target_os = "android", feature = "gles"))]
        {
            let display = Arc::new(display);
            let window = Arc::new(compatible_window);
            match Self::request_once(settings, display.clone(), window.clone(), shell.clone()).await
            {
                Ok(compositor) => Ok(compositor),
                Err(error)
                    if settings.backends.contains(wgpu::Backends::GL)
                        && settings.backends.contains(wgpu::Backends::VULKAN) =>
                {
                    log::warn!("Android Vulkan initialization rejected ({error}); trying GLES");
                    Self::request_once(
                        Settings {
                            backends: wgpu::Backends::GL,
                            ..settings
                        },
                        display,
                        window,
                        shell,
                    )
                    .await
                }
                Err(error) => Err(error),
            }
        }
        #[cfg(not(all(target_os = "android", feature = "gles")))]
        Self::request_once(settings, display, compatible_window, shell).await
    }

    async fn request_once(
        settings: Settings,
        display: impl compositor::Display,
        compatible_window: impl compositor::Window,
        shell: Shell,
    ) -> Result<Self, Error> {
        let instance = wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
            backends: settings.backends,
            flags: if cfg!(feature = "strict-assertions") {
                wgpu::InstanceFlags::debugging()
            } else {
                wgpu::InstanceFlags::empty()
            },
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display))
        })
        .await;

        log::info!("{settings:#?}");

        #[cfg(not(target_arch = "wasm32"))]
        if log::max_level() >= log::LevelFilter::Info {
            let available_adapters: Vec<_> = instance
                .enumerate_adapters(settings.backends)
                .await
                .iter()
                .map(wgpu::Adapter::get_info)
                .collect();
            log::info!("Available adapters: {available_adapters:#?}");
        }

        // Surface creation failures must reach the application. Suppressing
        // them leads to a misleading adapter-selection error on Android.
        let compatible_surface = Some(
            instance.create_surface(wgpu::SurfaceTarget::Window(Box::new(compatible_window)))?,
        );

        let power_preference = match settings.power_preference {
            backend::PowerPreference::None => wgpu::PowerPreference::None,
            backend::PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            backend::PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
        };

        let adapter_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or(power_preference),
            compatible_surface: compatible_surface.as_ref(),
            force_fallback_adapter: false,
        };

        let adapter = instance
            .request_adapter(&adapter_options)
            .await
            .map_err(|error| Error::NoAdapterFound(format!("{error}; {adapter_options:?}")))?;

        log::info!("Selected: {:#?}", adapter.get_info());

        #[cfg(all(target_os = "android", feature = "gles"))]
        if adapter.get_info().backend == wgpu::Backend::Gl {
            verify_egl_precision(&instance, &adapter)?;
        }
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;

        let (format, alpha_mode) = compatible_surface
            .as_ref()
            .and_then(|surface| {
                let capabilities = surface.get_capabilities(&adapter);

                // Glass and effects copy regions of the presented frame;
                // some surfaces (WebGL's) can only be rendered to, and then
                // those fall back rather than failing validation.
                if capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC) {
                    usage |= wgpu::TextureUsages::COPY_SRC;
                } else {
                    log::info!(
                        "the surface cannot be copied from: glass and effects are drawn flat"
                    );
                }

                log::info!("Available formats: {:#?}", capabilities.formats);

                let format = select_format(&capabilities.formats, settings.color).or_else(|| {
                    log::warn!("No format found!");

                    capabilities.formats.first().copied()
                });

                let alpha_modes = capabilities.alpha_modes;

                log::info!("Available alpha modes: {alpha_modes:#?}");

                let preferred_alpha =
                    if alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                        wgpu::CompositeAlphaMode::PreMultiplied
                    } else {
                        wgpu::CompositeAlphaMode::Auto
                    };

                format.zip(Some(preferred_alpha))
            })
            .ok_or(Error::IncompatibleSurface)?;

        log::info!("Selected format: {format:?} with alpha mode: {alpha_mode:?}");

        #[cfg(target_arch = "wasm32")]
        let limits = [wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())];

        #[cfg(not(target_arch = "wasm32"))]
        let limits = [
            wgpu::Limits::default().using_resolution(adapter.limits()),
            wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ];

        let limits = limits.into_iter().map(|limits| {
            let limits = if adapter.get_info().backend == wgpu::Backend::Gl {
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
            } else {
                limits
            };
            wgpu::Limits {
                max_bind_groups: 2,
                max_non_sampler_bindings: 2048,
                ..limits
            }
        });

        // Request SHADER_F16 only if the adapter supports it (e.g., not available in WebGL2)
        let required_features = if adapter.features().contains(wgpu::Features::SHADER_F16) {
            wgpu::Features::SHADER_F16
        } else {
            wgpu::Features::empty()
        };

        let mut errors = Vec::new();

        for required_limits in limits {
            let result = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("taim_wgpu::window::compositor device descriptor"),
                    required_features,
                    required_limits: required_limits.clone(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                })
                .await;

            match result {
                Ok((device, queue)) => {
                    #[cfg(target_os = "android")]
                    if adapter.get_info().backend == wgpu::Backend::Vulkan {
                        verify_buffer_integrity(&device, &queue)?;
                    }
                    let device_lost = Arc::new(AtomicBool::new(false));

                    {
                        let device_lost = device_lost.clone();

                        device.set_device_lost_callback(move |reason, message| {
                            log::error!("wgpu device lost ({reason:?}): {message}");
                            device_lost.store(true, Ordering::Relaxed);
                        });
                    }

                    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
                    let engine = Engine::new(
                        &adapter,
                        device,
                        queue,
                        format,
                        settings.antialiasing,
                        shell,
                    );

                    let internal_error = internal.pop().await;
                    let validation_error = validation.pop().await;
                    if let Some(error) = internal_error.or(validation_error) {
                        return Err(Error::PipelineInitialization(error.to_string()));
                    }
                    return Ok(Compositor {
                        instance,
                        adapter,
                        format,
                        alpha_mode,
                        usage,
                        engine,
                        settings,
                        device_lost,
                    });
                }
                Err(error) => {
                    errors.push((required_limits, error));
                }
            }
        }

        Err(Error::RequestDeviceFailed(errors))
    }

    /// Returns whether the underlying `wgpu::Device` has been lost
    /// (e.g. a driver crash, an OS-level GPU reset, or a laptop's
    /// discrete/integrated GPU switch).
    ///
    /// A lost device leaves every GPU resource owned by this
    /// [`Compositor`]'s [`Engine`] (pipelines, buffers, atlases) in an
    /// unusable state; nothing in this [`Compositor`] currently rebuilds
    /// them automatically (see `docs/performance/gpu-pipeline.md` for
    /// why a full rebuild path is a documented follow-up, not yet
    /// implemented here). Callers that observe `true` should, at
    /// minimum, stop attempting to `present` frames with this
    /// [`Compositor`] and surface the condition to the application
    /// (e.g. by requesting a fresh [`Compositor`] for the affected
    /// window) rather than continuing to submit work to a device that
    /// will silently reject it.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Relaxed)
    }
}

/// Query the owned EGL configuration without mutating driver resources. This
/// also makes a manually assembled consumer missing the local patch fail
/// explicitly instead of silently quantizing its entire UI to RGB565.
#[cfg(all(target_os = "android", feature = "gles"))]
#[allow(unsafe_code)]
fn verify_egl_precision(instance: &wgpu::Instance, adapter: &wgpu::Adapter) -> Result<(), Error> {
    // SAFETY: Both objects stay alive, are known GLES objects, and only read-only
    // EGL configuration queries are performed. No HAL resource is destroyed,
    // submitted, made current, or retained past either owner.
    let raw_instance = unsafe { instance.as_hal::<wgpu_hal::api::Gles>() }
        .ok_or_else(|| Error::SurfacePrecision("GLES instance unavailable".into()))?;
    let raw_adapter = unsafe { adapter.as_hal::<wgpu_hal::api::Gles>() }
        .ok_or_else(|| Error::SurfacePrecision("GLES adapter unavailable".into()))?;
    let egl = raw_adapter
        .adapter_context()
        .egl_instance()
        .ok_or_else(|| Error::SurfacePrecision("owned EGL context unavailable".into()))?;
    let display = raw_instance.raw_display();
    let config = raw_instance.egl_config();
    // EGL 1.0 attributes: RED_SIZE, GREEN_SIZE, BLUE_SIZE, ALPHA_SIZE.
    let mut bits = [0; 4];
    for (slot, attribute) in bits.iter_mut().zip([0x3024, 0x3023, 0x3022, 0x3021]) {
        *slot = egl
            .get_config_attrib(display, config, attribute)
            .map_err(|error| Error::SurfacePrecision(error.to_string()))?;
    }
    if bits.iter().any(|bits| *bits < 8) {
        return Err(Error::SurfacePrecision(format!(
            "EGL RGBA bits {bits:?}; require at least [8, 8, 8, 8]. Use the generated Android Cargo workspace with Taim's reviewed wgpu-hal patch"
        )));
    }
    log::info!("Android EGL channel precision: RGBA={bits:?}");
    Ok(())
}

/// Validate upload/readback once per Android Vulkan initialization. This is
/// not a per-frame fence and does not change the renderer's memory policy.
#[cfg(target_os = "android")]
fn verify_buffer_integrity(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), Error> {
    use wgpu::util::DeviceExt;
    const SIZE: u64 = 32768;
    let mut expected: Vec<u8> = (0..SIZE)
        .map(|index| ((index * 37 + index / 256 + 11) % 256) as u8)
        .collect();
    let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("taim.android.integrity.source"),
        contents: &expected,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("taim.android.integrity.readback"),
        size: SIZE,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut belt = wgpu::util::StagingBelt::new(device.clone(), SIZE);
    for phase in 0..3 {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        if phase != 0 {
            for byte in &mut expected {
                *byte = byte.wrapping_add(53);
            }
            if phase == 1 {
                queue.write_buffer(&source, 0, &expected);
            } else {
                belt.write_buffer(
                    &mut encoder,
                    &source,
                    0,
                    std::num::NonZeroU64::new(SIZE).expect("nonzero integrity size"),
                )
                .copy_from_slice(&expected);
                belt.finish();
            }
        }
        encoder.copy_buffer_to_buffer(&source, 0, &output, 0, SIZE);
        let index = queue.submit([encoder.finish()]);
        if phase == 2 {
            belt.recall();
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let slice = output.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: Some(std::time::Duration::from_secs(2)),
            })
            .map_err(|error| Error::BufferIntegrity(error.to_string()))?;
        receiver
            .try_recv()
            .map_err(|error| Error::BufferIntegrity(error.to_string()))?
            .map_err(|error| Error::BufferIntegrity(error.to_string()))?;
        let mapped = slice.get_mapped_range();
        let matches = mapped.as_ref() == expected.as_slice();
        drop(mapped);
        output.unmap();
        if !matches {
            return Err(Error::BufferIntegrity(format!(
                "upload/copy/readback bytes differ in phase {phase}"
            )));
        }
    }
    log::info!("Android Vulkan buffer integrity: PASS (mapped, queue, staging)");
    Ok(())
}

/// Creates a [`Compositor`] with the given [`Settings`] and window.
pub async fn new(
    settings: Settings,
    display: impl compositor::Display,
    compatible_window: impl compositor::Window,
    shell: Shell,
) -> Result<Compositor, Error> {
    Compositor::request(settings, display, compatible_window, shell).await
}

/// Presents the given primitives with the given [`Compositor`].
pub fn present(
    renderer: &mut Renderer,
    surface: &mut wgpu::Surface<'static>,
    viewport: &Viewport,
    background_color: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    present_to(
        renderer,
        surface,
        viewport,
        background_color,
        Display::SDR,
        on_pre_present,
    )
}

/// Presents onto `display`, applying its transform when the surface can carry
/// the extra range.
///
/// The panel is a parameter because nothing negotiates it yet: Wayland's
/// `wp_color_management_v1` is what will eventually report a real one, and
/// until then a caller who knows what their panel is has to say so. The
/// default in [`present`] is [`Display::SDR`], which is what every surface
/// that is not half-float actually is.
pub fn present_to(
    renderer: &mut Renderer,
    surface: &mut wgpu::Surface<'static>,
    viewport: &Viewport,
    background_color: Color,
    display: Display,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => {
            let view = &frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let format = frame.texture.format();

            // Two conditions. A half-float surface is only ever selected
            // under `ColorPolicy::PreferHdr`, so its presence signals that
            // the extra range was asked for and can be delivered.
            //
            // The second decides whether any of it means anything: under the
            // `web-colors` compatibility feature the renderer blends
            // gamma-encoded components, so there is no linear light in the
            // pipeline to carry a value above diffuse white and a luminance
            // multiplier would change hue rather than brightness. HDR
            // requires the linear path, which is the production default.
            //
            // There is deliberately no longer a condition on the engine's
            // format: pipelines are resolved per target, so an engine created
            // for an 8-bit surface can still render an HDR frame.
            let hdr = format == crate::display::Target::FORMAT && color::GAMMA_CORRECTION;

            let _submission = if hdr {
                renderer.present_hdr(Some(background_color), view, format, viewport, display)
            } else if !frame
                .texture
                .usage()
                .contains(wgpu::TextureUsages::COPY_SRC)
                || (color::GAMMA_CORRECTION && !format.is_srgb())
            {
                renderer.present_offscreen(background_color, view, &frame.texture, viewport)
            } else {
                renderer.present(
                    Some(background_color),
                    format,
                    view,
                    &frame.texture,
                    viewport,
                )
            };

            // Present the frame
            on_pre_present();
            frame.present();

            Ok(())
        }
        wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
            Err(compositor::SurfaceError::Outdated)
        }
        wgpu::CurrentSurfaceTexture::Timeout => Err(compositor::SurfaceError::Timeout),
        wgpu::CurrentSurfaceTexture::Occluded => Err(compositor::SurfaceError::Occluded),
        wgpu::CurrentSurfaceTexture::Lost => Err(compositor::SurfaceError::Lost),
        wgpu::CurrentSurfaceTexture::Validation => Err(compositor::SurfaceError::Other),
    }
}

impl graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = wgpu::Surface<'static>;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display,
        compatible_window: impl compositor::Window,
        shell: Shell,
    ) -> Result<Self, backend::Error> {
        if settings.backend.hardware().is_none() && !settings.backend.matches("wgpu") {
            return Err(backend::Error::GraphicsAdapterNotFound {
                backend: "wgpu",
                reason: backend::Reason::DidNotMatch {
                    preferred_backend: settings.backend,
                },
            });
        }

        let mut settings = Settings::from(settings);

        if let Some(backends) = wgpu::Backends::from_env() {
            settings.backends = backends;
        }

        if let Some(present_mode) = present_mode_from_env() {
            settings.present_mode = present_mode;
        }

        Ok(new(settings, display, compatible_window, shell).await?)
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer {
        Renderer::new(self.engine.clone(), settings)
    }

    fn create_surface(
        &mut self,
        window: impl compositor::Window,
        width: u32,
        height: u32,
    ) -> Result<Self::Surface, backend::Error> {
        let mut surface = self
            .instance
            .create_surface(wgpu::SurfaceTarget::Window(Box::new(window)))
            .map_err(|error| backend::Error::BackendError(error.to_string()))?;

        if width > 0 && height > 0 {
            self.configure_surface(&mut surface, width, height);
        }

        Ok(surface)
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        surface.configure(
            &self.engine.device,
            &wgpu::SurfaceConfiguration {
                // COPY_SRC as well as RENDER_ATTACHMENT where the surface
                // allows it: the effect pipeline and glass copy regions of
                // the presented frame. Where it does not, the renderer
                // checks the frame's usage and draws them flat.
                usage: self.usage,
                format: self.format,
                present_mode: self.settings.present_mode,
                width,
                height,
                alpha_mode: self.alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: self.settings.desired_maximum_frame_latency,
            },
        );
    }

    fn is_device_lost(&self) -> bool {
        Compositor::is_device_lost(self)
    }

    fn information(&self) -> compositor::Information {
        let information = self.adapter.get_info();

        compositor::Information {
            adapter: information.name,
            backend: format!("{:?}", information.backend),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn present_with_damage(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        frame_context: &graphics::FrameContext,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        // Log damage statistics at debug level for diagnostics.
        if let Some(ref damage) = frame_context.damage {
            if damage.is_full() {
                log::debug!("Frame damage: FULL surface redraw");
            } else if damage.is_empty() {
                log::debug!("Frame damage: empty (no-op)");
            } else if let Some(rects) = damage.rects() {
                let surface_area = viewport.logical_size().width * viewport.logical_size().height;
                let damage_area: f32 = rects.iter().map(crate::core::Rectangle::area).sum();
                let ratio = if surface_area > 0.0 {
                    damage_area / surface_area
                } else {
                    1.0
                };
                log::debug!(
                    "Frame damage: {} rects, {:.1}% of surface",
                    rects.len(),
                    ratio * 100.0,
                );
            }
        }

        // wgpu exposes no damage hint on its surface API, so the saving here
        // is in what gets *rasterised*, not in what gets handed to the
        // compositor: the render pass is scissored to the damaged region and
        // loads the surface rather than clearing it, so undamaged pixels are
        // neither shaded nor overwritten.
        //
        // Which region is safe to confine the frame to is the whole problem.
        // A swapchain returns one of several rotating buffers, holding what
        // was drawn to it N frames ago rather than last frame, and wgpu does
        // not report buffer age. `Renderer::accumulate_damage` therefore
        // redraws the union of the last N frames' damage, N being the
        // swapchain's depth: whichever buffer comes back, everything that has
        // changed since it was last drawn falls inside that union.
        //
        // It returns `None` -- a full redraw -- whenever that cannot be
        // guaranteed: unknown damage, full-surface damage, a history not yet
        // as deep as the swapchain, or a union that covers the surface anyway.
        let depth = self.settings.desired_maximum_frame_latency as usize + 1;

        let scissor = renderer.accumulate_damage(
            frame_context.damage.as_ref(),
            viewport.physical_size(),
            depth,
        );

        renderer.set_frame_damage(scissor);

        let result = present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        );

        // Cleared unconditionally: a `present` that failed must not leave the
        // next frame confined to a region computed for a frame that never
        // reached the screen.
        renderer.set_frame_damage(None);

        result
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        renderer.screenshot(viewport, background_color)
    }
}

/// The settings of a [`Compositor`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The present mode of the [`Renderer`].
    ///
    /// [`Renderer`]: crate::Renderer
    pub present_mode: wgpu::PresentMode,

    /// The graphics backends to use.
    pub backends: wgpu::Backends,

    /// The power-usage preference for graphics adapters.
    ///
    /// By default, it is [`backend::PowerPreference::None`].
    pub power_preference: backend::PowerPreference,

    /// The antialiasing strategy that will be used for triangle primitives.
    ///
    /// By default, it is `None`.
    pub antialiasing: Option<Antialiasing>,

    /// The maximum number of frames the presentation engine is allowed
    /// to queue ahead of the one currently displayed
    /// (`wgpu::SurfaceConfiguration::desired_maximum_frame_latency`).
    ///
    /// By default, `1`: the lowest-latency setting `wgpu` allows, so
    /// `Surface::get_current_texture` blocks until the previous frame has
    /// actually left the presentation queue, keeping input-to-photon
    /// latency as low as this present mode allows. A higher value trades
    /// some latency for smoother pacing under a CPU/GPU workload whose
    /// per-frame time varies -- "lower-latency modes require evidence on
    /// every supported target GPU/compositor" (`phase.md` Phase 6), so
    /// this crate does not raise it by default without that evidence;
    /// raising it is left to a caller that has gathered it for their own
    /// target hardware.
    pub desired_maximum_frame_latency: u32,

    /// How the surface format should be chosen.
    ///
    /// By default, [`ColorPolicy::Sdr`], which reproduces the format this
    /// crate has always selected.
    pub color: ColorPolicy,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            present_mode: wgpu::PresentMode::AutoVsync,
            backends: wgpu::Backends::all(),
            power_preference: backend::PowerPreference::None,
            antialiasing: None,
            desired_maximum_frame_latency: 1,
            color: ColorPolicy::Sdr,
        }
    }
}

impl From<backend::Settings> for Settings {
    fn from(settings: backend::Settings) -> Self {
        let backends = settings
            .backend
            .hardware()
            .map(|api| match api {
                backend::Api::Best => wgpu::Backends::all(),
                backend::Api::Vulkan => wgpu::Backends::VULKAN,
                backend::Api::Metal => wgpu::Backends::METAL,
                backend::Api::DirectX12 => wgpu::Backends::DX12,
                backend::Api::OpenGL => wgpu::Backends::GL,
                backend::Api::WebGPU => wgpu::Backends::BROWSER_WEBGPU,
            })
            .unwrap_or_else(wgpu::Backends::all);

        Self {
            present_mode: if settings.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            antialiasing: settings.antialiasing.then_some(Antialiasing::MSAAx4),
            backends,
            power_preference: settings.power_preference,
            desired_maximum_frame_latency: Settings::default().desired_maximum_frame_latency,
            color: Settings::default().color,
        }
    }
}

/// Obtains a [`wgpu::PresentMode`] from the current environment
/// configuration, if set.
///
/// The value returned by this function can be changed by setting
/// the `TAIM_PRESENT_MODE` env variable. The possible values are:
///
/// - `vsync` → [`wgpu::PresentMode::AutoVsync`]
/// - `no_vsync` → [`wgpu::PresentMode::AutoNoVsync`]
/// - `immediate` → [`wgpu::PresentMode::Immediate`]
/// - `fifo` → [`wgpu::PresentMode::Fifo`]
/// - `fifo_relaxed` → [`wgpu::PresentMode::FifoRelaxed`]
/// - `mailbox` → [`wgpu::PresentMode::Mailbox`]
pub fn present_mode_from_env() -> Option<wgpu::PresentMode> {
    let present_mode = std::env::var("TAIM_PRESENT_MODE").ok()?;

    match present_mode.to_lowercase().as_str() {
        "vsync" => Some(wgpu::PresentMode::AutoVsync),
        "no_vsync" => Some(wgpu::PresentMode::AutoNoVsync),
        "immediate" => Some(wgpu::PresentMode::Immediate),
        "fifo" => Some(wgpu::PresentMode::Fifo),
        "fifo_relaxed" => Some(wgpu::PresentMode::FifoRelaxed),
        "mailbox" => Some(wgpu::PresentMode::Mailbox),
        _ => None,
    }
}

#[cfg(test)]
mod device_lost_tests {
    use super::*;

    /// Exercises the flag/callback mechanism `Compositor::request` wires
    /// up to `wgpu::Device::set_device_lost_callback`, without needing a
    /// real `wgpu::Device` (which requires a live adapter and cannot be
    /// constructed in a headless unit test). `Compositor::request`'s own
    /// closure is a thin, untestable-in-isolation wrapper around exactly
    /// this store-into-a-shared-flag pattern; this test pins that the
    /// pattern itself is correct (starts `false`, becomes `true` once
    /// set, and is visible through a cloned `Arc`, matching how the
    /// callback closure and `Compositor` each hold their own clone).
    #[test]
    fn the_device_lost_flag_starts_false_and_becomes_true_once_set() {
        let device_lost = Arc::new(AtomicBool::new(false));
        assert!(!device_lost.load(Ordering::Relaxed));

        let callback_flag = device_lost.clone();
        let simulate_device_lost_callback = move || {
            callback_flag.store(true, Ordering::Relaxed);
        };

        simulate_device_lost_callback();

        assert!(device_lost.load(Ordering::Relaxed));
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn default_settings_use_the_lowest_latency_frame_queue() {
        // "FIFO is the safe default" (`phase.md` Phase 6) extends to
        // this setting too: a fresh `Settings` should not queue frames
        // ahead of the one currently displayed unless a caller has
        // opted into that trade-off explicitly.
        assert_eq!(Settings::default().desired_maximum_frame_latency, 1);
    }

    #[test]
    fn converting_from_backend_settings_preserves_the_default_frame_latency() {
        let converted = Settings::from(backend::Settings {
            backend: backend::Backend::Best,
            antialiasing: false,
            power_preference: backend::PowerPreference::None,
            vsync: true,
        });

        assert_eq!(
            converted.desired_maximum_frame_latency,
            Settings::default().desired_maximum_frame_latency
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use wgpu::TextureFormat;

    /// What a mainstream desktop surface reports today.
    const TYPICAL: &[TextureFormat] = &[
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb,
    ];

    /// The same surface on a display that can actually show the extra range.
    const WITH_HDR: &[TextureFormat] = &[
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba16Float,
    ];

    #[test]
    fn sdr_picks_what_this_crate_has_always_picked() {
        // The default must not move. Every committed golden was rendered
        // through the format this returns.
        let format = select_format(TYPICAL, ColorPolicy::Sdr)
            .expect("a typical surface must yield a format");

        assert_eq!(format.is_srgb(), color::GAMMA_CORRECTION);
    }

    #[test]
    fn hdr_is_taken_when_it_is_offered() {
        assert_eq!(
            select_format(WITH_HDR, ColorPolicy::PreferHdr),
            Some(TextureFormat::Rgba16Float)
        );
    }

    #[test]
    fn hdr_falls_back_rather_than_failing() {
        // A device with no half-float surface must still start. Refusing to
        // run because a panel is ordinary would be a worse failure than
        // presenting in standard range.
        let fallback =
            select_format(TYPICAL, ColorPolicy::PreferHdr).expect("HDR must fall back, not fail");

        assert_eq!(fallback, select_format(TYPICAL, ColorPolicy::Sdr).unwrap());
    }

    #[test]
    fn sdr_never_takes_a_half_float_surface() {
        // The inverse guard: asking for standard range must not silently
        // hand back an HDR surface just because one was listed first.
        assert_ne!(
            select_format(WITH_HDR, ColorPolicy::Sdr),
            Some(TextureFormat::Rgba16Float)
        );
    }

    #[test]
    fn the_packed_ten_bit_formats_stay_excluded() {
        // `Rgb10a2Uint` is not blendable, and neither packed format is a
        // dependable target across this crate's backends. Both were excluded
        // before this function existed; keep it that way.
        let only_packed = &[TextureFormat::Rgb10a2Unorm, TextureFormat::Rgb10a2Uint];

        assert_eq!(select_format(only_packed, ColorPolicy::Sdr), None);
        assert_eq!(select_format(only_packed, ColorPolicy::PreferHdr), None);
    }

    #[test]
    fn an_empty_surface_selects_nothing() {
        assert_eq!(select_format(&[], ColorPolicy::Sdr), None);
        assert_eq!(select_format(&[], ColorPolicy::PreferHdr), None);
    }

    #[test]
    fn the_default_policy_is_standard_range() {
        // Opting in to HDR must be a decision someone made, because it
        // changes what the compositor receives.
        assert_eq!(ColorPolicy::default(), ColorPolicy::Sdr);
        assert_eq!(Settings::default().color, ColorPolicy::Sdr);
    }
}
