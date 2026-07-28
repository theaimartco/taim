//! End-to-end tests against a **real** accessibility bus.
//!
//! Everything else in this crate is CPU-verifiable. This is not: the only
//! way to know a screen reader can see an Aura application is to serve a
//! tree, connect a second client the way Orca does, and ask it questions
//! over D-Bus.
//!
//! Skipped, not failed, when there is no accessibility bus — that is the
//! ordinary state of a CI box, and a suite that goes red because nobody
//! is running a screen reader teaches its readers to ignore it.

#![cfg(feature = "atspi")]

use aura_accessibility::atspi::server::Server;
use aura_accessibility::atspi::store::{ROOT_PATH, Store};
use aura_accessibility::node::{Entry, Order};
use aura_accessibility::tree::Change;
use aura_core::semantics::{Actions, Role, State};
use aura_core::widget::Id;
use aura_core::{Point, Rectangle, Size, SmolStr};
use zbus::zvariant::OwnedObjectPath;

/// Whether an accessibility bus is reachable at all.
fn has_bus() -> bool {
    zbus::block_on(async { aura_accessibility::atspi::server::connect().await })
        .ok()
        .flatten()
        .is_some()
}

macro_rules! needs_bus {
    () => {
        if !has_bus() {
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
        let connection = aura_accessibility::atspi::server::connect()
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
fn a_screen_reader_can_walk_an_aura_tree_over_the_bus() {
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

        let mut server = Server::start(store.clone(), "Aura Test")
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
        assert_eq!(toolkit, "Aura");

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

        let mut server = Server::start(store.clone(), "Aura Test")
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
        // though Aura's node set no such flag: Aura reports what is wrong,
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

        let mut server = Server::start(store.clone(), "Aura Test")
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
