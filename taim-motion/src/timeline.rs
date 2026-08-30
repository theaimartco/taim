//! Timeline sequencing, staggering, pause, reverse, and seek.

/// Playback direction for a [`Timeline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Time increases toward the end.
    Forward,
    /// Time decreases toward the beginning.
    Reverse,
}

/// A normalized interval inside a timeline, useful for sequencing/staggering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Start time in seconds.
    pub start: f64,
    /// Duration in seconds.
    pub duration: f64,
}

impl Segment {
    /// Returns the local normalized progress for this segment.
    pub fn progress(self, time: f64) -> f32 {
        if self.duration <= 0.0 {
            return if time >= self.start { 1.0 } else { 0.0 };
        }
        ((time - self.start) / self.duration).clamp(0.0, 1.0) as f32
    }
}

/// A finite timeline driven only by elapsed shared-clock time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    duration: f64,
    position: f64,
    direction: Direction,
    paused: bool,
}

impl Timeline {
    /// Creates a timeline positioned at its beginning.
    pub fn new(duration: f64) -> Self {
        Self {
            duration: duration.max(0.0),
            position: 0.0,
            direction: Direction::Forward,
            paused: false,
        }
    }

    /// Returns total duration in seconds.
    pub const fn duration(&self) -> f64 {
        self.duration
    }

    /// Returns current position in seconds.
    pub const fn position(&self) -> f64 {
        self.position
    }

    /// Returns normalized current progress.
    pub fn progress(&self) -> f32 {
        Segment {
            start: 0.0,
            duration: self.duration,
        }
        .progress(self.position)
    }

    /// Advances this timeline by elapsed seconds.
    pub fn advance(&mut self, seconds: f64) {
        if self.paused {
            return;
        }
        let seconds = seconds.max(0.0);
        self.position = match self.direction {
            Direction::Forward => (self.position + seconds).min(self.duration),
            Direction::Reverse => (self.position - seconds).max(0.0),
        };
    }

    /// Seeks to a position, clamped to the timeline duration.
    pub fn seek(&mut self, position: f64) {
        self.position = position.clamp(0.0, self.duration);
    }

    /// Pauses advancement.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resumes advancement.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Reverses playback while preserving current position.
    pub fn reverse(&mut self) {
        self.direction = match self.direction {
            Direction::Forward => Direction::Reverse,
            Direction::Reverse => Direction::Forward,
        };
    }

    /// Returns whether this timeline has reached the active direction's end.
    pub fn is_complete(&self) -> bool {
        match self.direction {
            Direction::Forward => self.position >= self.duration,
            Direction::Reverse => self.position <= 0.0,
        }
    }
}
