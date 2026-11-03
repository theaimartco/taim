//! Bounded, non-blocking clipboard bridge.

use crate::core::clipboard::{Content, Error, Kind};

pub use platform::*;

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::*;

    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Arc;
    use std::sync::mpsc::{self, SyncSender, TrySendError};
    use std::thread;

    const QUEUE_CAPACITY: usize = 8;
    const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
    #[cfg(feature = "image")]
    const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;
    const MAX_FILES: usize = 4_096;

    type ReadCallback = Box<dyn FnOnce(Result<Content, Error>) + Send + 'static>;
    type WriteCallback = Box<dyn FnOnce(Result<(), Error>) + Send + 'static>;

    /// A single-worker clipboard bridge with bounded command storage.
    pub struct Clipboard {
        state: State,
    }

    enum State {
        Connected { commands: SyncSender<Command> },
        Unavailable,
    }

    enum Command {
        Read {
            kind: Kind,
            callback: ReadCallback,
        },
        Write {
            content: Content,
            callback: WriteCallback,
        },
    }

    impl Command {
        fn reject(self, error: Error) {
            match self {
                Self::Read { callback, .. } => invoke(|| callback(Err(error))),
                Self::Write { callback, .. } => invoke(|| callback(Err(error))),
            }
        }
    }

    impl Clipboard {
        /// Creates a clipboard connection and its one bounded worker.
        pub fn new() -> Self {
            let Some(clipboard) = native::connect() else {
                return Self {
                    state: State::Unavailable,
                };
            };

            let (commands, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
            let worker = thread::Builder::new()
                .name("taim-clipboard".into())
                .spawn(move || run_worker(clipboard, receiver));

            let state = match worker {
                Ok(_worker) => State::Connected { commands },
                Err(error) => {
                    log::warn!("could not start TAIM clipboard worker: {error}");
                    State::Unavailable
                }
            };

            Self { state }
        }

        /// Queues a clipboard read without blocking the UI thread.
        pub fn read(
            &self,
            kind: Kind,
            callback: impl FnOnce(Result<Content, Error>) + Send + 'static,
        ) {
            let State::Connected { commands } = &self.state else {
                callback(Err(Error::ClipboardUnavailable));
                return;
            };

            enqueue(
                commands,
                Command::Read {
                    kind,
                    callback: Box::new(callback),
                },
            );
        }

        /// Queues a clipboard write without blocking the UI thread.
        pub fn write(
            &mut self,
            content: Content,
            callback: impl FnOnce(Result<(), Error>) + Send + 'static,
        ) {
            if let Err(error) = validate_content(&content) {
                callback(Err(error));
                return;
            }

            let State::Connected { commands } = &self.state else {
                callback(Err(Error::ClipboardUnavailable));
                return;
            };

            enqueue(
                commands,
                Command::Write {
                    content,
                    callback: Box::new(callback),
                },
            );
        }
    }

    fn enqueue(commands: &SyncSender<Command>, command: Command) {
        match commands.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(command)) => command.reject(Error::ClipboardOccupied),
            Err(TrySendError::Disconnected(command)) => {
                command.reject(Error::ClipboardUnavailable);
            }
        }
    }

    fn run_worker(mut clipboard: native::Native, receiver: mpsc::Receiver<Command>) {
        while let Ok(command) = receiver.recv() {
            match command {
                Command::Read { kind, callback } => {
                    let result = native::read_content(&mut clipboard, kind);
                    invoke(|| callback(result));
                }
                Command::Write { content, callback } => {
                    let result = native::write_content(&mut clipboard, content);
                    invoke(|| callback(result));
                }
            }
        }
    }

    fn validate_content(content: &Content) -> Result<(), Error> {
        match content {
            Content::Text(text) | Content::Html(text) if text.len() > MAX_TEXT_BYTES => {
                Err(too_large("clipboard text exceeds the 64 MiB TAIM limit"))
            }
            #[cfg(feature = "image")]
            Content::Image(image) => {
                let expected = image_byte_len(image.size.width, image.size.height)?;
                if expected != image.rgba.len() {
                    Err(Error::ConversionFailure)
                } else {
                    Ok(())
                }
            }
            Content::Files(files) if files.len() > MAX_FILES => Err(too_large(
                "clipboard file list exceeds the 4096-item TAIM limit",
            )),
            _ => Ok(()),
        }
    }

    #[cfg(feature = "image")]
    fn image_byte_len(width: u32, height: u32) -> Result<usize, Error> {
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| too_large("clipboard image dimensions overflow address space"))?;

        if bytes > MAX_IMAGE_BYTES {
            Err(too_large("clipboard image exceeds the 256 MiB TAIM limit"))
        } else {
            Ok(bytes)
        }
    }

    fn invoke(callback: impl FnOnce()) {
        let _ = catch_unwind(AssertUnwindSafe(callback));
    }

    fn too_large(description: &str) -> Error {
        Error::Unknown {
            description: Arc::new(description.to_owned()),
        }
    }

    /// The desktop clipboard, through `arboard`.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    mod native {
        use super::*;

        pub(super) type Native = arboard::Clipboard;

        pub(super) fn connect() -> Option<Native> {
            arboard::Clipboard::new().ok()
        }

        pub(super) fn read_content(
            clipboard: &mut arboard::Clipboard,
            kind: Kind,
        ) -> Result<Content, Error> {
            match kind {
                Kind::Text => clipboard.get().text().map(Content::Text).map_err(to_error),
                Kind::Html => clipboard.get().html().map(Content::Html).map_err(to_error),
                #[cfg(feature = "image")]
                Kind::Image => clipboard
                    .get()
                    .image()
                    .map_err(to_error)
                    .and_then(image_content),
                Kind::Files => clipboard
                    .get()
                    .file_list()
                    .map(Content::Files)
                    .map_err(to_error),
                kind => {
                    log::warn!("unsupported clipboard kind: {kind:?}");
                    Err(Error::ContentNotAvailable)
                }
            }
        }

        pub(super) fn write_content(
            clipboard: &mut arboard::Clipboard,
            content: Content,
        ) -> Result<(), Error> {
            let set = clipboard.set();
            let result = match content {
                Content::Text(text) => set.text(text),
                Content::Html(html) => set.html(html, None),
                #[cfg(feature = "image")]
                Content::Image(image) => set.image(arboard::ImageData {
                    bytes: image.rgba.as_ref().into(),
                    width: image.size.width as usize,
                    height: image.size.height as usize,
                }),
                Content::Files(files) => set.file_list(&files),
                content => {
                    log::warn!("unsupported clipboard content: {content:?}");
                    Err(arboard::Error::ClipboardNotSupported)
                }
            };

            result.map_err(to_error)
        }

        #[cfg(feature = "image")]
        fn image_content(image: arboard::ImageData<'static>) -> Result<Content, Error> {
            let width = u32::try_from(image.width)
                .map_err(|_| too_large("clipboard image width does not fit in u32"))?;
            let height = u32::try_from(image.height)
                .map_err(|_| too_large("clipboard image height does not fit in u32"))?;
            let expected = image_byte_len(width, height)?;

            if expected != image.bytes.len() {
                return Err(Error::ConversionFailure);
            }

            Ok(Content::Image(crate::core::clipboard::Image {
                rgba: crate::core::Bytes::from_owner(image.bytes),
                size: crate::core::Size::new(width, height),
            }))
        }

        fn to_error(error: arboard::Error) -> Error {
            match error {
                arboard::Error::ContentNotAvailable => Error::ContentNotAvailable,
                arboard::Error::ClipboardNotSupported => Error::ClipboardUnavailable,
                arboard::Error::ClipboardOccupied => Error::ClipboardOccupied,
                arboard::Error::ConversionFailure => Error::ConversionFailure,
                arboard::Error::Unknown { description } => Error::Unknown {
                    description: Arc::new(description),
                },
                error => Error::Unknown {
                    description: Arc::new(error.to_string()),
                },
            }
        }
    }

    /// iOS: `UIPasteboard` is not connected yet, so the clipboard reports
    /// itself unavailable rather than pretending to copy.
    #[cfg(target_os = "ios")]
    mod native {
        use super::*;

        pub(super) struct Native;

        pub(super) fn connect() -> Option<Native> {
            None
        }

        pub(super) fn read_content(_: &mut Native, _: Kind) -> Result<Content, Error> {
            Err(Error::ClipboardUnavailable)
        }

        pub(super) fn write_content(_: &mut Native, _: Content) -> Result<(), Error> {
            Err(Error::ClipboardUnavailable)
        }
    }

    /// The Android clipboard, through `ClipboardManager` over JNI.
    #[cfg(target_os = "android")]
    #[path = "android.rs"]
    mod native;
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;

    /// Clipboard placeholder for targets without a browser adapter yet.
    pub struct Clipboard;

    impl Clipboard {
        /// Creates an unavailable clipboard bridge.
        pub fn new() -> Self {
            Self
        }

        /// Reports that browser clipboard reads are not wired yet.
        pub fn read(&self, _kind: Kind, callback: impl FnOnce(Result<Content, Error>)) {
            callback(Err(Error::ClipboardUnavailable));
        }

        /// Reports that browser clipboard writes are not wired yet.
        pub fn write(&mut self, _content: Content, callback: impl FnOnce(Result<(), Error>)) {
            callback(Err(Error::ClipboardUnavailable));
        }
    }
}
