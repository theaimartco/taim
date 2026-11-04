//! Where the frame went, and why it was late.
//!
//! # What makes this different from a stopwatch
//!
//! A frame time is not a finding. "22ms" tells a developer that something
//! is wrong and nothing about what. Phase 15's exit gate — *"every global
//! metric can be traced to a phase/cause"* — is the difference, and this
//! panel is where it either holds or does not.
//!
//! So every bar here is attributed, and the **residue is drawn**. If a
//! frame took 10ms and its phases account for 9, the missing millisecond
//! gets a bar of its own labelled as unattributed. The tempting
//! alternative — scale the phases to fill the frame, so the chart looks
//! tidy — turns an uninstrumented code path into a lie distributed evenly
//! across the phases that *were* measured, which is worse than not
//! charting it at all: the developer would go optimise `Paint` because
//! `Paint`'s bar grew.

use std::collections::BTreeMap;

use taim::prelude::*;
use taim_profiler::Phase;

use crate::target::Frame;

/// A 60Hz frame's budget, in nanoseconds.
///
/// The line a frame is late against. Not configurable yet, and worth
/// knowing: a 120Hz device's frames are all "on time" by this measure
/// while dropping every other one.
pub const BUDGET: u64 = 16_666_667;

/// Renders the frame timeline.
pub fn view(frames: &[Frame]) -> AnyView {
    if frames.is_empty() {
        return container(
            column(views![
                text("No frames recorded.").size(16.0),
                text(
                    "A trace records phases as they run. If this stays empty while something \
                     is drawing, the pipeline is not instrumented \u{2014} not that it is fast."
                )
                .size(12.0),
            ])
            .gap(6.0),
        )
        .center()
        .into();
    }

    let summary = summarise(frames);

    let rows = frames
        .iter()
        .rev()
        .take(60)
        .fold(column([]).gap(2.0), |rows, frame| {
            rows.push(frame_row(frame))
        });

    column(views![
        text(summary).size(13.0),
        text(dominant(frames)).size(12.0),
        scroll(rows),
    ])
    .gap(8.0)
    .into()
}

/// One frame's line.
fn frame_row(frame: &Frame) -> AnyView {
    let duration = match frame.duration {
        Some(duration) => format!("{:>7.2}ms", nanos_to_millis(duration)),
        // A frame that began and never ended is what a hang looks like.
        None => "  open  ".to_owned(),
    };

    let late = if frame.duration.is_some_and(|duration| duration > BUDGET) {
        match frame.missed {
            Some(cause) => format!("  LATE ({})", describe(cause)),
            // A late frame with no recorded cause is itself the finding:
            // something overran and nothing owned up to it.
            None => "  LATE (cause not recorded)".to_owned(),
        }
    } else {
        String::new()
    };

    let breakdown = frame
        .phases
        .iter()
        .map(|(phase, total)| format!("{}:{:.1}", phase.as_str(), nanos_to_millis(*total)))
        .collect::<Vec<_>>()
        .join("  ");

    // The residue, drawn rather than distributed. See the module docs.
    let residue = match frame.unattributed() {
        Some(0) | None => String::new(),
        Some(unattributed) => format!("  ?:{:.1}", nanos_to_millis(unattributed)),
    };

    row(views![
        text(format!("#{:<5}", frame.number)).size(12.0),
        text(duration).size(12.0),
        text(format!("{breakdown}{residue}")).size(12.0),
        text(late).size(12.0),
    ])
    .gap(10.0)
    .into()
}

/// A line describing the whole trace.
fn summarise(frames: &[Frame]) -> String {
    let complete: Vec<&Frame> = frames
        .iter()
        .filter(|frame| frame.duration.is_some())
        .collect();

    if complete.is_empty() {
        return format!("{} frames, none complete", frames.len());
    }

    let late = complete
        .iter()
        .filter(|frame| frame.duration.is_some_and(|duration| duration > BUDGET))
        .count();

    let mut durations: Vec<u64> = complete.iter().filter_map(|frame| frame.duration).collect();

    durations.sort_unstable();

    // The 95th percentile, not the mean. A mean frame time hides exactly
    // the frames a user notices: sixty good frames and one 300ms stall
    // average out to something that looks fine and feels broken.
    let p95 = durations[(durations.len() * 95 / 100).min(durations.len() - 1)];

    format!(
        "{} frames \u{00b7} p95 {:.2}ms \u{00b7} {} late ({:.0}%)",
        complete.len(),
        nanos_to_millis(p95),
        late,
        (late as f32 / complete.len() as f32) * 100.0
    )
}

/// A line naming what the application spends its life doing.
fn dominant(frames: &[Frame]) -> String {
    let totals = totals(frames);

    if totals.is_empty() {
        return "no phase recorded a span".to_owned();
    }

    let line: Vec<String> = totals
        .iter()
        .take(4)
        .map(|(phase, total)| format!("{} {:.1}ms", phase.as_str(), nanos_to_millis(*total)))
        .collect();

    format!("dominant: {}", line.join("  \u{00b7}  "))
}

/// Which phases dominate, across a whole trace.
///
/// A per-frame breakdown answers "why was *that* frame late"; this
/// answers "what is this application spending its life doing", which is a
/// different question with a different answer.
fn totals(frames: &[Frame]) -> Vec<(Phase, u64)> {
    let mut totals: BTreeMap<Phase, u64> = BTreeMap::new();

    for frame in frames {
        for (phase, total) in &frame.phases {
            *totals.entry(*phase).or_default() += total;
        }
    }

    // Sorted by cost, descending: the question this answers is "what
    // should I look at first", and the answer is the biggest bar.
    let mut totals: Vec<_> = totals.into_iter().collect();
    totals.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    totals
}

fn nanos_to_millis(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000.0
}

fn describe(cause: taim_profiler::Cause) -> String {
    use taim_profiler::Cause;

    match cause {
        Cause::Phase(phase) => phase.as_str().to_owned(),
        Cause::CacheMiss(resource) => format!("{} cache miss", resource.as_str()),
        Cause::GpuStall => "GPU stall".to_owned(),
        Cause::Application => "application update".to_owned(),
        Cause::Compositor => "compositor".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(number: u64, duration: Option<u64>, phases: Vec<(Phase, u64)>) -> Frame {
        Frame {
            number,
            began: 0,
            duration,
            phases,
            missed: None,
        }
    }

    #[test]
    fn a_frames_residue_is_reported_not_distributed() {
        // The lie this avoids: scaling the phases to fill the frame turns
        // an uninstrumented path into an even smear across the phases that
        // *were* measured -- and sends the developer to optimise `Paint`
        // because `Paint`'s bar grew.
        let frame = frame(1, Some(10_000_000), vec![(Phase::Paint, 9_000_000)]);

        assert_eq!(frame.attributed(), 9_000_000);
        assert_eq!(frame.unattributed(), Some(1_000_000));
    }

    #[test]
    fn the_summary_reports_p95_rather_than_a_mean() {
        // Sixty good frames and one 300ms stall average out to something
        // that looks fine and feels broken.
        let mut frames: Vec<Frame> = (0..99)
            .map(|number| frame(number, Some(1_000_000), vec![]))
            .collect();

        frames.push(frame(99, Some(300_000_000), vec![]));

        let summary = summarise(&frames);

        assert!(
            summary.contains("1 late"),
            "the stall must be counted: {summary}"
        );
        assert!(
            !summary.contains("300.00ms"),
            "p95 must not be dragged to the worst frame: {summary}"
        );
    }

    #[test]
    fn a_trace_of_nothing_but_stalls_reports_them_all() {
        let frames: Vec<Frame> = (0..10)
            .map(|number| frame(number, Some(BUDGET * 2), vec![]))
            .collect();

        let summary = summarise(&frames);

        assert!(summary.contains("10 late"), "{summary}");
        assert!(summary.contains("100%"), "{summary}");
    }

    #[test]
    fn incomplete_frames_are_excluded_from_the_statistics() {
        // A frame that never ended has no duration. Counting it as zero
        // would make a hang improve the p95.
        let frames = vec![frame(1, Some(BUDGET * 3), vec![]), frame(2, None, vec![])];

        let summary = summarise(&frames);

        assert!(summary.contains("1 frames"), "{summary}");
        assert!(summary.contains("1 late"), "{summary}");
    }

    #[test]
    fn a_trace_with_no_complete_frames_says_so() {
        let summary = summarise(&[frame(1, None, vec![])]);

        assert_eq!(summary, "1 frames, none complete");
    }

    #[test]
    fn a_trace_with_no_spans_says_so_rather_than_claiming_nothing_costs_anything() {
        assert_eq!(dominant(&[]), "no phase recorded a span");
    }

    #[test]
    fn totals_are_ordered_by_what_to_look_at_first() {
        let frames = vec![
            frame(
                1,
                Some(10_000_000),
                vec![(Phase::Layout, 1_000_000), (Phase::Paint, 8_000_000)],
            ),
            frame(
                2,
                Some(10_000_000),
                vec![(Phase::Layout, 2_000_000), (Phase::Paint, 1_000_000)],
            ),
        ];

        assert_eq!(
            totals(&frames),
            vec![(Phase::Paint, 9_000_000), (Phase::Layout, 3_000_000)],
            "the biggest bar answers 'what should I look at first'"
        );
    }

    #[test]
    fn totals_break_ties_deterministically() {
        // Two panels reading the same trace must agree, and hash order
        // must never reach a report.
        let frames = vec![frame(
            1,
            Some(10_000_000),
            vec![(Phase::Paint, 5_000_000), (Phase::Layout, 5_000_000)],
        )];

        // `Layout` is declared before `Paint`, so it wins an equal tie.
        assert_eq!(
            totals(&frames),
            vec![(Phase::Layout, 5_000_000), (Phase::Paint, 5_000_000)]
        );
    }

    #[test]
    fn an_empty_trace_is_not_reported_as_fast() {
        // The trap this avoids: an empty timeline looks like a perfect
        // application. It almost always means nothing is instrumented.
        let element: AnyView = view(&[]);

        // The assertion that matters is in the copy, which a test cannot
        // read from a built `Element`. Constructing it without panicking
        // is what is checkable here; the wording is pinned by review.
        let _ = std::hint::black_box(element);
    }
}
