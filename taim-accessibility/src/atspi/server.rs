//! The D-Bus transport: what a screen reader actually talks to.
//!
//! # The shape of AT-SPI2
//!
//! An application does not push its tree anywhere. It **serves** it: it
//! exposes a D-Bus object per accessible node, registers its root with
//! the accessibility registry, and answers questions — `GetChildren`,
//! `GetRole`, `GetState` — whenever a reader asks. Events are a hint that
//! something changed and worth re-asking about, not the data itself.
//!
//! That inversion is why [`Store`] exists: the reader's questions arrive
//! on the bus task, at times unrelated to our frames, and must be
//! answered from a consistent snapshot rather than from a tree the UI
//! thread is halfway through rewriting.
//!
//! # The bus is not the session bus
//!
//! AT-SPI runs on its own bus. The address is found by asking
//! `org.a11y.Bus.GetAddress` on the *session* bus, then connecting to
//! what it hands back. Connecting to the session bus and serving objects
//! there is a silent no-op: everything works, nothing is ever asked.
//!
//! # Every signature here was read off a live bus
//!
//! The interface definitions below were taken from at-spi2-registryd's
//! own `org.freedesktop.DBus.Introspectable.Introspect` output, not
//! recalled. `GetState` returns `au` — an *array* of `u32`, low word
//! then high — not the `(uu)` tuple it would be natural to assume, and
//! `Parent` is a `(so)` struct of `(bus_name, object_path)` rather than a
//! bare path. Re-derive before changing any of it:
//!
//! ```text
//! busctl --address=unix:path=$XDG_RUNTIME_DIR/at-spi/bus \
//!     call :1.1 /org/a11y/atspi/accessible/root \
//!     org.freedesktop.DBus.Introspectable Introspect
//! ```
//!
//! # One type per interface
//!
//! An AT-SPI object implements several interfaces at one path —
//! `Accessible` *and* `Component` *and*, sometimes, `Action`. `#[interface]`
//! generates one `Interface` impl per type, so each interface needs its
//! own type, and all of them are registered at the same path. Hence the
//! newtypes over [`Node`]: they are one object, seen from three sides.

use zbus::zvariant::{ObjectPath, OwnedObjectPath};
use zbus::{Connection, interface, proxy};

use crate::atspi::store::{NULL_PATH, Object, ROOT_PATH, Store};
use crate::atspi::vocabulary;

/// An AT-SPI object reference: `(bus_name, object_path)`.
///
/// The pair, not the path alone — a reader has to know *who* to ask, and
/// an application's objects live on its own unique bus name.
pub type Reference = (String, OwnedObjectPath);

/// Builds the null reference, which is how AT-SPI says "no object".
fn null() -> Reference {
    (
        String::new(),
        ObjectPath::try_from(NULL_PATH)
            .expect("the null path is a valid object path")
            .into(),
    )
}

/// Finds the accessibility bus address by asking the session bus.
#[proxy(
    interface = "org.a11y.Bus",
    default_service = "org.a11y.Bus",
    default_path = "/org/a11y/bus"
)]
trait Bus {
    /// Returns the address of the accessibility bus.
    fn get_address(&self) -> zbus::Result<String>;
}

/// The registry's socket, which an application plugs its root into.
#[proxy(
    interface = "org.a11y.atspi.Socket",
    default_service = "org.a11y.atspi.Registry",
    default_path = "/org/a11y/atspi/accessible/root"
)]
trait Socket {
    /// Embeds `plug` (our root) into the desktop, returning the desktop
    /// object that is now our root's parent.
    fn embed(&self, plug: &(String, OwnedObjectPath)) -> zbus::Result<Reference>;

    /// Removes `plug` from the desktop.
    fn unembed(&self, plug: &(String, OwnedObjectPath)) -> zbus::Result<()>;
}

/// Connects to the accessibility bus.
///
/// Returns `Ok(None)` — not an error — when there is no accessibility bus
/// to connect to. That is the ordinary case on a machine with no
/// assistive technology running, and an application must not fail to
/// start because of it.
pub async fn connect() -> zbus::Result<Option<Connection>> {
    let session = Connection::session().await?;

    let Ok(bus) = BusProxy::new(&session).await else {
        return Ok(None);
    };

    let Ok(address) = bus.get_address().await else {
        return Ok(None);
    };

    let address: zbus::Address = address.parse()?;

    Ok(Some(
        zbus::connection::Builder::address(address)?.build().await?,
    ))
}

/// What every served object needs: where to read from, and who it is.
#[derive(Clone)]
pub struct Node {
    store: Store,
    bus_name: String,
    path: String,
}

impl Node {
    /// This node, as of the current snapshot.
    ///
    /// `None` for a node that has since been removed. A reader asking
    /// about a retired object is not an error — it is a race, and it lost
    /// — so the answer is an empty one rather than a D-Bus fault.
    fn object(&self) -> Option<Object> {
        self.store.snapshot().get(&self.path).cloned()
    }

    /// A [`Reference`] to `path` on this application's bus name.
    fn reference(&self, path: &str) -> Reference {
        ObjectPath::try_from(path.to_owned())
            .map(|path| (self.bus_name.clone(), path.into()))
            .unwrap_or_else(|_| null())
    }
}

/// The application root, as `org.a11y.atspi.Accessible`.
///
/// The one object the registry is told about; every other node hangs
/// beneath it, and a reader discovers them by walking down from here.
pub struct Root {
    node: Node,
    name: String,
    /// The desktop object the registry handed back from `Embed`, which is
    /// this root's parent.
    desktop: Option<Reference>,
}

#[interface(name = "org.a11y.atspi.Accessible")]
impl Root {
    /// The application's name, as announced.
    #[zbus(property)]
    fn name(&self) -> String {
        self.name.clone()
    }

    /// The application has no description of its own.
    #[zbus(property)]
    fn description(&self) -> String {
        String::new()
    }

    /// The desktop, once embedded.
    #[zbus(property)]
    fn parent(&self) -> Reference {
        self.desktop.clone().unwrap_or_else(null)
    }

    /// How many top-level nodes this application exposes.
    #[zbus(property)]
    fn child_count(&self) -> i32 {
        self.node.store.snapshot().roots().len() as i32
    }

    /// The AT-SPI interface version this object speaks.
    #[zbus(property)]
    fn version(&self) -> u32 {
        0
    }

    /// The application root has no locale of its own.
    #[zbus(property)]
    fn locale(&self) -> String {
        String::new()
    }

    /// No toolkit-specific id.
    #[zbus(property, name = "AccessibleId")]
    fn accessible_id(&self) -> String {
        String::new()
    }

    /// No help text.
    #[zbus(property, name = "HelpText")]
    fn help_text(&self) -> String {
        String::new()
    }

    /// The `index`-th top-level node.
    fn get_child_at_index(&self, index: i32) -> Reference {
        let snapshot = self.node.store.snapshot();

        usize::try_from(index)
            .ok()
            .and_then(|index| snapshot.roots().get(index).cloned())
            .map(|path| self.node.reference(&path))
            .unwrap_or_else(null)
    }

    /// Every top-level node, in traversal order.
    fn get_children(&self) -> Vec<Reference> {
        self.node
            .store
            .snapshot()
            .roots()
            .iter()
            .map(|path| self.node.reference(path))
            .collect()
    }

    /// The registry tracks where we sit among the desktop's children.
    fn get_index_in_parent(&self) -> i32 {
        -1
    }

    /// The root has no relations.
    fn get_relation_set(&self) -> Vec<(u32, Vec<Reference>)> {
        Vec::new()
    }

    /// `ATSPI_ROLE_APPLICATION`, read from `Atspi.Role.APPLICATION` like
    /// every other constant here (see [`vocabulary`]'s module docs).
    fn get_role(&self) -> u32 {
        75
    }

    /// The role's machine name.
    fn get_role_name(&self) -> String {
        "application".to_owned()
    }

    /// The role's human-readable name.
    fn get_localized_role_name(&self) -> String {
        "application".to_owned()
    }

    /// The root is always enabled and showing.
    fn get_state(&self) -> Vec<u32> {
        let (low, high) =
            vocabulary::state_words(vocabulary::state_set(taim_core::semantics::State::empty()));

        vec![low, high]
    }

    /// No attributes.
    fn get_attributes(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    /// The application object is this object.
    fn get_application(&self) -> Reference {
        self.node.reference(ROOT_PATH)
    }

    /// The interfaces this object implements.
    fn get_interfaces(&self) -> Vec<String> {
        vec![
            "org.a11y.atspi.Accessible".to_owned(),
            "org.a11y.atspi.Application".to_owned(),
        ]
    }
}

/// The application root, as `org.a11y.atspi.Application`.
pub struct Application {
    id: i32,
}

#[interface(name = "org.a11y.atspi.Application")]
impl Application {
    /// Which toolkit this is — what a reader keys quirks off.
    #[zbus(property, name = "ToolkitName")]
    fn toolkit_name(&self) -> String {
        "Taim".to_owned()
    }

    /// The toolkit's version.
    #[zbus(property)]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    /// The AT-SPI version spoken.
    #[zbus(property, name = "AtspiVersion")]
    fn atspi_version(&self) -> String {
        "2.1".to_owned()
    }

    /// The registry's id for this application. It assigns this, so the
    /// property is writable and we simply remember what we are told.
    #[zbus(property)]
    fn id(&self) -> i32 {
        self.id
    }

    #[zbus(property)]
    fn set_id(&mut self, id: i32) {
        self.id = id;
    }

    /// The application's locale for `lctype`.
    fn get_locale(&self, _lctype: u32) -> String {
        std::env::var("LANG").unwrap_or_else(|_| "C".to_owned())
    }

    /// Event listener registration is the registry's job on this bus.
    fn register_event_listener(&self, _event: &str) {}

    /// As [`Application::register_event_listener`].
    fn deregister_event_listener(&self, _event: &str) {}
}

/// A node, as `org.a11y.atspi.Accessible`.
pub struct Accessible {
    node: Node,
}

#[interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    /// The accessible name.
    #[zbus(property)]
    fn name(&self) -> String {
        self.node
            .object()
            .and_then(|object| object.name)
            .map(|name| name.to_string())
            .unwrap_or_default()
    }

    /// The accessible description.
    #[zbus(property)]
    fn description(&self) -> String {
        self.node
            .object()
            .and_then(|object| object.description)
            .map(|description| description.to_string())
            .unwrap_or_default()
    }

    /// This node's parent.
    #[zbus(property)]
    fn parent(&self) -> Reference {
        match self.node.object() {
            Some(object) => self.node.reference(&object.parent),
            None => null(),
        }
    }

    /// How many children this node has.
    #[zbus(property)]
    fn child_count(&self) -> i32 {
        self.node
            .object()
            .map(|object| object.children.len() as i32)
            .unwrap_or_default()
    }

    /// The AT-SPI interface version this object speaks.
    #[zbus(property)]
    fn version(&self) -> u32 {
        0
    }

    /// Inherited from the application.
    #[zbus(property)]
    fn locale(&self) -> String {
        String::new()
    }

    /// No toolkit-specific id.
    #[zbus(property, name = "AccessibleId")]
    fn accessible_id(&self) -> String {
        String::new()
    }

    /// No help text.
    #[zbus(property, name = "HelpText")]
    fn help_text(&self) -> String {
        String::new()
    }

    /// The `index`-th child.
    fn get_child_at_index(&self, index: i32) -> Reference {
        let Some(object) = self.node.object() else {
            return null();
        };

        usize::try_from(index)
            .ok()
            .and_then(|index| object.children.get(index).cloned())
            .map(|path| self.node.reference(&path))
            .unwrap_or_else(null)
    }

    /// Every child, in traversal order.
    fn get_children(&self) -> Vec<Reference> {
        let Some(object) = self.node.object() else {
            return Vec::new();
        };

        object
            .children
            .iter()
            .map(|path| self.node.reference(path))
            .collect()
    }

    /// This node's index among its parent's children.
    fn get_index_in_parent(&self) -> i32 {
        self.node
            .object()
            .map(|object| object.index_in_parent)
            .unwrap_or(-1)
    }

    /// No relations are modelled yet.
    fn get_relation_set(&self) -> Vec<(u32, Vec<Reference>)> {
        Vec::new()
    }

    /// This node's AT-SPI role.
    fn get_role(&self) -> u32 {
        self.node
            .object()
            .map(|object| vocabulary::role(object.role) as u32)
            .unwrap_or(vocabulary::AtspiRole::Invalid as u32)
    }

    /// The role's machine name.
    fn get_role_name(&self) -> String {
        self.node
            .object()
            .map(|object| vocabulary::role_name(object.role).to_owned())
            .unwrap_or_default()
    }

    /// The role's human-readable name.
    fn get_localized_role_name(&self) -> String {
        self.get_role_name()
    }

    /// This node's state, as the `(low, high)` word pair.
    fn get_state(&self) -> Vec<u32> {
        let state = self
            .node
            .object()
            .map(|object| object.state)
            .unwrap_or_default();

        let (low, high) = vocabulary::state_words(vocabulary::state_set(state));

        vec![low, high]
    }

    /// No attributes are modelled yet.
    fn get_attributes(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    /// The application this node belongs to.
    fn get_application(&self) -> Reference {
        self.node.reference(ROOT_PATH)
    }

    /// The interfaces this object implements.
    ///
    /// Computed, not fixed: a reader uses this to decide what to ask, and
    /// claiming `Action` on a node with none would offer the user an
    /// empty menu.
    fn get_interfaces(&self) -> Vec<String> {
        let mut interfaces = vec![
            "org.a11y.atspi.Accessible".to_owned(),
            "org.a11y.atspi.Component".to_owned(),
        ];

        if self
            .node
            .object()
            .is_some_and(|object| !object.actions.is_empty())
        {
            interfaces.push("org.a11y.atspi.Action".to_owned());
        }

        interfaces
    }
}

/// A node, as `org.a11y.atspi.Component`.
pub struct Component {
    node: Node,
}

#[interface(name = "org.a11y.atspi.Component")]
impl Component {
    /// This node's bounds, as `(x, y, width, height)`.
    fn get_extents(&self, _coord_type: u32) -> (i32, i32, i32, i32) {
        let Some(object) = self.node.object() else {
            return (0, 0, 0, 0);
        };

        let bounds = object.bounds;

        (
            bounds.x as i32,
            bounds.y as i32,
            bounds.width as i32,
            bounds.height as i32,
        )
    }

    /// This node's position.
    fn get_position(&self, coord_type: u32) -> (i32, i32) {
        let (x, y, _, _) = self.get_extents(coord_type);

        (x, y)
    }

    /// This node's size.
    fn get_size(&self) -> (i32, i32) {
        let (_, _, width, height) = self.get_extents(0);

        (width, height)
    }

    /// Whether `(x, y)` is inside this node.
    fn contains(&self, x: i32, y: i32, _coord_type: u32) -> bool {
        self.node.object().is_some_and(|object| {
            object
                .bounds
                .contains(taim_core::Point::new(x as f32, y as f32))
        })
    }

    /// The child containing `(x, y)`.
    fn get_accessible_at_point(&self, x: i32, y: i32, _coord_type: u32) -> Reference {
        let snapshot = self.node.store.snapshot();
        let point = taim_core::Point::new(x as f32, y as f32);

        let Some(object) = snapshot.get(&self.node.path) else {
            return null();
        };

        // Last match wins, matching the topmost-sibling convention the
        // rest of the framework follows.
        object
            .children
            .iter()
            .rev()
            .find(|path| {
                snapshot
                    .get(path)
                    .is_some_and(|child| child.bounds.contains(point))
            })
            .map(|path| self.node.reference(path))
            .unwrap_or_else(null)
    }

    /// `ATSPI_LAYER_WIDGET`, read from `Atspi.ComponentLayer.WIDGET`.
    fn get_layer(&self) -> u32 {
        3
    }

    /// Z-order is meaningless outside an MDI window manager.
    #[zbus(name = "GetMDIZOrder")]
    fn get_mdi_z_order(&self) -> i16 {
        0
    }

    /// Everything is opaque.
    fn get_alpha(&self) -> f64 {
        1.0
    }

    /// Focus cannot be grabbed from the bus yet: doing so has to reach the
    /// widget tree, which lives on the UI thread and has no channel back.
    /// Reports failure rather than claiming to have done it.
    fn grab_focus(&self) -> bool {
        false
    }
}

/// A node, as `org.a11y.atspi.Action`.
pub struct Action {
    node: Node,
}

impl Action {
    fn action(&self, index: i32) -> Option<vocabulary::Action> {
        let object = self.node.object()?;
        let index = usize::try_from(index).ok()?;

        vocabulary::actions(object.actions).get(index).copied()
    }
}

#[interface(name = "org.a11y.atspi.Action")]
impl Action {
    /// How many actions this node offers.
    #[zbus(property, name = "NActions")]
    fn n_actions(&self) -> i32 {
        self.node
            .object()
            .map(|object| vocabulary::actions(object.actions).len() as i32)
            .unwrap_or_default()
    }

    /// The `index`-th action's machine name.
    fn get_name(&self, index: i32) -> String {
        self.action(index)
            .map(|action| action.name.to_owned())
            .unwrap_or_default()
    }

    /// The `index`-th action's description.
    fn get_description(&self, index: i32) -> String {
        self.action(index)
            .map(|action| action.description.to_owned())
            .unwrap_or_default()
    }

    /// No keybindings are modelled yet.
    fn get_key_binding(&self, _index: i32) -> String {
        String::new()
    }

    /// Every action this node offers.
    fn get_actions(&self) -> Vec<(String, String, String)> {
        self.node
            .object()
            .map(|object| {
                vocabulary::actions(object.actions)
                    .into_iter()
                    .map(|action| {
                        (
                            action.name.to_owned(),
                            action.description.to_owned(),
                            String::new(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Performs the `index`-th action.
    ///
    /// Always reports failure today. Performing one has to reach the
    /// widget on the UI thread, which needs a channel back that does not
    /// exist yet — and reporting success for something that did not happen
    /// is the worse failure: a user would be told their button was
    /// pressed.
    fn do_action(&self, _index: i32) -> bool {
        false
    }
}

/// A running AT-SPI bridge: an object server on the accessibility bus,
/// embedded in the desktop.
///
/// Holds the connection open. Dropping it drops the connection, and the
/// registry notices the bus name vanish and retires the application.
pub struct Server {
    connection: Connection,
    node: Node,
    /// Paths currently served, so a retired node's object is removed.
    served: std::collections::HashSet<String>,
}

impl Server {
    /// Starts a bridge for `store`, announcing the application as `name`.
    ///
    /// Returns `Ok(None)` when there is no accessibility bus — the
    /// ordinary case with no assistive technology running. An application
    /// must not fail to start because nobody is listening.
    pub async fn start(store: Store, name: &str) -> zbus::Result<Option<Self>> {
        let Some(connection) = connect().await? else {
            return Ok(None);
        };

        let bus_name = connection
            .unique_name()
            .map(ToString::to_string)
            .unwrap_or_default();

        let node = Node {
            store,
            bus_name: bus_name.clone(),
            path: ROOT_PATH.to_owned(),
        };

        let server = connection.object_server();

        let _ = server
            .at(
                ROOT_PATH,
                Root {
                    node: node.clone(),
                    name: name.to_owned(),
                    desktop: None,
                },
            )
            .await?;

        let _ = server.at(ROOT_PATH, Application { id: 0 }).await?;

        // Plug into the desktop. Failing here is not fatal: the objects
        // are served regardless, and a registry starting later can still
        // find them — whereas refusing to run because the registry was
        // busy would take an application down over an accessibility
        // hiccup.
        if let Ok(socket) = SocketProxy::new(&connection).await {
            let plug = (
                bus_name,
                ObjectPath::try_from(ROOT_PATH)
                    .expect("the root path is a valid object path")
                    .into(),
            );

            if let Ok(desktop) = socket.embed(&plug).await {
                let root = server.interface::<_, Root>(ROOT_PATH).await?;
                root.get_mut().await.desktop = Some(desktop);
            }
        }

        Ok(Some(Self {
            connection: connection.clone(),
            node,
            served: std::collections::HashSet::new(),
        }))
    }

    /// Brings the served object set in line with the current snapshot.
    ///
    /// Adding an object for a node the store now knows, and removing one
    /// for a node it has forgotten, is all this does — the objects read
    /// through to the store, so their *contents* need no syncing.
    pub async fn sync(&mut self) -> zbus::Result<()> {
        let snapshot = self.node.store.snapshot();

        let mut wanted = std::collections::HashSet::new();
        let mut stack: Vec<String> = snapshot.roots().to_vec();

        while let Some(path) = stack.pop() {
            let Some(object) = snapshot.get(&path) else {
                continue;
            };

            stack.extend(object.children.iter().cloned());
            let _ = wanted.insert(path);
        }

        let server = self.connection.object_server();

        for path in wanted.difference(&self.served.clone()) {
            let node = Node {
                store: self.node.store.clone(),
                bus_name: self.node.bus_name.clone(),
                path: path.clone(),
            };

            let _ = server
                .at(path.as_str(), Accessible { node: node.clone() })
                .await?;
            let _ = server
                .at(path.as_str(), Component { node: node.clone() })
                .await?;
            let _ = server.at(path.as_str(), Action { node }).await?;

            let _ = self.served.insert(path.clone());
        }

        for path in self.served.clone().difference(&wanted) {
            let _ = server.remove::<Accessible, _>(path.as_str()).await?;
            let _ = server.remove::<Component, _>(path.as_str()).await?;
            let _ = server.remove::<Action, _>(path.as_str()).await?;

            let _ = self.served.remove(path);
        }

        Ok(())
    }

    /// The connection this bridge runs on.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
