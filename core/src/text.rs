//! Draw and interact with text.
pub mod editor;
pub mod fallback;
pub mod highlighter;
pub mod line_break;
pub mod paragraph;
pub mod selection;

pub use editor::Editor;
pub use fallback::{FallbackChain, Script};
pub use highlighter::Highlighter;
pub use line_break::{LineBreak, LineBreakPolicy};
pub use paragraph::Paragraph;
pub use selection::{Selection, SelectionGeometry};

use crate::alignment;
use crate::{Background, Border, Color, Padding, Pixels, Point, Rectangle, Size};

use std::borrow::Cow;
use std::hash::{Hash, Hasher};

mod model;
mod renderer;
mod span;

pub use model::{Alignment, Difference, Ellipsis, Hit, LineHeight, Shaping, Text, Wrapping};
pub use renderer::Renderer;
pub use span::{Fragment, Highlight, IntoFragment, Span};
