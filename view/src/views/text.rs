use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::Constraints;
use crate::state::{IntoProp, Prop};
use crate::{Color, Pixels, Renderer, Size, View, semantics};

use taim_core::Renderer as _;
use taim_core::text::{self, Paragraph as _, Renderer as _};
use taim_core::widget;

type Paragraph = <Renderer as text::Renderer>::Paragraph;

/// A line or paragraph of text: `text("Hello")`, or
/// `text(move || format!("{}", count.get()))` to follow state.
pub fn text(content: impl IntoProp<String>) -> Text {
    Text::from_prop(content.into_prop())
}

impl Text {
    pub(crate) fn from_prop(content: Prop<String>) -> Self {
        Self {
            content,
            size: None,
            color: None,
            font: None,
            shaped: None,
            id: widget::Id::unique(),
        }
    }
}

/// See [`text`].
pub struct Text {
    content: Prop<String>,
    size: Option<Pixels>,
    color: Option<Color>,
    font: Option<taim_core::Font>,
    shaped: Option<Paragraph>,
    id: widget::Id,
}

impl Text {
    /// Font size in logical pixels.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// The font; the renderer's default when unset.
    pub fn font(mut self, font: taim_core::Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Text color; the window's text color when unset.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl View for Text {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let renderer = cx.renderer();
        let content = self.content.get();

        let shaped = Paragraph::with_text(text::Text {
            content: content.as_str(),
            bounds: constraints.max,
            size: self.size.unwrap_or_else(|| renderer.default_size()),
            line_height: text::LineHeight::default(),
            font: self.font.unwrap_or_else(|| renderer.default_font()),
            align_x: text::Alignment::Default,
            align_y: taim_core::alignment::Vertical::Top,
            shaping: text::Shaping::default(),
            wrapping: text::Wrapping::Word,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        });

        let size = shaped.min_bounds();
        self.shaped = Some(shaped);
        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let Some(shaped) = &self.shaped else {
            return;
        };

        let bounds = cx.bounds();
        let color = self.color.unwrap_or_else(|| cx.text_color());
        cx.renderer()
            .fill_paragraph(shaped, bounds.position(), color, bounds);
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::StaticText)
                .with_name(self.content.get()),
        )
    }
}
