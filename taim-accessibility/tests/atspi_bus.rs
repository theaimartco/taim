//! End-to-end tests against a **real** accessibility bus.
//!
//! Everything else in this crate is CPU-verifiable. This is not: the only
//! way to know a screen reader can see an Taim application is to serve a
//! tree, connect a second client the way Orca does, and ask it questions
//! over D-Bus.
//!
//! Skipped, not failed, when there is no accessibility bus — that is the
//! ordinary state of a CI box, and a suite that goes red because nobody
//! is running a screen reader teaches its readers to ignore it.

#![cfg(feature = "atspi")]

use taim_accessibility::atspi::server::Server;
use taim_accessibility::atspi::store::{ROOT_PATH, Store};
use taim_accessibility::node::{Entry, Order};
use taim_accessibility::tree::Change;
use taim_core::semantics::{Actions, Role, State};
use taim_core::widget::Id;
use taim_core::{Point, Rectangle, Size, SmolStr};
use zbus::interface;
use zbus::zvariant::OwnedObjectPath;

use std::sync::OnceLock;

/// Whether an accessibility bus is reachable at all.
fn has_bus() -> bool {
    zbus::block_on(async { taim_accessibility::atspi::server::connect().await })
        .ok()
        .flatten()
        .is_some()
}

/// Stands in for `at-spi-bus-launcher`.
///
/// [`connect`](taim_accessibility::atspi::server::connect) finds the
/// accessibility bus by asking `org.a11y.Bus.GetAddress` on the *session*
/// bus and connecting to whatever address comes back. Answering with the
/// session bus's own address puts the server on the bus this test is
/// already using — which is what lets these tests run on a CI box with
/// `dbus-run-session` and nothing else, rather than needing a desktop
/// with at-spi2-core and a screen reader installed.
///
/// Faithful for everything asserted here. The one thing it does not cover
/// is the registry handshake: `Socket.Embed` finds no registry and fails,
/// exactly as `Server::start` already tolerates in production.
struct A11yBus {
    address: String,
}

#[interface(name = "org.a11y.Bus")]
impl A11yBus {
    fn get_address(&self) -> String {
        self.address.clone()
    }
}

/// Holds the stand-in launcher's connection open for the whole process.
///
/// A `Connection` deregisters its names on drop, so this cannot be a
/// local: the first test would take the bus down on the way out and every
/// test after it would skip.
static LAUNCHER: OnceLock<Option<zbus::Connection>> = OnceLock::new();

/// Makes an accessibility bus reachable, if one can be.
///
/// A real bus already running is left alone — on a developer's desktop
/// these tests should exercise the actual AT-SPI stack, not a stand-in.
fn ensure_bus() -> bool {
    if has_bus() {
        return true;
    }

    let _ = LAUNCHER.get_or_init(|| {
        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .ok()
            .filter(|address| !address.is_empty())?;

        zbus::block_on(async {
            zbus::connection::Builder::session()
                .ok()?
                .name("org.a11y.Bus")
                .ok()?
                .serve_at("/org/a11y/bus", A11yBus { address })
                .ok()?
                .build()
                .await
                .ok()
        })
    });

    has_bus()
}

/// Whether a missing bus is a skip or a failure.
///
/// CI sets this. Without it a runner whose D-Bus failed to start would go
/// green on a suite that ran nothing — which is the exact shape of the bug
/// F-15 was: a transport nobody noticed was doing nothing.
fn bus_required() -> bool {
    std::env::var("TAIM_ATSPI_REQUIRE_BUS").is_ok_and(|value| value == "1")
}

macro_rules! needs_bus {
    () => {
        if !ensure_bus() {
            assert!(
                !bus_required(),
                "TAIM_ATSPI_REQUIRE_BUS=1 but no bus was reachable and none could \
                 be stood up: run under `dbus-run-session --`"
            );

            eprintln!("skipping: no accessibility bus on this machine");
            return;
        }
    };
}

fn entry(id: &Id, parent: Option<&Id>, order: Order, role: Role, name: &str) -> Entry {
    Entry {
        id: id.clone(),
        parent: parent.cloned(),
        order,
        bounds: Rectangle::new(Point::new(4.0, 8.0), Size::new(64.0, 32.0)),
        role,
        state: State::FOCUSABLE,
        name: Some(SmolStr::new(name)),
        description: None,
        value: None,
        actions: Actions::ACTIVATE | Actions::FOCUS,
        live: None,
    }
}

/// An independent client on the accessibility bus, asking the questions
/// Orca asks.
///
/// Deliberately its *own* connection rather than the server's: a screen
/// reader is a separate process, and a test that called the object server
/// through the same connection it was registered on would prove only that
/// zbus can talk to itself.
struct Reader {
    connection: zbus::Connection,
    bus_name: String,
}

impl Reader {
    /// Opens a fresh connection to the accessibility bus, aimed at
    /// `server`'s bus name.
    async fn attach_to(server: &Server) -> Self {
        let connection = taim_accessibility::atspi::server::connect()
            .await
            .expect("the bus was reachable a moment ago")
            .expect("the bus was reachable a moment ago");

        Self {
            connection,
            bus_name: server
                .connection()
                .unique_name()
                .expect("a connected server has a unique name")
                .to_string(),
        }
    }

    async fn call<T>(&self, path: &str, interface: &str, method: &str) -> zbus::Result<T>
    where
        T: for<'d> zbus::zvariant::Type + serde::de::DeserializeOwned + 'static,
    {
        let reply = self
            .connection
            .call_method(
                Some(self.bus_name.as_str()),
                path,
                Some(interface),
                method,
                &(),
            )
            .await?;

        reply.body().deserialize()
    }

    /// Reads a property the way a reader does: through
    /// `org.freedesktop.DBus.Properties.Get`, which wraps the answer in a
    /// variant.
    async fn property(&self, path: &str, interface: &str, name: &str) -> zbus::Result<String> {
        let reply = self
            .connection
            .call_method(
                Some(self.bus_name.as_str()),
                path,
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(interface, name),
            )
            .await?;

        let value: zbus::zvariant::OwnedValue = reply.body().deserialize()?;

        String::try_from(value)
            .map_err(|_| zbus::Error::Failure(format!("{name} was not a string")))
    }
}

#[test]
fn a_screen_reader_can_walk_an_taim_tree_over_the_bus() {
    needs_bus!();

    zbus::block_on(async {
        let store = Store::new();

        let group = Id::unique();
        let button = Id::unique();

        store.apply(&[
            Change::Added(entry(&group, None, Order::root(0), Role::Group, "Toolbar")),
            Change::Added(entry(
                &button,
                Some(&group),
                Order::root(0).child(0),
                Role::Button,
                "Save",
            )),
        ]);

        let mut server = Server::start(store.clone(), "Taim Test")
            .await
            .expect("the bridge must start")
            .expect("a bus was found a moment ago");

        server.sync().await.expect("objects must be served");

        let reader = Reader::attach_to(&server).await;

        // The root announces itself as an application.
        let role: u32 = reader
            .call(ROOT_PATH, "org.a11y.atspi.Accessible", "GetRole")
            .await
            .expect("the root must answer GetRole");
        assert_eq!(role, 75, "ATSPI_ROLE_APPLICATION");

        let toolkit = reader
            .property(ROOT_PATH, "org.a11y.atspi.Application", "ToolkitName")
            .await
            .expect("the root must report its toolkit");
        assert_eq!(toolkit, "Taim");

        // Walk down: root -> group -> button, the way Orca does.
        let roots: Vec<(String, OwnedObjectPath)> = reader
            .call(ROOT_PATH, "org.a11y.atspi.Accessible", "GetChildren")
            .await
            .expect("the root must answer GetChildren");

        assert_eq!(roots.len(), 1, "one top-level node was published");

        let group_path = roots[0].1.as_str().to_owned();

        let name = reader
            .property(&group_path, "org.a11y.atspi.Accessible", "Name")
            .await
            .expect("the group must report its name");
        assert_eq!(name, "Toolbar");

        let children: Vec<(String, OwnedObjectPath)> = reader
            .call(&group_path, "org.a11y.atspi.Accessible", "GetChildren")
            .await
            .expect("the group must answer GetChildren");

        assert_eq!(children.len(), 1);

        let button_path = children[0].1.as_str().to_owned();

        let name = reader
            .property(&button_path, "org.a11y.atspi.Accessible", "Name")
            .await
            .expect("the button must report its name");
        assert_eq!(name, "Save", "a screen reader must hear the button's label");

        let role: u32 = reader
            .call(&button_path, "org.a11y.atspi.Accessible", "GetRole")
            .await
            .expect("the button must answer GetRole");
        assert_eq!(role, 43, "ATSPI_ROLE_PUSH_BUTTON");

        let role_name: String = reader
            .call(&button_path, "org.a11y.atspi.Accessible", "GetRoleName")
            .await
            .expect("the button must answer GetRoleName");
        assert_eq!(role_name, "button");
    });
}

#[test]
fn a_node_reports_its_geometry_and_state_over_the_bus() {
    needs_bus!();

    zbus::block_on(async {
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(
            &id,
            None,
            Order::root(0),
            Role::Button,
            "Save",
        ))]);

        let mut server = Server::start(store.clone(), "Taim Test")
            .await
            .expect("the bridge must start")
            .expect("a bus was found a moment ago");

        server.sync().await.expect("objects must be served");

        let reader = Reader::attach_to(&server).await;

        let path = store.path_of(&id).expect("the node is published");

        let extents: (i32, i32, i32, i32) = reader
            .connection
            .call_method(
                Some(reader.bus_name.as_str()),
                path.as_str(),
                Some("org.a11y.atspi.Component"),
                "GetExtents",
                &(0u32,),
            )
            .await
            .expect("the node must answer GetExtents")
            .body()
            .deserialize()
            .expect("extents are (iiii)");

        assert_eq!(
            extents,
            (4, 8, 64, 32),
            "geometry must survive the crossing"
        );

        // `GetState` returns `au` -- an array of u32, low word then high --
        // not the `(uu)` tuple it would be natural to assume. This is the
        // shape read off the live registry's own introspection.
        let state: Vec<u32> = reader
            .call(&path, "org.a11y.atspi.Accessible", "GetState")
            .await
            .expect("the node must answer GetState");

        assert_eq!(state.len(), 2, "GetState is (low, high)");

        // ATSPI_STATE_ENABLED (8) and SENSITIVE (24) must be present even
        // though Taim's node set no such flag: Taim reports what is wrong,
        // AT-SPI reports what is right.
        assert!(state[0] & (1 << 8) != 0, "must be ENABLED");
        assert!(state[0] & (1 << 24) != 0, "must be SENSITIVE");
        assert!(state[0] & (1 << 11) != 0, "must be FOCUSABLE");
    });
}

#[test]
fn a_node_offers_its_actions_over_the_bus() {
    needs_bus!();

    zbus::block_on(async {
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(
            &id,
            None,
            Order::root(0),
            Role::Button,
            "Save",
        ))]);

        let mut server = Server::start(store.clone(), "Taim Test")
            .await
            .expect("the bridge must start")
            .expect("a bus was found a moment ago");

        server.sync().await.expect("objects must be served");

        let reader = Reader::attach_to(&server).await;

        let path = store.path_of(&id).expect("the node is published");

        let actions: Vec<(String, String, String)> = reader
            .call(&path, "org.a11y.atspi.Action", "GetActions")
            .await
            .expect("the node must answer GetActions");

        assert_eq!(
            actions
                .iter()
                .map(|(name, _, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["click", "grabFocus"],
            "a screen reader must be offered the button's actions, in wire order"
        );

        let interfaces: Vec<String> = reader
            .call(&path, "org.a11y.atspi.Accessible", "GetInterfaces")
            .await
            .expect("the node must answer GetInterfaces");

        assert!(
            interfaces
                .iter()
                .any(|name| name == "org.a11y.atspi.Action"),
            "a node with actions must advertise the Action interface: {interfaces:?}"
        );
    });
}

#[test]
fn a_retired_node_stops_being_served() {
    // The quiet failure: a node leaves the tree, its object stays on the
    // bus, and a reader keeps reading a control that is no longer on
    // screen. `Server::sync` is what has to notice, and nothing else in
    // this suite makes it remove anything.
    needs_bus!();

    zbus::block_on(async {
        let store = Store::new();
        let window = Id::unique();
        let save = Id::unique();
        let cancel = Id::unique();

        store.apply(&[
            Change::Added(entry(
                &window,
                None,
                Order::root(0),
                Role::Window,
                "Preferences",
            )),
            Change::Added(entry(
                &save,
                Some(&window),
                Order::root(0).child(0),
                Role::Button,
                "Save",
            )),
            Change::Added(entry(
                &cancel,
                Some(&window),
                Order::root(0).child(1),
                Role::Button,
                "Cancel",
            )),
        ]);

        let mut server = Server::start(store.clone(), "Taim Test")
            .await
            .expect("the bridge must start")
            .expect("a bus was found a moment ago");

        server.sync().await.expect("objects must be served");

        let window_path = store.path_of(&window).expect("the window is published");
        let cancel_path = store.path_of(&cancel).expect("Cancel is published");

        let reader = Reader::attach_to(&server).await;

        let before: Vec<(String, OwnedObjectPath)> = reader
            .call(&window_path, "org.a11y.atspi.Accessible", "GetChildren")
            .await
            .expect("the window must answer GetChildren");

        assert_eq!(before.len(), 2, "both buttons start out served");

        store.apply(&[Change::Removed(cancel.clone())]);
        server.sync().await.expect("the retirement must sync");

        let after: Vec<(String, OwnedObjectPath)> = reader
            .call(&window_path, "org.a11y.atspi.Accessible", "GetChildren")
            .await
            .expect("the window must still answer GetChildren");

        assert_eq!(after.len(), 1, "the retired button must be gone");

        assert!(
            after.iter().all(|(_, path)| path.as_str() != cancel_path),
            "the retired button's path must not still be listed"
        );

        // And asking it directly must fail rather than answer stale data:
        // a reader that kept the reference must be told it is gone.
        let answer: zbus::Result<u32> = reader
            .call(&cancel_path, "org.a11y.atspi.Accessible", "GetRole")
            .await;

        assert!(
            answer.is_err(),
            "a retired object must not answer, it must be gone"
        );
    });
}
