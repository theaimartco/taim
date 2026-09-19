use super::*;

/// An horizontal bar and a handle that selects a single value from a range of
/// values.
///
/// A [`Slider`] will try to fill the horizontal space of its container.
///
/// The [`Slider`] range of numeric values is generic and its step size defaults
/// to 1 unit.
///
/// Note: Under the hood values are converted to/from f64 so only values representable exactly as an f64
/// are possible to select via the slider. However it is likely that the precision of the slider at these
/// scales is already less than the precision lost from the f64 representation.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::widget::slider;
///
/// struct State {
///    value: f32,
/// }
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     ValueChanged(f32),
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     slider(0.0..=100.0, state.value, Message::ValueChanged).into()
/// }
///
/// fn update(state: &mut State, message: Message) {
///     match message {
///         Message::ValueChanged(value) => {
///             state.value = value;
///         }
///     }
/// }
/// ```
pub struct Slider<'a, T, Message, Theme = crate::Theme>
where
    Theme: Catalog,
{
    pub(super) range: RangeInclusive<T>,
    pub(super) step: f64,
    pub(super) shift_step: Option<f64>,
    pub(super) value: T,
    pub(super) default: Option<T>,
    pub(super) on_change: Box<dyn Fn(T) -> Message + 'a>,
    pub(super) on_release: Option<Message>,
    pub(super) width: Length,
    pub(super) height: f32,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, T, Message, Theme> Slider<'a, T, Message, Theme>
where
    T: Copy + PartialOrd,
    Message: Clone,
    Theme: Catalog,
{
    /// The default height of a [`Slider`].
    pub const DEFAULT_HEIGHT: f32 = 16.0;

    /// Creates a new [`Slider`].
    ///
    /// It expects:
    ///   * an inclusive range of possible values
    ///   * the current value of the [`Slider`]
    ///   * a function that will be called when the [`Slider`] is dragged.
    ///     It receives the new value of the [`Slider`] and must produce a
    ///     `Message`.
    pub fn new<F>(range: RangeInclusive<T>, value: T, on_change: F) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        let value = if value >= *range.start() {
            value
        } else {
            *range.start()
        };

        let value = if value <= *range.end() {
            value
        } else {
            *range.end()
        };

        Slider {
            value,
            default: None,
            range,
            step: 1.0,
            shift_step: None,
            on_change: Box::new(on_change),
            on_release: None,
            width: Length::Fill,
            height: Self::DEFAULT_HEIGHT,
            class: Theme::default(),
        }
    }

    /// Sets the optional default value for the [`Slider`].
    ///
    /// If set, the [`Slider`] will reset to this value when ctrl-clicked or command-clicked.
    pub fn default(mut self, default: impl Into<T>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Sets the release message of the [`Slider`].
    /// This is called when the mouse is released from the slider.
    ///
    /// Typically, the user's interaction with the slider is finished when this message is produced.
    /// This is useful if you need to spawn a long-running task from the slider's result, where
    /// the default on_change message could create too many events.
    pub fn on_release(mut self, on_release: Message) -> Self {
        self.on_release = Some(on_release);
        self
    }

    /// Sets the width of the [`Slider`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Slider`].
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into().0;
        self
    }

    /// Sets the step size of the [`Slider`].
    pub fn step(mut self, step: impl num_traits::AsPrimitive<f64>) -> Self {
        self.step = step.as_();
        self
    }

    /// Sets the optional "shift" step for the [`Slider`].
    ///
    /// If set, this value is used as the step while the shift key is pressed.
    pub fn shift_step(mut self, shift_step: impl num_traits::AsPrimitive<f64>) -> Self {
        self.shift_step = Some(shift_step.as_());
        self
    }

    /// Sets the style of the [`Slider`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Slider`].
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}
