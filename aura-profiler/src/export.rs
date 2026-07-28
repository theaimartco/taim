//! Turning a trace into something a tool can read.
//!
//! Two formats, for two audiences:
//!
//! - [`chrome`] — the Trace Event Format, which `chrome://tracing` and
//!   Perfetto already open. A developer with a performance question wants
//!   a flame chart *now*, not after installing something.
//! - [`aura`] — everything, including what Chrome's format has no place
//!   for: drop accounting, phase attribution, redaction markers.
//!
//! # No serde
//!
//! Both writers are hand-rolled. That is deliberate: an export path that
//! derives `Serialize` on the event types is one `#[derive]` away from
//! serialising a field someone later adds — and the whole privacy design
//! rests on nothing being written that was not written on purpose. A
//! hand-rolled writer means adding a field to a trace is a visible edit
//! here, in the file whose tests are about exactly that.

use crate::collector::{Collector, Dropped};
use crate::events::{Cause, Event, Nanos, Phase, SCHEMA_VERSION, Sensitivity};

/// Escapes a string for JSON.
///
/// Only ever called on [`Label`](crate::events::Label)s and our own
/// constants — never on user content, which cannot reach here — but
/// correct escaping is not optional: a label with a quote in it would
/// otherwise produce a trace no tool can parse.
fn escape(value: &str, out: &mut String) {
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
}

/// Writes `value` as a JSON string.
fn string(value: &str, out: &mut String) {
    out.push('"');
    escape(value, out);
    out.push('"');
}

/// The name a [`Cause`] exports under.
fn cause_name(cause: Cause) -> String {
    match cause {
        Cause::Phase(phase) => format!("phase:{}", phase.as_str()),
        Cause::CacheMiss(resource) => format!("cache_miss:{}", resource.as_str()),
        Cause::GpuStall => "gpu_stall".to_owned(),
        Cause::Application => "application".to_owned(),
        Cause::Compositor => "compositor".to_owned(),
    }
}

/// The name a [`Sensitivity`] exports under.
fn sensitivity_name(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Private => "private",
        Sensitivity::Secret => "secret",
        Sensitivity::Protected => "protected",
    }
}

/// Exports `events` as Chrome's Trace Event Format.
///
/// Timestamps are **microseconds** there, not nanoseconds: the format
/// says so, and a trace that ignores it renders a 16ms frame as 16
/// seconds — which looks like a catastrophic hang rather than a unit bug.
pub fn chrome(events: &[Event]) -> String {
    let mut out = String::from("{\"traceEvents\":[");
    let mut first = true;

    for event in events {
        if first {
            first = false;
        } else {
            out.push(',');
        }

        let micros = |at: Nanos| at as f64 / 1_000.0;

        match event {
            Event::FrameBegin { frame, at } => {
                out.push_str(&format!(
                    "{{\"name\":\"frame {frame}\",\"ph\":\"B\",\"pid\":1,\"tid\":1,\"ts\":{}}}",
                    micros(*at)
                ));
            }
            Event::FrameEnd { at, .. } => {
                out.push_str(&format!(
                    "{{\"ph\":\"E\",\"pid\":1,\"tid\":1,\"ts\":{}}}",
                    micros(*at)
                ));
            }
            Event::PhaseBegin { phase, at } => {
                out.push_str("{\"name\":");
                string(phase.as_str(), &mut out);
                out.push_str(&format!(
                    ",\"ph\":\"B\",\"pid\":1,\"tid\":1,\"ts\":{}}}",
                    micros(*at)
                ));
            }
            Event::PhaseEnd { at, .. } => {
                out.push_str(&format!(
                    "{{\"ph\":\"E\",\"pid\":1,\"tid\":1,\"ts\":{}}}",
                    micros(*at)
                ));
            }
            Event::Missed { frame, over, cause } => {
                out.push_str(&format!(
                    "{{\"name\":\"missed frame {frame}\",\"ph\":\"i\",\"s\":\"g\",\
                     \"pid\":1,\"tid\":1,\"ts\":0,\"args\":{{\"over_us\":{},\"cause\":",
                    micros(*over)
                ));
                string(&cause_name(*cause), &mut out);
                out.push_str("}}");
            }
            Event::CacheHit { resource, at } | Event::CacheMiss { resource, at, .. } => {
                let hit = matches!(event, Event::CacheHit { .. });

                out.push_str("{\"name\":");
                string(
                    &format!("{} {}", resource.as_str(), if hit { "hit" } else { "miss" }),
                    &mut out,
                );
                out.push_str(&format!(
                    ",\"ph\":\"i\",\"s\":\"t\",\"pid\":1,\"tid\":1,\"ts\":{}}}",
                    micros(*at)
                ));
            }
            Event::Evicted {
                resource,
                bytes,
                at,
            } => {
                out.push_str("{\"name\":");
                string(&format!("{} evicted", resource.as_str()), &mut out);
                out.push_str(&format!(
                    ",\"ph\":\"i\",\"s\":\"p\",\"pid\":1,\"tid\":1,\"ts\":{},\
                     \"args\":{{\"bytes\":{bytes}}}}}",
                    micros(*at)
                ));
            }
            Event::Input { kind, at } => {
                out.push_str("{\"name\":");
                string(kind.as_str(), &mut out);
                out.push_str(&format!(
                    ",\"ph\":\"i\",\"s\":\"t\",\"pid\":1,\"tid\":1,\"ts\":{}}}",
                    micros(*at)
                ));
            }
            Event::Semantics { nodes, rebuilt, at } => {
                out.push_str(&format!(
                    "{{\"name\":\"semantics\",\"ph\":\"i\",\"s\":\"t\",\"pid\":1,\"tid\":1,\
                     \"ts\":{},\"args\":{{\"nodes\":{nodes},\"rebuilt\":{rebuilt}}}}}",
                    micros(*at)
                ));
            }
            Event::Withheld { at, content } => {
                out.push_str(&format!(
                    "{{\"name\":\"withheld\",\"ph\":\"i\",\"s\":\"t\",\"pid\":1,\"tid\":1,\
                     \"ts\":{},\"args\":{{\"sensitivity\":",
                    micros(*at)
                ));
                string(sensitivity_name(content.sensitivity()), &mut out);

                // Present only when the classification allows it: a secure
                // field's length is withheld even here.
                if let Some(length) = content.length() {
                    out.push_str(&format!(",\"length\":{length}"));
                }

                out.push_str("}}");
            }
        }
    }

    out.push_str("]}");
    out
}

/// How long each [`Phase`] took, in total.
///
/// This is the exit gate's "every global metric can be traced to a
/// phase/cause", computed: a frame time is the sum of these plus whatever
/// is unattributed, and unattributed time is itself the finding.
pub fn phase_totals(events: &[Event]) -> Vec<(Phase, Nanos)> {
    let mut open: Vec<(Phase, Nanos)> = Vec::new();
    let mut totals: rustc_hash::FxHashMap<Phase, Nanos> = rustc_hash::FxHashMap::default();

    for event in events {
        match event {
            Event::PhaseBegin { phase, at } => open.push((*phase, *at)),
            Event::PhaseEnd { phase, at } => {
                // Match the most recent open of the *same* phase, rather
                // than the most recent open of any: phases nest, and a
                // stack that pops blindly would charge `Layout`'s time to
                // `Paint` the moment one contains the other.
                if let Some(index) = open.iter().rposition(|(open, _)| open == phase) {
                    let (phase, began) = open.remove(index);

                    *totals.entry(phase).or_default() += at.saturating_sub(began);
                }
            }
            _ => {}
        }
    }

    let mut totals: Vec<_> = totals.into_iter().collect();
    // Sorted by phase, not by duration: a stable order means two reports
    // can be diffed, and hash iteration order must never reach a report.
    totals.sort_by_key(|(phase, _)| *phase);
    totals
}

/// Exports a full Aura trace: the events, the phase attribution, and what
/// was dropped.
pub fn aura(collector: &Collector) -> String {
    let events = collector.events();

    aura_from(&events, collector.dropped())
}

/// As [`aura`], from parts.
pub fn aura_from(events: &[Event], dropped: Dropped) -> String {
    let mut out = String::new();

    // The schema version leads, because a trace outlives the build that
    // made it: a bug report read weeks later by different tools must not
    // have to guess.
    out.push_str(&format!("{{\"schema\":{SCHEMA_VERSION},\"dropped\":{{"));
    out.push_str(&format!(
        "\"overflow\":{},\"sampled\":{},\"contended\":{}}},",
        dropped.overflow, dropped.sampled, dropped.contended
    ));

    out.push_str("\"phases\":[");

    for (index, (phase, total)) in phase_totals(events).iter().enumerate() {
        if index > 0 {
            out.push(',');
        }

        out.push_str("{\"phase\":");
        string(phase.as_str(), &mut out);
        out.push_str(&format!(",\"total_ns\":{total}}}"));
    }

    out.push_str("],\"trace\":");
    out.push_str(&chrome(events));
    out.push('}');

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Label, Redacted, Resource};

    #[test]
    fn chrome_timestamps_are_microseconds_not_nanoseconds() {
        // The format says microseconds. A trace that writes nanoseconds
        // renders a 16ms frame as 16 seconds -- which looks like a
        // catastrophic hang rather than a unit bug.
        let trace = chrome(&[Event::FrameBegin {
            frame: 1,
            at: 16_000_000,
        }]);

        assert!(
            trace.contains("\"ts\":16000"),
            "16ms must export as 16000us: {trace}"
        );
        assert!(!trace.contains("16000000"));
    }

    #[test]
    fn a_missed_frame_carries_its_cause_into_the_trace() {
        // The exit gate: a metric a developer cannot trace to a cause is a
        // number they can do nothing with.
        let trace = chrome(&[Event::Missed {
            frame: 3,
            over: 4_000_000,
            cause: Cause::CacheMiss(Resource::Text),
        }]);

        assert!(trace.contains("\"cause\":\"cache_miss:text\""), "{trace}");
        assert!(trace.contains("\"over_us\":4000"), "{trace}");
    }

    #[test]
    fn a_secret_withholding_exports_no_length() {
        // The privacy rule, at the last place it could leak.
        let trace = chrome(&[Event::Withheld {
            at: 0,
            content: Redacted::new(12, Sensitivity::Secret),
        }]);

        assert!(trace.contains("\"sensitivity\":\"secret\""), "{trace}");
        assert!(
            !trace.contains("length"),
            "a password's length must not reach a trace: {trace}"
        );
        assert!(!trace.contains("12"), "{trace}");
    }

    #[test]
    fn a_public_withholding_exports_its_length_but_no_content() {
        let trace = chrome(&[Event::Withheld {
            at: 0,
            content: Redacted::new(4096, Sensitivity::Public),
        }]);

        assert!(trace.contains("\"length\":4096"), "{trace}");
    }

    #[test]
    fn nested_phases_are_attributed_to_the_phase_that_opened_them() {
        // A stack that popped blindly would charge `Layout`'s time to
        // `Paint` the moment one contained the other.
        let totals = phase_totals(&[
            Event::PhaseBegin {
                phase: Phase::Paint,
                at: 0,
            },
            Event::PhaseBegin {
                phase: Phase::Layout,
                at: 10,
            },
            Event::PhaseEnd {
                phase: Phase::Layout,
                at: 30,
            },
            Event::PhaseEnd {
                phase: Phase::Paint,
                at: 100,
            },
        ]);

        assert_eq!(totals, vec![(Phase::Layout, 20), (Phase::Paint, 100)]);
    }

    #[test]
    fn phase_totals_accumulate_across_frames() {
        let totals = phase_totals(&[
            Event::PhaseBegin {
                phase: Phase::Layout,
                at: 0,
            },
            Event::PhaseEnd {
                phase: Phase::Layout,
                at: 5,
            },
            Event::PhaseBegin {
                phase: Phase::Layout,
                at: 10,
            },
            Event::PhaseEnd {
                phase: Phase::Layout,
                at: 17,
            },
        ]);

        assert_eq!(totals, vec![(Phase::Layout, 12)]);
    }

    #[test]
    fn an_unclosed_phase_contributes_nothing_rather_than_a_wild_total() {
        // A trace truncated mid-phase is ordinary -- the ring buffer drops
        // the oldest. Charging the unclosed phase some invented duration
        // would put a fictional bar on the flame chart.
        let totals = phase_totals(&[Event::PhaseBegin {
            phase: Phase::Paint,
            at: 0,
        }]);

        assert!(totals.is_empty());
    }

    #[test]
    fn a_phase_end_with_no_begin_is_ignored() {
        // The other half of a truncated trace: the `PhaseBegin` was
        // dropped and the `PhaseEnd` survived.
        let totals = phase_totals(&[Event::PhaseEnd {
            phase: Phase::Paint,
            at: 500,
        }]);

        assert!(
            totals.is_empty(),
            "an end with no begin must not charge time from zero"
        );
    }

    #[test]
    fn phase_totals_are_ordered_stably_so_reports_can_be_diffed() {
        let events = vec![
            Event::PhaseBegin {
                phase: Phase::Paint,
                at: 0,
            },
            Event::PhaseEnd {
                phase: Phase::Paint,
                at: 1,
            },
            Event::PhaseBegin {
                phase: Phase::Input,
                at: 2,
            },
            Event::PhaseEnd {
                phase: Phase::Input,
                at: 3,
            },
        ];

        // `Input` is declared before `Paint`, so it sorts first regardless
        // of when it happened -- hash order must never reach a report.
        assert_eq!(
            phase_totals(&events)
                .iter()
                .map(|(phase, _)| *phase)
                .collect::<Vec<_>>(),
            vec![Phase::Input, Phase::Paint]
        );
    }

    #[test]
    fn an_aura_trace_leads_with_its_schema_version() {
        // A trace outlives the build that made it.
        let export = aura_from(&[], Dropped::default());

        assert!(export.starts_with("{\"schema\":1,"), "{export}");
    }

    #[test]
    fn an_aura_trace_reports_what_was_dropped() {
        // A trace with a hole that does not say so is a trace that lies.
        let export = aura_from(
            &[],
            Dropped {
                overflow: 7,
                sampled: 3,
                contended: 1,
            },
        );

        assert!(export.contains("\"overflow\":7"), "{export}");
        assert!(export.contains("\"sampled\":3"), "{export}");
        assert!(export.contains("\"contended\":1"), "{export}");
    }

    #[test]
    fn labels_are_escaped() {
        // Only our own constants reach here, but a label with a quote
        // would otherwise produce a trace no tool can parse.
        let trace = chrome(&[Event::Input {
            kind: Label::new("say \"hello\"\n"),
            at: 0,
        }]);

        assert!(trace.contains(r#"say \"hello\"\n"#), "{trace}");
    }

    #[test]
    fn an_empty_trace_is_still_valid_json() {
        assert_eq!(chrome(&[]), "{\"traceEvents\":[]}");
    }

    #[test]
    fn a_full_export_round_trips_through_the_collector() {
        let collector = Collector::enabled();

        collector.record(|| Event::PhaseBegin {
            phase: Phase::Layout,
            at: 0,
        });
        collector.record(|| Event::PhaseEnd {
            phase: Phase::Layout,
            at: 2_000_000,
        });

        let export = aura(&collector);

        assert!(export.contains("\"phase\":\"layout\""), "{export}");
        assert!(export.contains("\"total_ns\":2000000"), "{export}");
        assert!(export.contains("\"ts\":2000"), "{export}");
    }
}
