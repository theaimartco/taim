//! Bounded, non-blocking clipboard bridge.

use crate::core::clipboard::{Content, Error, Kind};

pub use platform::*;

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
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
            let Ok(clipboard) = arboard::Clipboard::new() else {
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

    fn run_worker(mut clipboard: arboard::Clipboard, receiver: mpsc::Receiver<Command>) {
        while let Ok(command) = receiver.recv() {
            match command {
                Command::Read { kind, callback } => {
                    let result = read_content(&mut clipboard, kind);
                    invoke(|| callback(result));
                }
                Command::Write { content, callback } => {
                    let result = write_content(&mut clipboard, content);
                    invoke(|| callback(result));
                }
            }
        }
    }

    fn read_content(clipboard: &mut arboard::Clipboard, kind: Kind) -> Result<Content, Error> {
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

    fn write_content(clipboard: &mut arboard::Clipboard, content: Content) -> Result<(), Error> {
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
            _ => {
                log::warn!("unsupported clipboard content type");
                Err(arboard::Error::ClipboardNotSupported)
            }
        };

        result.map_err(to_error)
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

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;

    /// Explicitly unavailable clipboard for targets without a native adapter.
    pub struct Clipboard;

    impl Clipboard {
        /// Creates an unavailable clipboard bridge.
        pub fn new() -> Self {
            Self
        }

        /// Reports that platform clipboard reads are not wired yet.
        pub fn read(&self, _kind: Kind, callback: impl FnOnce(Result<Content, Error>)) {
            callback(Err(Error::ClipboardUnavailable));
        }

        /// Reports that platform clipboard writes are not wired yet.
        pub fn write(&mut self, _content: Content, callback: impl FnOnce(Result<(), Error>)) {
            callback(Err(Error::ClipboardUnavailable));
        }
    }
}

#[cfg(target_os = "android")]
mod platform {
    use super::*;
    use crate::android::{Request, services};
    use std::sync::mpsc::{self, SyncSender};
    type Read = Box<dyn FnOnce(Result<Content, Error>) + Send>;
    type Write = Box<dyn FnOnce(Result<(), Error>) + Send>;
    enum Command {
        Read(Kind, Read),
        Write(Content, Write),
    }
    /// Foreground Android ClipboardManager adapter with a bounded worker queue.
    pub struct Clipboard {
        commands: Option<SyncSender<Command>>,
    }
    impl Clipboard {
        /// Creates the native clipboard worker. Unsupported formats fail explicitly.
        pub fn new() -> Self {
            let (commands, receive) = mpsc::sync_channel(8);
            let worker = std::thread::Builder::new()
                .name("taim-android-clipboard".into())
                .spawn(move || {
                    while let Ok(command) = receive.recv() {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let client = services();
                            match command {
                                Command::Read(kind, callback) => {
                                    let html = match kind {
                                        Kind::Text => false,
                                        Kind::Html => true,
                                        _ => {
                                            callback(Err(Error::ContentNotAvailable));
                                            return;
                                        }
                                    };
                                    let result = client
                                        .ok_or(Error::ClipboardUnavailable)
                                        .and_then(|client| {
                                            futures::executor::block_on(
                                                client.request(Request::ClipboardRead { html }),
                                            )
                                            .map_err(|_| Error::ClipboardUnavailable)
                                        })
                                        .and_then(|value| {
                                            value
                                                .as_str()
                                                .map(|text| {
                                                    if html {
                                                        Content::Html(text.into())
                                                    } else {
                                                        Content::Text(text.into())
                                                    }
                                                })
                                                .ok_or(Error::ContentNotAvailable)
                                        });
                                    callback(result);
                                }
                                Command::Write(content, callback) => {
                                    let (text, html) = match content {
                                        Content::Text(text) => (text, None),
                                        Content::Html(text) => (String::new(), Some(text)),
                                        _ => {
                                            callback(Err(Error::ContentNotAvailable));
                                            return;
                                        }
                                    };
                                    if text.len() + html.as_ref().map_or(0, String::len)
                                        > 4 * 1024 * 1024
                                    {
                                        callback(Err(Error::ConversionFailure));
                                        return;
                                    }
                                    let result = client
                                        .ok_or(Error::ClipboardUnavailable)
                                        .and_then(|client| {
                                            futures::executor::block_on(client.request(
                                                Request::ClipboardWrite {
                                                    text,
                                                    html,
                                                    sensitive: false,
                                                },
                                            ))
                                            .map(|_| ())
                                            .map_err(|_| Error::ClipboardUnavailable)
                                        });
                                    callback(result);
                                }
                            }
                        }));
                    }
                });
            Self {
                commands: worker.ok().map(|_| commands),
            }
        }
        /// Reads foreground text/HTML without blocking the event loop.
        pub fn read(
            &self,
            kind: Kind,
            callback: impl FnOnce(Result<Content, Error>) + Send + 'static,
        ) {
            self.enqueue(Command::Read(kind, Box::new(callback)));
        }
        /// Writes text/HTML without blocking the event loop.
        pub fn write(
            &mut self,
            content: Content,
            callback: impl FnOnce(Result<(), Error>) + Send + 'static,
        ) {
            self.enqueue(Command::Write(content, Box::new(callback)));
        }
        fn enqueue(&self, command: Command) {
            let failed = match &self.commands {
                Some(send) => send.try_send(command).err().map(|error| match error {
                    mpsc::TrySendError::Full(command)
                    | mpsc::TrySendError::Disconnected(command) => command,
                }),
                None => Some(command),
            };
            if let Some(command) = failed {
                match command {
                    Command::Read(_, callback) => callback(Err(Error::ClipboardOccupied)),
                    Command::Write(_, callback) => callback(Err(Error::ClipboardOccupied)),
                }
            }
        }
    }
}
