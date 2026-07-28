# The Aura accessibility contract

> Status: Phase 13 baseline. This document describes how a semantic tree
> is produced, retained, diffed, focused, and delivered; the authoring
> rules a widget must follow; the privacy boundary; and — in full, at the
> end — what is not yet done.

## The pipeline

```
UserInterface::semantics    what an application calls, once per frame
        │                   (runtime/src/user_interface.rs)
        ├──────────────────────────────┐
        ▼                              ▼
Widget::semantics              Overlay::semantics
Widget::visit_children         overlay::Nested::semantics
        │  per-widget, dirty-cached    │  a forest: an overlay chain
        │  (core/src/widget.rs)        │  has no natural parent
        ▼                              │
runtime::semantics::build  ◄───────────┘
        │                   forest assembly, skips clean subtrees
        ▼                   (runtime/src/semantics.rs)
accessibility::Tree         retained, indexed by stable Id, diffable
        │                   (aura-accessibility)
        ├──► Tree::diff ──► Change[] ──► adapter ──► platform
        ├──► Focus          traversal, scopes, restoration
        ├──► Router         action routing back to identity
        └──► LiveRegions    announcements, coalescing, privacy
```

Nothing in that chain rebuilds what it can reuse. A clean subtree
contributes a cached node without the widget being asked; an unchanged
tree produces an empty diff; a moved node produces a geometry update
rather than a re-announcement.

`UserInterface::semantics` is the entry point: it assembles the widget
tree's forest and appends any overlay's, and is what an adapter calls per
frame. Everything above it is reachable from a real application;
everything below `accessibility::Tree` still needs a platform adapter
(follow-up #1) before a screen reader can hear any of it.

### How a widget's children are reached

`Widget` cannot be asked for its children generically — every one stores
them differently — so, exactly like `diff`, `layout`, `draw`, `update`
and `operate`, semantic traversal is driven by the widget itself:
`Widget::visit_children` visits each child with the `Tree` and `Layout`
belonging to it. `Widget::semantics` stays non-recursive and reports only
the widget's own node, which is what makes it cacheable.

Children arrive as `widget::visit::Visit` (via `visit::Widgets`), not as
`&mut dyn Widget`, because a widget reference cannot cross a **theme
boundary**: `themer` is a `Widget<Message, AnyTheme, Renderer>` wrapping a
`Widget<Message, Theme, Renderer>`, so no single `dyn Widget<..>` type
accepts both. `operate` crosses it only because `&mut dyn Operation`
mentions no theme; `Visit` erases all three parameters for the same
reason.

`Visit` is the **type-erased projection of `Widget`** — the methods whose
signatures name none of the three parameters — so it serves both
whole-tree traversals: semantic assembly *and* `runtime::hit_test`. One
hook, not two: a parallel children hook would mean every container
writing the same zip twice, with nothing keeping the copies in step.

**A container that does not implement `visit_children` makes its whole
subtree invisible to assistive technology and unreachable by
hit-testing.** Nothing fails loudly. Every container that recurses in
`operate` implements it today, and the two must be kept in step.

### Tree-transparent wrappers

`container`, `pin`, `float` and `themer` defer `tag`/`state`/`diff` to
their content, so wrapper and content **share one `Tree`** — and one
`semantics_with_cache` slot. They must therefore *delegate* both
`semantics` and `visit_children` rather than report a node and visit the
content as a child: a wrapper's own `None` would otherwise be cached
into the shared slot and clear the dirty flag, so the content's lookup
would hit the poisoned cache and report nothing. `container(button(..))`
— as ordinary as a view gets — announced nothing at all until this was
fixed. `tests/accessibility/user_interface.rs` pins it.

## Identity

`aura_core::semantics::Node::id` is **required**, not optional, because
every consumer downstream — diffing, focus, action routing, focus
restoration — needs a key that survives a rebuild.

That identity comes from `widget::Tree::id()`: minted once when the
`Tree` node is created, never regenerated. Reconciliation
(`diff_children_custom`) reuses `Tree` nodes in place, so an id survives
ordinary view reconstruction exactly as a widget's own `State` does. This
is `phase.md`'s "semantic IDs remain stable across ordinary view
reconstruction", and it holds for the same reason — and under the same
conditions — as state preservation already does.

A node whose position changes such that reconciliation cannot match it
gets a new `Tree` and a new identity. That is correct: as far as the
framework can tell, it *is* a different widget.

## Traversal order

`accessibility::node::Order` is the path of child indices from a root down
to a node — the same shape `runtime::hit_test::Path` uses, for the same
reason: it sorts lexicographically into exactly depth-first document
order, which is the order a screen reader reads and Tab moves through. No
tie-breaking, no separate ordering pass.

Order is structural, so it is **direction-agnostic**: an RTL layout
mirrors the geometry, not the document. Spatial (arrow-key) movement uses
geometry instead — which is why it needs no RTL special-casing either. In
a mirrored layout the controls have moved, and `Left` still means left.

## Diffing

`Tree::diff` reports the minimal `Change` set. The variants are narrower
than "something changed" on purpose, because an adapter reacts very
differently to each:

| Change | Meaning | Adapter response |
|---|---|---|
| `Added` | A node that did not exist | Announce it |
| `Removed` | A node that is gone | Retire it |
| `Updated` | Own properties changed (label, state, value) | Re-announce |
| `Moved` | **Geometry only** | Update bounds, stay quiet |
| `Reordered` | New parent/position, same properties | Re-link, stay quiet |

`Moved` is the one that matters for cost: a scroll moves a great many
nodes without changing anything worth speaking. That is `phase.md`'s
"dirty visual bounds update semantics without rebuilding clean subtrees",
seen from the diff's side.

Removals are emitted first, then everything else in traversal order, so
output is deterministic — hash iteration order must never reach an
adapter, or announcements would vary run to run.

### The correctness bar

Stated as a property, not a set of examples:

> For any two trees `a` and `b`, applying `a.diff(&b)` to `a` must
> produce a tree equal to `b`.

That is the exit gate ("incremental semantic output matches full rebuild
oracle"), and it is what lets an adapter trust the cheap path: a diff is
not "close enough", it is indistinguishable from having rebuilt
everything.

## Focus

Focus is state that outlives the tree it points into — the single cause
of both failures the exit gate names.

**Never lost.** `Focus::reconcile` runs after every diff. If the focused
node survived and can still take focus, nothing happens. If it did not,
focus moves to the nearest surviving node **in traversal order** — the one
that would have come next — rather than vanishing or jumping to the top. A
node that merely became disabled is treated the same way: focus leaves,
because landing focus on something inert is how focus gets "lost" from the
user's point of view.

**Never trapped unintentionally.** A `Scope` confines focus deliberately
(a modal must not leak Tab into the inert page behind it). The trap is
intentional while the scope is active, and:

- opening a trapped scope **autofocuses** inside it — leaving focus behind
  a modal means the first Tab appears to do nothing;
- a scope with nothing focusable inside clears focus rather than stranding
  it on content the trap just made unreachable;
- closing restores focus via a **stack** (dialogs open dialogs, and
  restoring the wrong one is as disorienting as restoring nothing);
- a scope whose owner disappears is **dismissed automatically** — an
  abandoned trap would confine focus to a subtree that no longer exists,
  which is the classic accessibility deadlock.

## Live regions and privacy

A live region speaks without being asked, which makes it the easiest place
to leak a secret. `LiveRegions::announce` is the enforcement point, and
the queue is private, so there is no path around it:

- a `Role::SecureField` source is refused **unconditionally**, checked
  against the node's role rather than trusting the caller to pass
  `private` — a caller who forgets must not be able to leak anything;
- an explicitly private announcement is refused (a notification whose
  privacy level withholds its body on a locked screen);
- a node that is not a live region has no standing to announce at all.

The queue coalesces per source — a progress message must not queue every
intermediate value, since by the time the reader reaches them they are all
wrong but the last — while keeping the original queue position, so a
chatty region cannot starve one that spoke earlier. It is bounded, and
drops the oldest **polite** announcement first: stale status is the least
valuable thing present, and an assertive error must outlive it.

## Action routing

An action arrives naming a node the platform saw some frames ago.
`Router::route` checks both facts that matter: does the node still exist,
and does it still claim this action *right now*. It never performs the
action — the caller does — which keeps the check in one place instead of
spread across every widget's handler.

A request naming zero or several actions is malformed: partial failure
would have no defined meaning.

## Authoring rules for a widget

1. **Report an identity from `tree.id()`.** Never mint one per frame.
2. **A disabled control advertises no actions.** Not "actions that get
   rejected" — an empty set. An action assistive technology offers that
   silently does nothing is worse than no action.
3. **Report state even when disabled.** A dimmed checkbox is still
   "checked, dimmed"; dropping the value loses more than it protects.
4. **The visible label is the accessible name.** A control announced
   without one is "checkbox" and nothing else. A widget that *owns* its
   label (`checkbox`, `radio`, `toggler`, `icon_button`) reports it
   directly. A widget whose label is a **child** (`button`, whose content
   is an arbitrary `Element`) cannot: `Widget::semantics` takes `&self`
   and cannot reach its own subtree. Those get their name from content
   during assembly — see [Name from content](#name-from-content) — so
   report `None` and let the rule fill it in.
5. **A value travels with its range.** "70" means nothing without "of 100".
6. **Do not report transient state.** `button`'s `is_pressed` is
   deliberately not surfaced: a momentary press is not something an
   assistive technology acts on, and reporting it would emit a semantic
   update on every mouse-down and mouse-up — pure churn.
7. **Never populate a value on a `SecureField`.** `Node::with_value`
   enforces this itself, but guard at the call site too: the setter
   protects a future caller who forgets, and the guard means the secret
   never reaches the setter at all.

## Overlays

An overlay is reached through `Widget::overlay`, **not** through the
widget tree — so `runtime::semantics::build`, which walks widgets, never
sees one. Until `Overlay::semantics` existed, that meant *every* overlay
was invisible to assistive technology.

That was not a theoretical gap. An open `pick_list` announced nothing: a
screen-reader user could hear the closed control, open it, and find the
entire list of choices unreachable. The same held for `combo_box`.

`Overlay::semantics(&self, layout)` mirrors `Widget::semantics` — 
non-recursive, `None` by default, so a presentational overlay invents
nothing. `overlay::Nested::semantics` walks an overlay chain (a menu whose
item opens a submenu) and returns a **forest**: each level is an
independent thing a screen reader may land on, and there is no natural
parent to gather them under. It takes `&mut self`, as `Nested::operate`
does and for the same reason — reaching a nested overlay goes through
`Overlay::overlay`, which needs a mutable borrow.

An overlay's node must carry a **stable** id, for the same reason a
widget's must. `pick_list`'s dropdown derives its identity from the
widget's own `Tree`, which survives reconciliation.

`UserInterface::semantics` calls `Nested::semantics` after assembling the
widget forest, so an open overlay reaches an application today; this is
proven end to end in `tests/accessibility/user_interface.rs` by clicking
a `pick_list` open through `UserInterface::update` and finding the
dropdown in the resulting forest.

### A dialog is not an overlay

`widget::dialog` is a stacked widget, not an overlay, and deliberately:
it predates this hook, and a modal is the one thing that must never be
invisible. The trade is that it covers what it is stacked over rather
than the whole window, so it belongs at a view's root. Now that overlays
can report semantics, that decision is worth revisiting — but a stacked
modal also participates in ordinary focus traversal, which an overlay
still does not.

## Name from content

`runtime::semantics::build` fills in the accessible name of a node that
has none, from the names of its descendants, when its role takes its name
from content: `Button`, `Link`, `Tab`, `MenuItem`, `ListItem`, `Checkbox`,
`RadioButton`. This is ARIA's rule, and AT-SPI/UIA behave the same way.

It exists because `button("Save")` is a button wrapping a `text`, with no
label of its own to report — it announced as a bare, unlabeled "button"
until this rule existed. The names are gathered depth-first and joined
with spaces, so `button(row![text("Save"), text("all")])` announces "Save
all".

Three deliberate limits:

- **An explicit name always wins.** The rule is a fallback, never an
  override: a `checkbox` that reports its own label keeps it.
- **A nested control's name is not absorbed.** It names itself, and
  folding it into an ancestor would announce it twice, out of context.
- **Nothing to say means no name**, not `""` — an empty name reads as
  "named, with an empty name", and an adapter can fall back on an honest
  absence.

`text` reports `Role::StaticText` carrying its content, which is what
makes the rule work — and independently fixes a larger hole: until it
did, **every label and paragraph in an application was invisible**, and a
screen reader heard the controls and none of the writing between them.
Empty text still reports nothing; `text("")` is a common spacer, and
announcing "" is noise.

A named-from-content node keeps its text children rather than absorbing
them. That is the shape AT-SPI expects, and it discards no information: an
adapter that would rather present a button as a leaf can ignore them, but
an adapter cannot recover what assembly threw away.

## Widgets reporting semantics today

`text`, `button`, `checkbox`, `radio`, `toggler`, `text_input`
(including `secure`), `slider`, `icon_button`, `list_tile`, `app_bar`,
`dialog`, and `pick_list`'s dropdown overlay.

## What Phase 13 does not close

| Clause | Status | Evidence |
|---|---|---|
| "incremental semantic output matches full rebuild oracle" | **Met** | `Tree::diff`/`apply` against `Tree::from_forest`, proven over hand-built cases and a generated mutation matrix. |
| "focus never becomes lost or trapped unintentionally" | **Met** | Node removal, becoming disabled, abandoned traps, nested restore, empty scopes — each with a test that creates the failure and asserts recovery. |
| "live regions do not leak secure/private text" | **Met** | Enforced at the only entry point, checked against the node's role rather than the caller's honesty. |
| "all Aura components have semantic snapshots/actions" | **Partially met** | Ten widgets plus `pick_list`'s dropdown report semantics, each proven into the real engine *by a test*. Four of the formerly contract-only `aura-design` components now have real widgets (`icon_button`, `list_tile`, `app_bar`, `dialog`); **six still have none** (`bottom_sheet`, `menu`, `toast`, `notification`, `navigation`, `window_chrome`) — they declare `role()`/`actions()` as contracts, and nothing renders or reports them. Most catalogued widgets (`scrollable`, `combo_box`, `text_editor`, `table`, `pane_grid`, …) still report nothing. |
| "large text and high contrast remain usable on all device profiles" | **Partially met** | High contrast is enforced and tested (`aura-design` pins the AAA bar and sweeps every component × state × scheme). **Large text is not tested**, and "usable on all device profiles" is partly a visual judgement — the same class of clause left explicitly unmet in Phase 11. There is no watch profile. |
| Platform delivery | **Not met** | `adapter.rs` defines the trait and capability negotiation. **No platform adapter exists**: nothing bridges to AT-SPI, UIA, or AX. An application can now assemble its semantic forest via `UserInterface::semantics`, and there is nothing at the far end to deliver it to. |

## Follow-ups carried forward

1. **A real platform adapter.** AT-SPI (Linux/VYOM) first. Until one
   exists, every guarantee here is inspectable but not *usable* by an
   actual screen reader — the largest remaining gap in this phase.
2. **Semantics for the remaining widgets**, and real widgets for the
   `aura-design` components that are contracts only (`bottom_sheet`,
   `menu`, `toast`, `notification`, `navigation`, `window_chrome`).
3. **`Role::Image` is reported by nothing**, so an `image` has no
   alternative text and no way to declare one. A decorative image should
   stay silent; a meaningful one needs an `alt`-style API on the widget.
   Until then, an icon-only control that is not an `icon_button` (which
   requires its label) announces nothing.
4. **`pick_list`'s options have no nodes of their own.** The dropdown now
   announces itself and that it is expanded, but its options are drawn
   directly by the overlay rather than being child widgets, so a screen
   reader cannot yet enumerate them. Same for `combo_box`.
5. **A virtualized-content focus test** against `virtual_list`. The
   `reconcile` path is designed for it and unit-tested with synthetic
   removals, but not yet driven through a real virtualized scroll.
6. Large-text policy tests, and a watch profile if VYOM targets one.
7. A `focus_scope` widget: the engine supports scopes and traps, but
   nothing declares one declaratively yet -- though `dialog` now proves
   the engine side works end-to-end.
