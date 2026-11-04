//! Deterministic render, capture, diff, tolerance and report.
//!
//! # A missing golden must fail
//!
//! The tempting behaviour — if there is no golden, write what we rendered
//! and pass — is the one thing this harness will not do. A harness that
//! adopts whatever it first saw pins the bug along with the feature, and
//! it does so *silently*, on the one run where a human was most likely to
//! be looking at something else. Every golden here is adopted by someone
//! typing [`UPDATE_ENV`] on purpose.
//!
//! `phase.md` says it outright: *"never auto-update goldens in CI"*. The
//! enforcement is [`Harness::compare`] returning [`Outcome::Missing`], and
//! [`Outcome::is_failure`] treating it as a failure rather than a skip.
//!
//! # Two tolerance classes, because two things are being tested
//!
//! - [`Tolerance::Exact`] — a quad is a quad. Every pixel is a decision
//!   somebody made, and a single changed byte is a real change. Anything
//!   less than exact here is a test that stops noticing.
//! - [`Tolerance::Approximate`] — text and effects go through a
//!   rasteriser, and a font hinting change or a driver update moves
//!   subpixels without moving anything a user would call different.
//!   Demanding exactness would produce a suite that is red on every
//!   machine but the one it was recorded on, and a suite that is always
//!   red teaches people to ignore it.
//!
//! The classes are declared per case rather than inferred, because
//! "does this contain text?" is not something the harness can see, and
//! guessing wrong in the lenient direction is invisible.
//!
//! # Determinism
//!
//! *"Pin fonts, scale, color space, renderer configuration and
//! deterministic time."* A golden compares two runs, so anything that
//! varies between them is noise the suite will eventually blame on the
//! code. [`Harness`] pins the renderer backend and the clear colour; the
//! caller pins scale and content.

use taim_core::Renderer as _;
use taim_core::renderer::Headless as _;
use taim_core::{Color, Rectangle, Size};
use taim_renderer::Renderer;

/// The environment variable that permits adopting a golden.
///
/// Deliberately awkward and deliberately explicit. It exists so that
/// adopting a golden is an act, recorded in a shell history, rather than
/// a side effect of running the suite.
pub const UPDATE_ENV: &str = "TAIM_UPDATE_GOLDEN";

/// The clear colour every case renders against.
///
/// Opaque black, not transparent: a case that draws nothing is then
/// distinguishable from a case whose alpha handling broke, which a
/// transparent clear would hide by making both all-zero.
pub const CLEAR: Color = Color::BLACK;

/// How exactly a case must match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tolerance {
    /// Every pixel identical.
    ///
    /// For geometry the renderer produces exactly: quads, borders, solid
    /// fills. A changed byte is a changed decision.
    Exact,

    /// Close enough that no user would call it different.
    ///
    /// For text and effects, which go through a rasteriser whose output
    /// legitimately moves with font hinting and driver versions.
    Approximate {
        /// The largest per-channel difference tolerated in any one pixel.
        ///
        /// Bounded, not unlimited: a pixel that went from black to white
        /// is a *different picture*, however few of them there are.
        channel: u8,

        /// The fraction of pixels that may differ at all, `0.0..=1.0`.
        ///
        /// Bounds the other axis: a suite that allowed a small delta on
        /// every pixel would accept a uniform brightness shift across the
        /// whole surface, which is exactly the kind of regression a golden
        /// is for.
        pixels: f32,
    },
}

impl Tolerance {
    /// The tolerance for antialiased text and effects.
    ///
    /// One channel step, on up to 2% of pixels: enough for hinting to move
    /// an edge, nowhere near enough for a colour, a glyph, or a layout to
    /// change.
    pub const TEXT: Self = Self::Approximate {
        channel: 1,
        pixels: 0.02,
    };

    /// The tolerance for blurred or composited effects, whose edges move
    /// more than text's.
    pub const EFFECT: Self = Self::Approximate {
        channel: 2,
        pixels: 0.05,
    };
}

/// How two images differed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Difference {
    /// How many pixels differed at all.
    pub pixels: usize,
    /// How many pixels there were.
    pub total: usize,
    /// The largest per-channel difference seen.
    pub worst_channel: u8,
    /// Mean absolute per-channel error, over every channel of every pixel.
    ///
    /// Separates "a few pixels moved a long way" from "everything moved a
    /// little", which is exactly the distinction between a real regression
    /// and a precision change, and which neither `pixels` nor `worst_channel`
    /// can express on its own.
    pub mean_absolute_error: f64,
    /// Root-mean-square per-channel error.
    ///
    /// Weights large excursions more heavily than [`Difference::mean_absolute_error`],
    /// so a localised but severe change stays visible in the number even when
    /// it is a small fraction of the image.
    pub root_mean_square_error: f64,
}

impl Difference {
    /// The fraction of pixels that differed.
    pub fn ratio(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }

        self.pixels as f32 / self.total as f32
    }

    /// Whether anything differed.
    pub fn any(&self) -> bool {
        self.pixels > 0
    }

    /// A one-line summary of every metric, for a classification report.
    pub fn summary(&self) -> String {
        format!(
            "{:.2}% of pixels, worst channel {}, MAE {:.3}, RMSE {:.3}",
            self.ratio() * 100.0,
            self.worst_channel,
            self.mean_absolute_error,
            self.root_mean_square_error
        )
    }

    /// Whether this difference is within `tolerance`.
    pub fn is_within(&self, tolerance: Tolerance) -> bool {
        match tolerance {
            Tolerance::Exact => !self.any(),
            Tolerance::Approximate { channel, pixels } => {
                self.worst_channel <= channel && self.ratio() <= pixels
            }
        }
    }
}

/// What comparing a case produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The render matched.
    Matched,

    /// The render differed within tolerance.
    ///
    /// Distinct from [`Outcome::Matched`] so a report can say *how close*
    /// a case is running to its tolerance. A case that has silently crept
    /// to 1.9% of a 2% budget is one commit from being flaky, and a
    /// harness that only said "pass" would never mention it.
    Tolerated(Difference),

    /// The render differed beyond tolerance.
    Differed(Difference),

    /// There is no golden to compare against.
    ///
    /// A **failure**, not a skip. See the module docs: a harness that
    /// adopts what it first saw pins the bug along with the feature.
    Missing,

    /// The render and the golden are different sizes.
    ///
    /// Its own variant rather than "everything differed": a size change is
    /// almost always a layout regression or a scale-factor mistake, and
    /// reporting it as 100% of pixels differing buries the one fact that
    /// explains it.
    Resized {
        /// The golden's size, in pixels.
        golden: (u32, u32),
        /// What was rendered.
        rendered: (u32, u32),
    },
}

impl Outcome {
    /// Whether this outcome should fail a test.
    pub fn is_failure(&self) -> bool {
        !matches!(self, Outcome::Matched | Outcome::Tolerated(_))
    }

    /// A description a human can act on.
    ///
    /// Every failure says what to do next, because the alternative is a
    /// developer who knows only that a picture changed and not how to
    /// look at it.
    pub fn report(&self, case: &str) -> String {
        match self {
            Outcome::Matched => format!("{case}: matched"),
            Outcome::Tolerated(difference) => {
                format!("{case}: within tolerance ({})", difference.summary())
            }
            Outcome::Differed(difference) => format!(
                "{case}: DIFFERED -- {}.\n\
                 Look at the diff before doing anything else. If the change is intended, it needs \
                 a rendered diff, a design reason, and a performance report on the pull request. \
                 Then adopt it with {UPDATE_ENV}=1.",
                difference.summary()
            ),
            Outcome::Missing => format!(
                "{case}: NO GOLDEN.\n\
                 This is a failure, not a skip: a harness that adopted whatever it first saw \
                 would pin a bug as readily as a feature. Render it, look at it, and if it is \
                 right, adopt it with {UPDATE_ENV}=1."
            ),
            Outcome::Resized { golden, rendered } => format!(
                "{case}: SIZE CHANGED -- golden is {}x{}, render is {}x{}.\n\
                 Almost always a layout regression or a scale-factor mistake rather than a \
                 visual change.",
                golden.0, golden.1, rendered.0, rendered.1
            ),
        }
    }
}

/// A captured image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// RGBA8, row-major.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Compares against `other`.
    pub fn diff(&self, other: &Image) -> Result<Difference, Outcome> {
        if self.width != other.width || self.height != other.height {
            return Err(Outcome::Resized {
                golden: (other.width, other.height),
                rendered: (self.width, self.height),
            });
        }

        let mut pixels = 0;
        let mut worst_channel = 0u8;
        let mut absolute_error = 0u64;
        let mut squared_error = 0u64;
        let mut channels = 0u64;

        for (a, b) in self.rgba.chunks_exact(4).zip(other.rgba.chunks_exact(4)) {
            let mut differed = false;

            for (a, b) in a.iter().zip(b.iter()) {
                let delta = a.abs_diff(*b);

                channels += 1;
                absolute_error += u64::from(delta);
                squared_error += u64::from(delta) * u64::from(delta);

                if delta > 0 {
                    differed = true;
                    worst_channel = worst_channel.max(delta);
                }
            }

            if differed {
                pixels += 1;
            }
        }

        let channels = channels.max(1) as f64;

        Ok(Difference {
            pixels,
            total: (self.width * self.height) as usize,
            worst_channel,
            mean_absolute_error: absolute_error as f64 / channels,
            root_mean_square_error: (squared_error as f64 / channels).sqrt(),
        })
    }
}

/// Renders and compares cases against stored goldens.
pub struct Harness {
    directory: std::path::PathBuf,
    /// Read once, at construction, rather than consulted per comparison.
    ///
    /// An environment variable is process-global, and a test that set one
    /// to exercise the adopt path would change the behaviour of every
    /// other case running in parallel — turning the suite's own safety
    /// rule off underneath them, nondeterministically. Reading it once
    /// into a field makes the permission a value, which a test can pass
    /// without touching anything else.
    may_update: bool,
}

impl Harness {
    /// A harness storing goldens under `directory`, permitting adoption
    /// if [`UPDATE_ENV`] is set.
    pub fn new(directory: impl Into<std::path::PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            may_update: std::env::var(UPDATE_ENV).is_ok_and(|value| value != "0"),
        }
    }

    /// The default harness, storing goldens beside this file.
    pub fn default_directory() -> Self {
        Self::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/images"))
    }

    /// This harness, permitted (or not) to adopt goldens.
    #[must_use]
    pub fn with_updates(mut self, may_update: bool) -> Self {
        self.may_update = may_update;
        self
    }

    /// Whether goldens may be adopted.
    pub fn may_update(&self) -> bool {
        self.may_update
    }

    /// Renders `draw` at `size` and `scale`, deterministically.
    ///
    /// The renderer is constructed per call rather than reused: a
    /// renderer carries caches, and a case that passed only because a
    /// previous case warmed an atlas is a case that will fail the day
    /// someone reorders the file.
    pub fn render(size: Size<u32>, scale: f32, draw: impl FnOnce(&mut Renderer)) -> Option<Image> {
        Self::render_onto(size, scale, CLEAR, draw)
    }

    /// [`Harness::render`], against a chosen background.
    ///
    /// Primitive cases clear to [`CLEAR`] so "drew nothing" is
    /// distinguishable from "drew black". Widget cases clear to their
    /// theme's own background instead, because a widget rendered against a
    /// colour its theme never uses is not the picture anyone is shipping --
    /// and text at the theme's colour on a black clear is how this harness
    /// first "proved" that every glyph in the framework was blank.
    pub fn render_onto(
        size: Size<u32>,
        scale: f32,
        clear: Color,
        draw: impl FnOnce(&mut Renderer),
    ) -> Option<Image> {
        // Pinned to the software renderer, not merely to a software *adapter*:
        // `tiny-skia` rasterises on the CPU with no GPU, no driver, and no
        // Vulkan implementation in the picture at all, so a golden recorded
        // on one machine is reproducible on any other. A GPU backend --
        // llvmpipe included -- would make these images a property of the
        // driver version.
        let mut renderer = taim_futures::futures::executor::block_on(
            <Renderer as taim_core::renderer::Headless>::new(
                taim_core::renderer::Settings::default(),
                Some("tiny-skia"),
            ),
        )?;

        let logical = Size::new(size.width as f32 / scale, size.height as f32 / scale);

        renderer.reset(Rectangle::with_size(logical));
        draw(&mut renderer);

        let rgba = renderer.screenshot(size, scale, clear);

        Some(Image {
            width: size.width,
            height: size.height,
            rgba,
        })
    }

    /// Renders one [`Element`] through the real widget pipeline.
    ///
    /// Goes through `UserInterface::build` and `draw` rather than calling
    /// renderer primitives directly, because the thing worth pinning is what
    /// a *widget* produces -- its layout, its catalogue lookup, its glyphs --
    /// not what a quad looks like. A suite that drew primitives would have
    /// passed happily while every icon in the framework resolved to a
    /// missing glyph, which is exactly what happened.
    /// Renders one [`Element`] through the real widget pipeline.
    ///
    /// Goes through `UserInterface::build` and `draw` rather than calling
    /// renderer primitives directly, because the thing worth pinning is what
    /// a *widget* produces -- its layout, its catalogue lookup, its glyphs --
    /// not what a quad looks like. A suite that drew primitives would have
    /// passed happily while every icon in the framework resolved to a
    /// missing glyph, which is exactly what happened.
    ///
    /// # The tree must outlive the screenshot
    ///
    /// This does not delegate to [`Harness::render_onto`], and the reason is
    /// worth stating because it costs an afternoon to rediscover: **the
    /// renderer's recorded text does not survive its widget tree being
    /// dropped.** A `fill_paragraph` records a handle into text the tree
    /// owns, so dropping the `UserInterface` before `screenshot` yields a
    /// uniform background -- on both backends, with no error, no panic, and
    /// no warning. A harness shaped as "draw in a closure, screenshot after"
    /// therefore captures a perfectly plausible blank, and would happily
    /// adopt it as a golden.
    ///
    /// So the interface is built, drawn, and screenshotted in one scope, and
    /// only then dropped.
    pub fn render_element<'a, Message>(
        size: Size<u32>,
        scale: f32,
        theme: &taim::Theme,
        element: impl Into<taim_core::Element<'a, Message, taim::Theme, Renderer>>,
    ) -> Option<Image>
    where
        Message: 'a,
    {
        use taim_core::theme::Base as _;

        // Both the clear colour and the text colour come from the theme,
        // exactly as `taim_winit` derives them for a real window. Using
        // `Style::default()` means black text, which against the default
        // black clear renders an invisible, entirely plausible-looking blank.
        let base = theme.base();

        let mut renderer = taim_futures::futures::executor::block_on(
            <Renderer as taim_core::renderer::Headless>::new(
                taim_core::renderer::Settings::default(),
                Some("tiny-skia"),
            ),
        )?;

        let logical = Size::new(size.width as f32 / scale, size.height as f32 / scale);

        renderer.reset(Rectangle::with_size(logical));

        let mut interface = taim_runtime::UserInterface::build(
            element.into(),
            logical,
            taim_runtime::user_interface::Cache::new(),
            &mut renderer,
        );

        // `update` before `draw`, exactly as the real frame loop does. This
        // is not ceremony: several widgets only learn their own status while
        // handling events, and `button` resolves an unset status to
        // `Status::Disabled` at paint time (`widget/src/button.rs`). A
        // harness that skipped this rendered every button in the suite as
        // greyed out -- and the goldens looked entirely plausible.
        // A `RedrawRequested`, exactly as a real window delivers every
        // frame. Not ceremony, and not satisfied by an empty event slice:
        // `button` latches its paint status *only* while handling this
        // specific event (`widget/src/button.rs`), and an unset status
        // resolves to `Status::Disabled` at paint time. A harness that
        // skipped it rendered every button in the suite greyed out, and the
        // goldens looked entirely plausible -- an enabled and a disabled
        // button were pixel identical, which is what gave it away.
        let mut messages = Vec::new();

        let _ = interface.update(
            &taim_core::window::Headless,
            &taim_core::shell::Waker::noop(),
            &[taim_core::Event::Window(
                taim_core::window::Event::RedrawRequested(taim_core::time::Instant::now()),
            )],
            taim_core::mouse::Cursor::Unavailable,
            &mut renderer,
            &mut messages,
        );

        interface.draw(
            &mut renderer,
            theme,
            &taim_core::renderer::Style {
                text_color: base.text_color,
            },
            taim_core::mouse::Cursor::Unavailable,
        );

        // Still in scope. See the note above.
        let rgba = renderer.screenshot(size, scale, base.background_color);

        drop(interface);

        Some(Image {
            width: size.width,
            height: size.height,
            rgba,
        })
    }

    /// Compares `rendered` against the golden named `case`.
    pub fn compare(&self, case: &str, rendered: &Image, tolerance: Tolerance) -> Outcome {
        let Some(golden) = self.load(case) else {
            if self.may_update {
                self.store(case, rendered);

                // Still not `Matched`: adopting a golden is not evidence
                // that it is right, and reporting a pass would tell a
                // developer their change was verified when nothing
                // compared it to anything.
                return Outcome::Tolerated(Difference {
                    pixels: 0,
                    total: (rendered.width * rendered.height) as usize,
                    worst_channel: 0,
                    mean_absolute_error: 0.0,
                    root_mean_square_error: 0.0,
                });
            }

            return Outcome::Missing;
        };

        let difference = match rendered.diff(&golden) {
            Ok(difference) => difference,
            Err(outcome) => return outcome,
        };

        if !difference.any() {
            return Outcome::Matched;
        }

        if difference.is_within(tolerance) {
            return Outcome::Tolerated(difference);
        }

        if self.may_update {
            self.store(case, rendered);

            return Outcome::Tolerated(difference);
        }

        Outcome::Differed(difference)
    }

    /// Writes the golden, the render, and their difference to `diffs/`.
    ///
    /// `.github/workflows/golden.yml` uploads that directory when the job
    /// fails, which is the only way a reviewer can see *what* changed: a
    /// percentage in a log tells them a case moved, not whether it moved for
    /// a good reason. Nothing wrote there until this existed, so the
    /// workflow's upload step had been collecting an empty directory.
    ///
    /// Written as binary PPM (P6) rather than PNG: no encoder dependency, no
    /// compressor between the renderer and what a human looks at, and every
    /// image viewer opens it.
    pub fn write_diff(&self, case: &str, rendered: &Image) {
        let directory = self
            .directory
            .parent()
            .unwrap_or(&self.directory)
            .join("diffs");

        let _ = std::fs::create_dir_all(&directory);

        let write = |suffix: &str, image: &Image| {
            let mut bytes = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();

            bytes.extend(image.rgba.chunks_exact(4).flat_map(|pixel| &pixel[..3]));

            let _ = std::fs::write(directory.join(format!("{case}.{suffix}.ppm")), bytes);
        };

        write("rendered", rendered);

        let Some(golden) = self.load(case) else {
            return;
        };

        write("golden", &golden);

        if golden.width != rendered.width || golden.height != rendered.height {
            return;
        }

        // Magenta marks every changed pixel; unchanged pixels are dimmed so
        // the change is legible against the picture it happened in.
        let mut marked = rendered.clone();

        for (pixel, reference) in marked
            .rgba
            .chunks_exact_mut(4)
            .zip(golden.rgba.chunks_exact(4))
        {
            if pixel[..3].iter().zip(&reference[..3]).any(|(a, b)| a != b) {
                pixel[0] = 255;
                pixel[1] = 0;
                pixel[2] = 255;
            } else {
                for channel in &mut pixel[..3] {
                    *channel /= 3;
                }
            }
        }

        write("diff", &marked);
    }

    /// Loads the golden for `case`.
    ///
    /// Stored as raw RGBA with a small header rather than PNG: a golden is
    /// compared byte-for-byte, and putting a compressor between the
    /// renderer and the comparison means a library upgrade can change what
    /// the suite believes it recorded.
    fn load(&self, case: &str) -> Option<Image> {
        let bytes = std::fs::read(self.path(case)).ok()?;

        if bytes.len() < 8 {
            return None;
        }

        let width = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        let height = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let rgba = bytes[8..].to_vec();

        if rgba.len() != (width as usize) * (height as usize) * 4 {
            // A truncated golden is a corrupt one. Treating it as absent
            // makes it a loud failure rather than a comparison against
            // garbage.
            return None;
        }

        Some(Image {
            width,
            height,
            rgba,
        })
    }

    fn store(&self, case: &str, image: &Image) {
        let _ = std::fs::create_dir_all(&self.directory);

        let mut bytes = Vec::with_capacity(8 + image.rgba.len());
        bytes.extend_from_slice(&image.width.to_le_bytes());
        bytes.extend_from_slice(&image.height.to_le_bytes());
        bytes.extend_from_slice(&image.rgba);

        let _ = std::fs::write(self.path(case), bytes);
    }

    fn path(&self, case: &str) -> std::path::PathBuf {
        self.directory.join(format!("{case}.rgba"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, fill: u8) -> Image {
        Image {
            width,
            height,
            rgba: vec![fill; (width * height * 4) as usize],
        }
    }

    fn scratch() -> Harness {
        let directory = std::env::temp_dir().join(format!(
            "taim-golden-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));

        let _ = std::fs::remove_dir_all(&directory);

        // Explicitly not permitted, whatever the environment says: these
        // tests decide for themselves.
        Harness::new(directory).with_updates(false)
    }

    #[test]
    fn a_missing_golden_is_a_failure_not_a_pass() {
        // The whole reason this file exists. A harness that adopted
        // whatever it first saw would pin a bug as readily as a feature,
        // silently, on the run nobody was watching.
        let harness = scratch();
        let outcome = harness.compare("nonexistent", &image(4, 4, 0), Tolerance::Exact);

        assert_eq!(outcome, Outcome::Missing);
        assert!(outcome.is_failure());
    }

    #[test]
    fn a_missing_golden_says_how_to_adopt_it() {
        // A failure a developer cannot act on is a failure they learn to
        // ignore.
        let report = Outcome::Missing.report("button_hover");

        assert!(report.contains(UPDATE_ENV), "{report}");
        assert!(report.contains("NO GOLDEN"), "{report}");
    }

    #[test]
    fn an_identical_render_matches() {
        let rendered = image(4, 4, 128);

        let adopting = scratch().with_updates(true);
        let _ = adopting.compare("solid", &rendered, Tolerance::Exact);

        let comparing = Harness::new(adopting.directory.clone()).with_updates(false);

        assert_eq!(
            comparing.compare("solid", &rendered, Tolerance::Exact),
            Outcome::Matched
        );
    }

    #[test]
    fn adopting_a_golden_does_not_report_a_match() {
        // Adopting is not evidence of correctness. Reporting `Matched`
        // would tell a developer their change was verified when nothing
        // compared it to anything.
        let harness = scratch().with_updates(true);

        let outcome = harness.compare("fresh", &image(4, 4, 7), Tolerance::Exact);

        assert_ne!(outcome, Outcome::Matched);
        assert!(!outcome.is_failure(), "adopting must not fail the run");
    }

    #[test]
    fn a_harness_that_may_not_update_never_writes() {
        // The rule CI depends on. A run that quietly wrote a golden would
        // make the next run pass for the wrong reason.
        let harness = scratch().with_updates(false);

        let _ = harness.compare("never_written", &image(4, 4, 3), Tolerance::Exact);

        assert!(
            !harness.path("never_written").exists(),
            "a harness without permission must not write a golden"
        );
    }

    #[test]
    fn one_changed_byte_fails_an_exact_case() {
        // A quad is a quad. Every pixel is a decision somebody made.
        let mut changed = image(4, 4, 128);
        changed.rgba[0] = 129;

        let difference = changed.diff(&image(4, 4, 128)).unwrap();

        assert!(difference.any());
        assert!(!difference.is_within(Tolerance::Exact));
        assert_eq!(difference.worst_channel, 1);
        assert_eq!(difference.pixels, 1);
    }

    #[test]
    fn a_subpixel_shift_is_tolerated_for_text() {
        // Font hinting and driver updates move edges by a step. Demanding
        // exactness here produces a suite that is red on every machine but
        // the one it was recorded on -- and a suite that is always red is
        // one nobody reads.
        let golden = image(10, 10, 128);
        let mut rendered = golden.clone();

        // One channel step on a single pixel: 1% of 100.
        rendered.rgba[0] = 129;

        let difference = rendered.diff(&golden).unwrap();

        assert!(difference.is_within(Tolerance::TEXT));
    }

    #[test]
    fn a_uniform_brightness_shift_is_not_tolerated() {
        // The regression a per-pixel-only tolerance would accept: every
        // pixel moved by one step is a different picture, and it is
        // exactly what a colour-space or gamma mistake looks like.
        let golden = image(10, 10, 128);
        let rendered = image(10, 10, 129);

        let difference = rendered.diff(&golden).unwrap();

        assert_eq!(difference.worst_channel, 1, "within the per-pixel budget");
        assert_eq!(difference.ratio(), 1.0, "but on every pixel");
        assert!(
            !difference.is_within(Tolerance::TEXT),
            "a shift across the whole surface must not pass as antialiasing"
        );
    }

    #[test]
    fn a_black_to_white_pixel_is_not_tolerated_however_rare() {
        // The other axis: a single pixel that inverted is a different
        // picture, and a ratio-only tolerance would wave it through.
        let golden = image(100, 100, 0);
        let mut rendered = golden.clone();

        rendered.rgba[0] = 255;

        let difference = rendered.diff(&golden).unwrap();

        assert!(difference.ratio() < 0.0002, "one pixel in ten thousand");
        assert!(
            !difference.is_within(Tolerance::TEXT),
            "a pixel that went from black to white is a different picture"
        );
    }

    #[test]
    fn a_size_change_is_reported_as_itself() {
        // Reporting it as "100% of pixels differed" would bury the one
        // fact that explains it.
        let outcome = image(4, 4, 0).diff(&image(8, 8, 0)).unwrap_err();

        assert_eq!(
            outcome,
            Outcome::Resized {
                golden: (8, 8),
                rendered: (4, 4),
            }
        );
        assert!(outcome.is_failure());
        assert!(outcome.report("case").contains("SIZE CHANGED"));
    }

    #[test]
    fn a_corrupt_golden_is_treated_as_missing() {
        // Loudly absent rather than quietly compared against garbage.
        let harness = scratch();
        let _ = std::fs::create_dir_all(&harness.directory);
        let _ = std::fs::write(harness.path("truncated"), [4, 0, 0, 0, 4, 0, 0, 0, 1, 2, 3]);

        assert_eq!(
            harness.compare("truncated", &image(4, 4, 0), Tolerance::Exact),
            Outcome::Missing
        );
    }

    #[test]
    fn a_difference_report_says_how_far_from_tolerance_it_ran() {
        // A case that has crept to 1.9% of a 2% budget is one commit from
        // flaky. A harness that only said "pass" would never mention it.
        let report = Outcome::Tolerated(Difference {
            pixels: 19,
            total: 1000,
            worst_channel: 1,
            mean_absolute_error: 0.0,
            root_mean_square_error: 0.0,
        })
        .report("text_hinting");

        assert!(report.contains("1.90%"), "{report}");
    }

    #[test]
    fn rendering_is_deterministic_across_calls() {
        // The premise of the whole suite: two runs of the same content
        // produce the same bytes. If this fails, every other golden test
        // is noise.
        let draw = |renderer: &mut Renderer| {
            <Renderer as taim_core::Renderer>::fill_quad(
                renderer,
                taim_core::renderer::Quad {
                    bounds: Rectangle::new(taim_core::Point::new(4.0, 4.0), Size::new(24.0, 16.0)),
                    ..Default::default()
                },
                Color::WHITE,
            );
        };

        let first = Harness::render(Size::new(32, 32), 1.0, draw).expect("tiny-skia renders");
        let second = Harness::render(Size::new(32, 32), 1.0, draw).expect("tiny-skia renders");

        assert_eq!(first, second, "the renderer must be deterministic");
    }

    #[test]
    fn a_rendered_quad_actually_draws_something() {
        // Guards the guard: a harness whose `render` silently produced a
        // blank image would make every exact case pass forever.
        let image = Harness::render(Size::new(32, 32), 1.0, |renderer| {
            <Renderer as taim_core::Renderer>::fill_quad(
                renderer,
                taim_core::renderer::Quad {
                    bounds: Rectangle::new(taim_core::Point::ORIGIN, Size::new(32.0, 32.0)),
                    ..Default::default()
                },
                Color::WHITE,
            );
        })
        .expect("tiny-skia renders");

        assert!(
            image.rgba.iter().any(|channel| *channel > 250),
            "a white quad over the whole surface must produce white pixels"
        );
    }
}
