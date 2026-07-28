# Text, editing, and IME contracts

> Status: Phase 10 baseline. This document describes the shaping-cache
> identity, the editing model, the IME composition lifecycle, and — at
> greatest length, because it is the highest-risk part — the secure-field
> disclosure boundary and what still enforces it only by convention.

## Shaping identity

`core::text::paragraph::ShapingKey` is the cache identity for a shaped
run. It carries every input that can change the resulting glyphs:

| Field | Why it is part of identity |
|---|---|
| `script` | A run shaped for Devanagari must never be served from an entry shaped for Arabic. |
| `fallback` | Follows deterministically from the script (`FallbackChain::for_script`), with no platform font-database lookup, so cache keys reproduce across devices. |
| `locale` | Locale-aware shaping and Han unification make the same codepoints genuinely different runs. |
| `direction` | A mirrored run must not reuse an LTR shaping. |
| `axes` | Variable-font axes, quantized by bit pattern and sorted by tag. |

Two properties follow, and both are pinned by
`tests/golden/multilingual_text.rs`:

- **Every shaping-affecting input discriminates.** Changing script,
  locale, direction, or any axis value yields a different key and a
  different hash. The whole mandatory language corpus, at every reference
  scale factor, produces pairwise-distinct keys.
- **Nothing else does.** Supplying the same axes in a different order is
  the same shaping and hits the same entry; re-setting an axis replaces
  rather than appends. Identical inputs produce an identical key *and*
  hash, which is what "no reshaping for unchanged text" means
  mechanically.

## Editing

Editing is grapheme-aware, not codepoint- or byte-aware:
`widget::text_input::Value` stores graphemes, so an emoji ZWJ sequence
(`👨‍👩‍👧‍👦`) and a combining pair (`e` + U+0301) are each **one** unit for
movement, selection, and deletion.

`core::text::Selection` holds logical ranges that are always valid UTF-8
boundaries; a mid-codepoint range is rejected at construction rather than
panicking later. Renderers translate a logical range into one or more
visual rectangles, because a single logical selection in bidirectional
text is visually discontiguous.

## The IME composition lifecycle

```
Opened ──► Preedit(content, selection)* ──► Commit(text) ──► (empty Preedit)
   │                     │
   └────────► Closed ◄───┘        (cancellation: composition is discarded)
```

- `Opened` starts a session with an empty composition.
- `Preedit` updates the in-flight composition. It **never** publishes a
  message — otherwise every keystroke of a Devanagari or Japanese
  composition would fire an `on_input`.
- `Commit` publishes the composed text exactly once.
- `Closed` **discards** whatever was in flight. Committing an abandoned
  composition would insert text the user never confirmed.

`tests/text/ime.rs` drives a real `TextInput` through each of these paths
rather than asserting on the event enum, because the property that
matters lives in the widget's state machine. The composition is observed
through `Shell::request_input_method` — the real path by which it reaches
the runtime — rather than a test-only accessor. That is deliberate: a
public `State::preedit()` getter would be a fresh disclosure channel for
exactly the content the next section protects.

## Secure fields: the disclosure boundary

`phase.md` names six channels a secure field must never expose content
through. Each has a test in `tests/text/security.rs` that *attempts* the
disclosure and asserts it fails.

| Channel | Enforcement | Status |
|---|---|---|
| Surrounding-text APIs | `input_method::SurroundingText::for_field(.., secure: true)` returns `Redacted` — not bullets, not a length-preserving mask, since either still discloses. | **Enforced in code** |
| Semantics value | `semantics::Node::value` is a **private** field; `with_value` refuses for `Role::SecureField`, and `Node::value()` returns `None` for it regardless. Both paths enforce, so a password cannot reach an assistive-technology bridge. | **Enforced in code** |
| Debug logs | `text_input::Value` has a **manual** `Debug` that prints `Value(<redacted>)` — no content, and no length either. | **Enforced in code** |
| Clipboard | Copy/cut are gated on `!is_secure` in `TextInput::update`; the rendered/copied value is `Value::secure()`, which masks every *grapheme* to exactly one bullet. | **Enforced in code** |
| IME composition | A secure field withholds its preedit from `InputMethod::Enabled` entirely; the widget renders its own masked composition locally. | **Enforced in code** |
| Retained screenshots / profiler captures | `input_method::Purpose::Secure` is the **signal** a backend maps onto the platform's own capture policy (`FLAG_SECURE` on Android, compositor policy on Wayland). | **Signal only — see below** |

### Why `Value`'s Debug redacts length too

`Value` cannot know whether the field it belongs to is secure — that flag
lives on `TextInput`, one layer up — so it must default to denying
disclosure. It withholds length as well as content because this codebase
already holds that line for `SurroundingText` ("bullets or a
length-preserving mask ... still disclose[] information"), and a
password's length is exactly what that principle protects. Code that
genuinely needs the content calls `Value::to_string()`, which is an
auditable choice at the call site rather than an accident of formatting.

This matters more than it looks: nobody debug-prints a `Value` directly.
They debug-print the struct that *holds* one, and a derived `Debug` would
spill the plaintext into every log line, panic message, and crash report
that formatted it.

### Why `with_value` fails safe instead of panicking

A `Role::SecureField` node silently ignores `with_value` rather than
asserting. A crash on the exact code path that handles a password turns a
caller's mistake into a denial of service, and panic messages are their
own disclosure channel. There is no legitimate reason to give a secure
field a semantic value, so there is nothing a louder failure would save.
Refusal *is* the guarantee, and the private field means `with_value` is
the only way in.

## Untrusted text cannot allocate without bound

- A pathological combining run (2,000 combining acutes on one base) stays
  **one** grapheme, so per-grapheme costs downstream stay bounded by what
  the user can actually see.
- A 100,000-character paste stays proportional, and word navigation over
  it terminates in bounds.
- Shaping-cache entries are keyed by `ShapingKey` and byte-bounded by
  `graphics::text::cache`.

## What Phase 10 does not close

| Clause | Status | Evidence |
|---|---|---|
| "secure-field threat tests pass" | **Met for 5 of 6 channels** | 14 tests in `tests/text/security.rs`, each attempting a real disclosure. Verified non-vacuous: reintroducing the `derive(Debug)` makes the debug-log tests fail. |
| "cache keys include every shaping-affecting input" | **Met** | `tests/golden/multilingual_text.rs`: every input discriminates; order and re-setting do not; the whole corpus × every scale is pairwise distinct. |
| "no reshaping for unchanged text" | **Met at the key level** | Identical inputs produce an identical key and hash. Whether the *cache* then serves the hit is `graphics::text::cache`'s own tested concern. |
| "corpus and editing tests pass" | **Met** | The full mandatory matrix (English, Devanagari, Gurmukhi, Arabic, Bengali, Tamil, emoji ZWJ, combining marks, mixed BiDi) with deterministic fallbacks and safe break offsets. |
| Screenshot / profiler exclusion | **Not enforced here** | `Purpose::Secure` exists and is distinct, and the widget sets it. But **no backend consumes it**: nothing in `aura_winit`/`aura_wgpu` maps it onto `FLAG_SECURE` or a compositor policy. A screenshot today would capture a password field. This is the one channel of the six that is a signal without a consumer. |
| "text-heavy P95 improves ... with significantly fewer repeated shapes/uploads" | **Not measured** | `tools/aura_bench/src/scenes/text_edit.rs` exists, but no before/after P95 or shape/upload counts were recorded this phase. |
| Password reveal "timed/policy-controlled" | **Partially met** | Reveal is *explicit* (masked and revealed values are observably different; there is no ambient path to the content) and testable. There is no built-in **timed** auto-remask or policy hook — an application that adds a reveal toggle must implement the timeout itself. |

## Follow-ups carried forward

1. **Consume `Purpose::Secure` in a backend.** Map it onto `FLAG_SECURE`
   (Android) and the compositor's capture policy (Wayland). Until then,
   screenshot/profiler exclusion is a signal nobody acts on — the single
   most important remaining item in this phase.
2. A timed/policy-controlled password-reveal affordance, with the
   accessible announcement its requirement calls for.
3. Real multilingual pixel goldens, which need the licensed reference
   font `assets/fonts/test/README.md` describes and human visual approval.
4. Text-heavy P95 and repeated-shape/upload measurement through
   `aura_bench`'s `text_edit` scene.
