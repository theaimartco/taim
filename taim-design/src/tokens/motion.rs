//! Semantic motion tied to Taim Motion.
#![allow(missing_docs)]
use taim_motion::SpringConfig;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Feedback,
    Hierarchy,
    Gesture,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionTokens {
    pub feedback_ms: u16,
    pub hierarchy_ms: u16,
    pub gesture_spring: SpringConfig,
}
impl Default for MotionTokens {
    fn default() -> Self {
        Self {
            feedback_ms: 120,
            hierarchy_ms: 240,
            gesture_spring: SpringConfig {
                angular_frequency: 16.0,
                damping_ratio: 0.9,
                epsilon: 0.001,
            },
        }
    }
}
impl MotionTokens {
    pub const fn duration_ms(self, kind: TransitionKind) -> u16 {
        match kind {
            TransitionKind::Feedback => self.feedback_ms,
            TransitionKind::Hierarchy => self.hierarchy_ms,
            TransitionKind::Gesture => 0,
        }
    }
}
