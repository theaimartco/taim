//! Direct Wayland connection and dispatch.

use std::collections::{BTreeMap, VecDeque};

use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::capabilities::{Capabilities, Capability};
use crate::surface::{Damage, SurfaceId, SurfaceLifecycle, SurfaceState};

/// Native backend initialization or dispatch error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The Wayland display could not be opened.
    #[error("could not connect to the Wayland compositor: {0}")]
    Connect(#[from] wayland_client::ConnectError),
    /// Initial global discovery failed.
    #[error("could not initialize the Wayland registry: {0}")]
    Registry(#[from] wayland_client::globals::GlobalError),
    /// An event could not be decoded or dispatched.
    #[error("Wayland dispatch failed: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),
    /// A required core protocol was not advertised.
    #[error("required compositor capability is unavailable: {0:?}")]
    MissingCapability(Capability),
    /// A required global could not be bound at a compatible version.
    #[error("could not bind a required compositor global: {0}")]
    Bind(#[from] wayland_client::globals::BindError),
    /// No further stable surface identity can be allocated.
    #[error("surface identity space is exhausted")]
    SurfaceIdExhausted,
}

/// Event emitted by the native backend.
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent {
    /// The compositor's dynamic global set changed.
    GlobalsChanged(Capabilities),
    /// A surface configuration was acknowledged.
    Configured {
        /// Configured surface.
        surface: SurfaceId,
        /// Wayland configure serial.
        serial: u32,
        /// Logical width.
        width: u32,
        /// Logical height.
        height: u32,
    },
    /// The compositor requested that a surface close.
    CloseRequested(SurfaceId),
}

struct State {
    capabilities: Capabilities,
    surfaces: BTreeMap<SurfaceId, SurfaceLifecycle>,
    events: VecDeque<PlatformEvent>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        globals: &GlobalListContents,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        let capabilities = Capabilities::from(globals.clone_list());
        if capabilities != state.capabilities {
            state.capabilities = capabilities.clone();
            state
                .events
                .push_back(PlatformEvent::GlobalsChanged(capabilities));
        }
    }
}

wayland_client::delegate_noop!(State: ignore wl_compositor::WlCompositor);

impl Dispatch<wl_surface::WlSurface, SurfaceId> for State {
    fn event(
        _state: &mut Self,
        _surface: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _id: &SurfaceId,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _state: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _data: &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, SurfaceId> for State {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        id: &SurfaceId,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            if let Some(lifecycle) = state.surfaces.get_mut(id) {
                let (width, height) = match lifecycle.state() {
                    SurfaceState::Configured { width, height, .. } => (width, height),
                    _ => (1, 1),
                };
                let _ = lifecycle.configure(serial, width, height);
                state.events.push_back(PlatformEvent::Configured {
                    surface: *id,
                    serial,
                    width,
                    height,
                });
            }
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, SurfaceId> for State {
    fn event(
        state: &mut Self,
        _toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        id: &SurfaceId,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0
                    && height > 0
                    && let Some(lifecycle) = state.surfaces.get_mut(id)
                {
                    let serial = lifecycle.last_acked_serial().unwrap_or(0);
                    let _ = lifecycle.configure(serial, width as u32, height as u32);
                }
            }
            xdg_toplevel::Event::Close => {
                if let Some(lifecycle) = state.surfaces.get_mut(id) {
                    lifecycle.close();
                }
                state.events.push_back(PlatformEvent::CloseRequested(*id));
            }
            _ => {}
        }
    }
}

/// Live Wayland objects associated with an Aura toplevel.
#[derive(Debug)]
pub struct Toplevel {
    /// Stable framework identity.
    pub id: SurfaceId,
    /// Core Wayland surface proxy.
    pub surface: wl_surface::WlSurface,
    /// XDG configure/ack proxy.
    pub xdg_surface: xdg_surface::XdgSurface,
    /// XDG toplevel role proxy.
    pub toplevel: xdg_toplevel::XdgToplevel,
}

impl Toplevel {
    /// Performs the empty commit required to receive the first configure.
    pub fn initial_commit(&self) {
        self.surface.commit();
    }

    /// Submits `damage` and commits, telling the compositor which pixels
    /// actually changed.
    ///
    /// This is the bandwidth half of partial present. `aura_wgpu` already
    /// scissors *rasterisation* to the damaged region, but every frame is
    /// still handed to the compositor whole, so it re-reads and re-composites
    /// the entire surface. `wl_surface.damage_buffer` is what stops that, and
    /// on a device that is where the power goes.
    ///
    /// `lifecycle` clips the region and decides whether it may be submitted
    /// at all: a damage rectangle outside the surface is a protocol error
    /// that ends the connection, not a request the compositor ignores.
    ///
    /// Passing no damage commits the whole surface, which is the correct
    /// fallback whenever the region is unknown.
    pub fn commit_with_damage(&self, lifecycle: &SurfaceLifecycle, damage: Option<Damage>) {
        match damage.and_then(|damage| lifecycle.clip_damage(damage)) {
            Some(damage) => {
                self.surface.damage_buffer(
                    damage.x as i32,
                    damage.y as i32,
                    damage.width as i32,
                    damage.height as i32,
                );
            }
            // No damage submitted at all means "the whole surface changed",
            // which is what the protocol assumes and what an unknown region
            // must fall back to.
            None => {
                self.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            }
        }

        self.surface.commit();
    }
}

/// Native backend connection.
pub struct Backend {
    connection: Connection,
    globals: GlobalList,
    queue: EventQueue<State>,
    state: State,
    compositor: wl_compositor::WlCompositor,
    shell: xdg_wm_base::XdgWmBase,
    next_surface: u64,
}

impl Backend {
    /// Connects to the display named by the standard Wayland environment.
    pub fn connect() -> Result<Self, Error> {
        let connection = Connection::connect_to_env()?;
        let (globals, queue) = registry_queue_init::<State>(&connection)?;
        let capabilities = Capabilities::from(globals.contents().clone_list());

        if let Some(missing) = capabilities.missing_core().next() {
            return Err(Error::MissingCapability(missing));
        }

        let handle = queue.handle();
        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(
            &handle,
            4..=wl_compositor::WlCompositor::interface().version,
            (),
        )?;
        let shell = globals.bind::<xdg_wm_base::XdgWmBase, _, _>(
            &handle,
            1..=xdg_wm_base::XdgWmBase::interface().version,
            (),
        )?;

        Ok(Self {
            connection,
            globals,
            queue,
            state: State {
                capabilities,
                surfaces: BTreeMap::new(),
                events: VecDeque::new(),
            },
            compositor,
            shell,
            next_surface: 1,
        })
    }

    /// Returns the latest negotiated protocol snapshot.
    pub fn capabilities(&self) -> &Capabilities {
        &self.state.capabilities
    }

    /// Creates an xdg-shell toplevel and performs the empty initial commit.
    pub fn create_toplevel(&mut self, title: &str) -> Result<Toplevel, Error> {
        let id = SurfaceId::new(self.next_surface).ok_or(Error::SurfaceIdExhausted)?;
        self.next_surface = self
            .next_surface
            .checked_add(1)
            .ok_or(Error::SurfaceIdExhausted)?;

        let handle = self.queue.handle();
        let surface = self.compositor.create_surface(&handle, id);
        let xdg_surface = self.shell.get_xdg_surface(&surface, &handle, id);
        let toplevel = xdg_surface.get_toplevel(&handle, id);
        toplevel.set_title(title.to_owned());
        surface.commit();

        let _ = self.state.surfaces.insert(id, SurfaceLifecycle::new(id));

        Ok(Toplevel {
            id,
            surface,
            xdg_surface,
            toplevel,
        })
    }

    /// Blocks until at least one compositor event is dispatched.
    ///
    /// This is the idle path: it performs no polling loop and therefore
    /// consumes no CPU while the compositor is quiet.
    pub fn dispatch_blocking(&mut self) -> Result<usize, Error> {
        Ok(self.queue.blocking_dispatch(&mut self.state)?)
    }

    /// Dispatches events already read from the Wayland socket.
    pub fn dispatch_pending(&mut self) -> Result<usize, Error> {
        Ok(self.queue.dispatch_pending(&mut self.state)?)
    }

    /// Flushes pending client requests without blocking.
    pub fn flush(&self) -> Result<(), wayland_client::backend::WaylandError> {
        self.connection.flush()
    }

    /// Removes and returns the oldest translated platform event.
    pub fn next_event(&mut self) -> Option<PlatformEvent> {
        self.state.events.pop_front()
    }

    /// Returns lifecycle state for a known surface.
    pub fn surface(&self, id: SurfaceId) -> Option<&SurfaceLifecycle> {
        self.state.surfaces.get(&id)
    }

    /// Returns the number of currently advertised global instances.
    pub fn advertised_globals(&self) -> usize {
        self.globals
            .contents()
            .with_list(<[wayland_client::globals::Global]>::len)
    }
}
