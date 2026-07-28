//! Type ramp and language-aware line metrics.
#![allow(missing_docs)]
use aura_core::text::Script;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextStyle {
    Label,
    BodySmall,
    #[default]
    Body,
    Title,
    Headline,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
}
impl Typography {
    pub const fn style(style: TextStyle, script: Script) -> Self {
        let (s, h, w) = match style {
            TextStyle::Label => (12.0, 16.0, 600),
            TextStyle::BodySmall => (14.0, 20.0, 400),
            TextStyle::Body => (16.0, 24.0, 400),
            TextStyle::Title => (22.0, 28.0, 600),
            TextStyle::Headline => (32.0, 40.0, 700),
        };
        let extra = match script {
            Script::Devanagari
            | Script::Gurmukhi
            | Script::Arabic
            | Script::Bengali
            | Script::Tamil => 2.0,
            _ => 0.0,
        };
        Self {
            size: s,
            line_height: h + extra,
            weight: w,
        }
    }
}
