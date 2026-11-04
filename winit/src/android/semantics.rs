//! Stable virtual identities and a privacy-preserving Android semantic wire format.
use serde_json::{Value, json};
use std::collections::HashMap;
use taim_core::{
    Point, Rectangle,
    semantics::{self, Actions, Request, Role, State},
    widget::Id,
};

/// One validated operation; a single ordered queue preserves cross-operation arrival order.
pub(crate) enum SemanticCommand {
    Action(taim_core::semantics::Action),
    Fill(Id, String, bool),
    Selection(Id, usize, usize),
    Text(Id, String),
}

pub(crate) fn text_target_allowed(
    role: Role,
    state: State,
    explicit_autofill: bool,
    autofill: bool,
) -> bool {
    !state.intersects(State::HIDDEN | State::DISABLED)
        && matches!(role, Role::TextField | Role::SecureField)
        && (role != Role::SecureField || autofill)
        && (!autofill || explicit_autofill)
}

#[derive(Default)]
pub(crate) struct Snapshot {
    ids: HashMap<Id, i32>,
    targets: HashMap<i32, taim_accessibility::node::Entry>,
    next: i32,
    revision: u64,
    previous: String,
}

impl Snapshot {
    #[cfg(test)]
    pub fn update(
        &mut self,
        tree: &taim_accessibility::Tree,
        origin: Point,
        scale: f32,
        clip: Rectangle,
    ) -> Option<String> {
        self.update_with_editor(tree, origin, scale, clip, None)
    }
    pub fn update_with_editor(
        &mut self,
        tree: &taim_accessibility::Tree,
        origin: Point,
        scale: f32,
        clip: Rectangle,
        editor: Option<&(Id, taim_core::input_method::EditorState)>,
    ) -> Option<String> {
        // Atomic native controls already carry their label/value. Exposing
        // their decorative text children duplicates TalkBack stops/actions.
        let atomic = |role| {
            matches!(
                role,
                Role::Button
                    | Role::Checkbox
                    | Role::RadioButton
                    | Role::Slider
                    | Role::TextField
                    | Role::SecureField
                    | Role::Image
                    | Role::Tab
                    | Role::MenuItem
                    | Role::Link
            )
        };
        let ordered: Vec<_> = tree
            .in_order()
            .into_iter()
            .filter(|entry| {
                let mut parent = entry.parent.as_ref();
                while let Some(id) = parent {
                    let Some(ancestor) = tree.get(id) else {
                        break;
                    };
                    if atomic(ancestor.role) {
                        return false;
                    }
                    parent = ancestor.parent.as_ref();
                }
                true
            })
            .collect();
        if ordered.len() > 16384 {
            return None;
        }
        let live: std::collections::HashSet<_> =
            ordered.iter().map(|entry| entry.id.clone()).collect();
        self.ids.retain(|id, _| live.contains(id));
        for entry in &ordered {
            if !self.ids.contains_key(&entry.id) {
                self.next = self.next.checked_add(1)?;
                let _ = self.ids.insert(entry.id.clone(), self.next);
            }
        }
        self.targets.clear();
        let mut nodes = Vec::with_capacity(ordered.len());
        for entry in ordered {
            let id = self.ids[&entry.id];
            let _ = self.targets.insert(id, entry.clone());
            let bounds = Rectangle::new(
                entry.bounds.position() + (origin - Point::ORIGIN),
                entry.bounds.size(),
            ) * scale;
            let value = if entry.role == Role::SecureField {
                Value::Null
            } else {
                match &entry.value {
                    Some(semantics::Value::Text(text)) => json!({"text":text.as_str()}),
                    Some(semantics::Value::Number { current, min, max }) => {
                        json!({"current":current,"min":min,"max":max})
                    }
                    None => Value::Null,
                }
            };
            nodes.push(json!({"id":id,"parent":entry.parent.as_ref().and_then(|parent|self.ids.get(parent)),
                "bounds":[bounds.x,bounds.y,bounds.width,bounds.height],"role":format!("{:?}",entry.role),
                "state":entry.state.bits(),"actions":entry.actions.bits(),"name":entry.name.as_deref(),
                "description":entry.description.as_deref(),"value":value,
                "autofill":entry.autofill.map(|purpose|format!("{purpose:?}")),
                "selection":editor.filter(|(target,_)|*target==entry.id && entry.role==Role::TextField).map(|(_,state)|[taim_core::input_method::utf8_to_utf16(&state.content,state.selection.start),taim_core::input_method::utf8_to_utf16(&state.content,state.selection.end)]),
                "live":if entry.role==Role::SecureField {None}else{entry.live.map(|live| if live == semantics::Live::Assertive {2} else {1})}}));
        }
        let clip = clip * scale;
        let body = json!({"nodes":nodes,"clip":[clip.x,clip.y,clip.width,clip.height]});
        let encoded = body.to_string();
        if encoded.len() > 8 * 1024 * 1024 || encoded == self.previous {
            return None;
        }
        self.previous = encoded;
        self.revision = self.revision.checked_add(1)?;
        Some(json!({"revision":self.revision,"tree":body}).to_string())
    }

    pub fn autofill_id(&self, target: &Id) -> Option<i32> {
        let id = *self.ids.get(target)?;
        let entry = self.targets.get(&id)?;
        (entry.autofill.is_some()
            && matches!(entry.role, Role::TextField | Role::SecureField)
            && !entry.state.intersects(State::DISABLED | State::HIDDEN))
        .then_some(id)
    }

    pub fn target(&self, id: i32, revision: u64) -> Option<&taim_accessibility::node::Entry> {
        if revision != self.revision {
            return None;
        }
        self.targets
            .get(&id)
            .filter(|entry| !entry.state.intersects(State::DISABLED | State::HIDDEN))
    }

    pub fn command(&self, value: &Value) -> Option<SemanticCommand> {
        let entry = self.target(
            i32::try_from(value["id"].as_i64()?).ok()?,
            value["revision"].as_u64()?,
        )?;
        let command = value["action"].as_str()?;
        match command {
            "copy" | "cut" | "paste" | "blur" => {
                if command != "blur" && entry.role != Role::TextField {
                    return None;
                }
                Some(SemanticCommand::Text(entry.id.clone(), command.to_owned()))
            }
            "selection" => {
                if entry.role != Role::TextField {
                    return None;
                }
                let semantics::Value::Text(text) = entry.value.as_ref()? else {
                    return None;
                };
                let start = usize::try_from(value["start"].as_u64()?).ok()?;
                let end = usize::try_from(value["end"].as_u64()?).ok()?;
                let length = text.encode_utf16().count();
                if start > 262144 || end > 262144 || start > length || end > length {
                    return None;
                }
                Some(SemanticCommand::Selection(entry.id.clone(), start, end))
            }
            "text" | "autofill" => {
                if !matches!(entry.role, Role::TextField | Role::SecureField)
                    || (command == "text" && entry.role == Role::SecureField)
                    || (command == "autofill" && entry.autofill.is_none())
                {
                    return None;
                }
                let text = value["text"]
                    .as_str()
                    .filter(|text| text.len() <= 1024 * 1024)?;
                Some(SemanticCommand::Fill(
                    entry.id.clone(),
                    text.to_owned(),
                    command == "autofill",
                ))
            }
            _ => self.action(value).map(SemanticCommand::Action),
        }
    }

    pub fn action(&self, value: &Value) -> Option<semantics::Action> {
        let entry = self.target(
            i32::try_from(value["id"].as_i64()?).ok()?,
            value["revision"].as_u64()?,
        )?;
        let (flag, request) = match value["action"].as_str()? {
            "activate" => (Actions::ACTIVATE, Request::Activate),
            "focus" => (Actions::FOCUS, Request::Focus),
            "increment" => (Actions::INCREMENT, Request::Increment),
            "decrement" => (Actions::DECREMENT, Request::Decrement),
            "expand" => (Actions::TOGGLE_EXPANDED, Request::ToggleExpanded),
            "scroll" => (Actions::SCROLL_INTO_VIEW, Request::ScrollIntoView),
            "dismiss" => (Actions::DISMISS, Request::Dismiss),
            "progress" => {
                let semantics::Value::Number { min, max, .. } = entry.value.as_ref()? else {
                    return None;
                };
                let current = value["value"].as_f64()?;
                if !current.is_finite() || !min.is_finite() || !max.is_finite() || min > max {
                    return None;
                }
                (
                    Actions::SET_VALUE,
                    Request::SetValue(semantics::Value::Number {
                        current: current.clamp(*min, *max),
                        min: *min,
                        max: *max,
                    }),
                )
            }
            _ => return None,
        };
        entry.actions.contains(flag).then(|| semantics::Action {
            target: entry.id.clone(),
            request,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim_core::{Size, semantics::Node};
    fn node(id: Id) -> Node {
        Node::new(
            id,
            Rectangle::with_size(Size::new(100.0, 40.0)),
            Role::Button,
        )
        .with_name("Run")
        .with_actions(Actions::ACTIVATE)
    }
    #[test]
    fn virtual_ids_survive_geometry_updates_but_old_revisions_cannot_act() {
        let id = Id::unique();
        let mut node = node(id);
        let mut snapshot = Snapshot::default();
        let first: Value = serde_json::from_str(
            &snapshot
                .update(
                    &taim_accessibility::Tree::from_forest(std::slice::from_ref(&node)),
                    Point::ORIGIN,
                    2.0,
                    node.bounds,
                )
                .unwrap(),
        )
        .unwrap();
        let virtual_id = first["tree"]["nodes"][0]["id"].clone();
        let action = json!({"id":virtual_id,"revision":first["revision"],"action":"activate"});
        assert!(snapshot.action(&action).is_some());
        node.bounds.x = 20.0;
        let second: Value = serde_json::from_str(
            &snapshot
                .update(
                    &taim_accessibility::Tree::from_forest(std::slice::from_ref(&node)),
                    Point::new(0.0, -5.0),
                    2.0,
                    node.bounds,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            first["tree"]["nodes"][0]["id"],
            second["tree"]["nodes"][0]["id"]
        );
        assert!(snapshot.action(&action).is_none());
        assert_eq!(
            second["tree"]["nodes"][0]["bounds"],
            json!([40.0, -10.0, 200.0, 80.0])
        );
    }
    #[test]
    fn atomic_controls_do_not_export_duplicate_decorative_children() {
        let mut button = node(Id::unique());
        let label = Node::new(Id::unique(), button.bounds, Role::StaticText).with_name("Run");
        button.children.push(label);
        let encoded = Snapshot::default()
            .update(
                &taim_accessibility::Tree::from_forest(&[button.clone()]),
                Point::ORIGIN,
                1.0,
                button.bounds,
            )
            .unwrap();
        let value: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["tree"]["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(value["tree"]["nodes"][0]["role"], "Button");
    }

    #[test]
    fn secure_autofill_metadata_never_exports_value_or_length() {
        let node = Node::new(
            Id::unique(),
            Rectangle::with_size(Size::new(100.0, 40.0)),
            Role::SecureField,
        )
        .with_value(semantics::Value::Text("never-export-this".into()))
        .with_autofill(semantics::Autofill::Password);
        let encoded = Snapshot::default()
            .update(
                &taim_accessibility::Tree::from_forest(std::slice::from_ref(&node)),
                Point::ORIGIN,
                1.0,
                node.bounds,
            )
            .unwrap();
        assert!(!encoded.contains("never-export-this"));
        let value: Value = serde_json::from_str(&encoded).unwrap();
        assert!(value["tree"]["nodes"][0]["value"].is_null());
        assert_eq!(value["tree"]["nodes"][0]["autofill"], "Password");
    }
}

#[cfg(test)]
mod editor_snapshot_tests {
    use super::*;
    use taim_core::{Size, input_method::EditorState, semantics::Node};
    #[test]
    fn selection_updates_publish_without_text_changes_and_secure_selection_is_withheld() {
        let id = Id::unique();
        let secret = Id::unique();
        let bounds = Rectangle::with_size(Size::new(100.0, 40.0));
        let nodes = [
            Node::new(id.clone(), bounds, Role::TextField)
                .with_value(semantics::Value::Text("A🌍".into())),
            Node::new(secret.clone(), bounds, Role::SecureField)
                .with_live(semantics::Live::Assertive),
        ];
        let tree = taim_accessibility::Tree::from_forest(&nodes);
        let mut snapshot = Snapshot::default();
        let mut editor = (
            id,
            EditorState {
                content: "A🌍".into(),
                selection: 1..5,
                composing: None,
                multiline: false,
            },
        );
        let value: Value = serde_json::from_str(
            &snapshot
                .update_with_editor(&tree, Point::ORIGIN, 1.0, bounds, Some(&editor))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["tree"]["nodes"][0]["selection"], json!([1, 3]));
        editor.1.selection = 5..5;
        let value: Value = serde_json::from_str(
            &snapshot
                .update_with_editor(&tree, Point::ORIGIN, 1.0, bounds, Some(&editor))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["tree"]["nodes"][0]["selection"], json!([3, 3]));
        editor.0 = secret;
        let value: Value = serde_json::from_str(
            &snapshot
                .update_with_editor(&tree, Point::ORIGIN, 1.0, bounds, Some(&editor))
                .unwrap(),
        )
        .unwrap();
        assert!(value["tree"]["nodes"][1]["selection"].is_null());
        assert!(value["tree"]["nodes"][1]["live"].is_null());
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use taim_core::{Size, semantics::Node};
    fn snapshot(role: Role, autofill: bool) -> (Snapshot, Value) {
        let bounds = Rectangle::with_size(Size::new(100.0, 40.0));
        let mut node = Node::new(Id::unique(), bounds, role)
            .with_actions(Actions::FOCUS)
            .with_value(semantics::Value::Text("A🌍".into()));
        if autofill {
            node = node.with_autofill(semantics::Autofill::Password);
        }
        let mut snapshot = Snapshot::default();
        let body: Value = serde_json::from_str(
            &snapshot
                .update(
                    &taim_accessibility::Tree::from_forest(&[node]),
                    Point::ORIGIN,
                    1.0,
                    bounds,
                )
                .unwrap(),
        )
        .unwrap();
        let value = json!({"id":body["tree"]["nodes"][0]["id"],"revision":body["revision"]});
        (snapshot, value)
    }
    #[test]
    fn interleaved_selection_and_clipboard_commands_keep_wire_order() {
        let (snapshot, base) = snapshot(Role::TextField, false);
        let mut ordered = Vec::new();
        for (action, start, end) in [
            ("selection", 0, 1),
            ("copy", 0, 0),
            ("selection", 1, 3),
            ("paste", 0, 0),
        ] {
            let mut wire = base.clone();
            wire["action"] = json!(action);
            wire["start"] = json!(start);
            wire["end"] = json!(end);
            ordered.push(snapshot.command(&wire).unwrap());
        }
        let target = &snapshot
            .target(
                base["id"].as_i64().unwrap() as i32,
                base["revision"].as_u64().unwrap(),
            )
            .unwrap()
            .id;
        assert!(matches!(&ordered[0], SemanticCommand::Selection(id, 0, 1) if id == target));
        assert!(
            matches!(&ordered[1],SemanticCommand::Text(id,name) if id == target && name=="copy")
        );
        assert!(matches!(&ordered[2], SemanticCommand::Selection(id, 1, 3) if id == target));
        assert!(
            matches!(&ordered[3],SemanticCommand::Text(id,name) if id == target && name=="paste")
        );
        let mut focus = base;
        focus["action"] = json!("focus");
        assert!(
            matches!(snapshot.command(&focus), Some(SemanticCommand::Action(action))
            if &action.target == target && matches!(action.request, Request::Focus))
        );
    }
    #[test]
    fn selection_rejects_invalid_offsets_and_commands_reject_stale_revision() {
        let (snapshot, mut wire) = snapshot(Role::TextField, false);
        wire["action"] = json!("selection");
        wire["start"] = json!(0);
        wire["end"] = json!(4);
        assert!(snapshot.command(&wire).is_none());
        wire["end"] = json!(3);
        assert!(snapshot.command(&wire).is_some());
        wire["revision"] = json!(0);
        assert!(snapshot.command(&wire).is_none());
    }
    #[test]
    fn secret_commands_require_explicit_autofill_and_never_allow_copy_or_selection() {
        let (snapshot, base) = snapshot(Role::SecureField, false);
        for name in ["copy", "cut", "paste", "selection", "text", "autofill"] {
            let mut wire = base.clone();
            wire["action"] = json!(name);
            wire["text"] = json!("fixture");
            wire["start"] = json!(0);
            wire["end"] = json!(0);
            assert!(snapshot.command(&wire).is_none());
        }
        let (snapshot, mut wire) = self::snapshot(Role::SecureField, true);
        wire["action"] = json!("autofill");
        wire["text"] = json!("fixture");
        assert!(matches!(
            snapshot.command(&wire),
            Some(SemanticCommand::Fill(..))
        ));
    }
}

#[cfg(test)]
mod live_text_privacy_tests {
    use super::*;
    #[test]
    fn plain_text_edits_cannot_upgrade_to_secret_autofill_after_a_role_change() {
        assert!(text_target_allowed(
            Role::TextField,
            State::empty(),
            false,
            false
        ));
        assert!(!text_target_allowed(
            Role::SecureField,
            State::empty(),
            true,
            false
        ));
        assert!(!text_target_allowed(
            Role::SecureField,
            State::empty(),
            false,
            true
        ));
        assert!(text_target_allowed(
            Role::SecureField,
            State::empty(),
            true,
            true
        ));
        assert!(!text_target_allowed(
            Role::SecureField,
            State::HIDDEN,
            true,
            true
        ));
        assert!(!text_target_allowed(
            Role::TextField,
            State::DISABLED,
            true,
            true
        ));
    }
    #[test]
    fn decoder_preserves_explicit_autofill_intent_in_the_ordered_command() {
        let bounds = Rectangle::with_size(taim_core::Size::new(100.0, 40.0));
        let node = taim_core::semantics::Node::new(Id::unique(), bounds, Role::TextField)
            .with_actions(Actions::FOCUS)
            .with_autofill(semantics::Autofill::Username);
        let target = node.id.clone();
        let mut snapshot = Snapshot::default();
        let value: Value = serde_json::from_str(
            &snapshot
                .update(
                    &taim_accessibility::Tree::from_forest(&[node]),
                    Point::ORIGIN,
                    1.0,
                    bounds,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            snapshot.autofill_id(&target),
            value["tree"]["nodes"][0]["id"].as_i64().map(|id| id as i32)
        );
        let mut wire = json!({"id":value["tree"]["nodes"][0]["id"],"revision":value["revision"],"action":"text","text":"fixture"});
        assert!(matches!(
            snapshot.command(&wire),
            Some(SemanticCommand::Fill(id, text, false)) if id == target && text == "fixture"
        ));
        wire["action"] = json!("autofill");
        assert!(matches!(
            snapshot.command(&wire),
            Some(SemanticCommand::Fill(id, text, true)) if id == target && text == "fixture"
        ));
    }
}
