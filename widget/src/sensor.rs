//! Observe when content approaches, enters, resizes within, or leaves a viewport.
mod implementation;
mod key;

pub use key::Key;

use crate::core::time::Duration;
use crate::core::{self, Element, Pixels, Size};

use key::Owned;

/// A retained viewport-presence sensor around one content widget.
pub struct Sensor<'a, Key, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    pub(super) content: Element<'a, Message, Theme, Renderer>,
    pub(super) key: Key,
    pub(super) on_show: Option<Box<dyn Fn(Size) -> Message + 'a>>,
    pub(super) on_resize: Option<Box<dyn Fn(Size) -> Message + 'a>>,
    pub(super) on_hide: Option<Message>,
    pub(super) anticipate: Pixels,
    pub(super) delay: Duration,
}

impl<'a, Message, Theme, Renderer> Sensor<'a, (), Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    /// Creates a sensor around content.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            key: (),
            on_show: None,
            on_resize: None,
            on_hide: None,
            anticipate: Pixels::ZERO,
            delay: Duration::ZERO,
        }
    }
}

impl<'a, KeyType, Message, Theme, Renderer> Sensor<'a, KeyType, Message, Theme, Renderer>
where
    KeyType: Key,
    Renderer: core::Renderer,
{
    /// Publishes a message when content enters the anticipated viewport.
    pub fn on_show(mut self, on_show: impl Fn(Size) -> Message + 'a) -> Self {
        self.on_show = Some(Box::new(on_show));
        self
    }

    /// Publishes a message when visible content changes size.
    pub fn on_resize(mut self, on_resize: impl Fn(Size) -> Message + 'a) -> Self {
        self.on_resize = Some(Box::new(on_resize));
        self
    }

    /// Publishes a message when content leaves the anticipated viewport.
    pub fn on_hide(mut self, on_hide: Message) -> Self {
        self.on_hide = Some(on_hide);
        self
    }

    /// Sets an owned continuity key.
    pub fn key<K>(self, key: K) -> Sensor<'a, impl Key, Message, Theme, Renderer>
    where
        K: Clone + PartialEq + 'static,
    {
        Sensor {
            content: self.content,
            key: Owned(key),
            on_show: self.on_show,
            on_resize: self.on_resize,
            on_hide: self.on_hide,
            anticipate: self.anticipate,
            delay: self.delay,
        }
    }

    /// Sets a borrowed continuity key.
    pub fn key_ref<K>(self, key: &'a K) -> Sensor<'a, &'a K, Message, Theme, Renderer>
    where
        K: ToOwned + PartialEq<K::Owned> + ?Sized,
        K::Owned: 'static,
    {
        Sensor {
            content: self.content,
            key,
            on_show: self.on_show,
            on_resize: self.on_resize,
            on_hide: self.on_hide,
            anticipate: self.anticipate,
            delay: self.delay,
        }
    }

    /// Extends visibility detection by a nonnegative logical-pixel distance.
    pub fn anticipate(mut self, distance: impl Into<Pixels>) -> Self {
        let distance = distance.into().0;
        self.anticipate = Pixels(if distance.is_finite() {
            distance.max(0.0)
        } else {
            0.0
        });
        self
    }

    /// Debounces show and hide notifications.
    pub fn delay(mut self, delay: impl Into<Duration>) -> Self {
        self.delay = delay.into();
        self
    }
}
