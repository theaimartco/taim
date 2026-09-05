//! Pillar VII's flat-cost signal workload.
//!
//! Both registered variants mutate one logical row every measured frame.
//! Their only difference is whether the virtual list contains 100,000 or
//! 1,000,000 rows, making a comparison isolate scaling with logical length.

use crate::scenes::{Element, Scene};

use taim_core::{Color, Event, Length};
use taim_signal::{Readable, Signal};
use taim_widget::{Space, container};

pub const HUNDRED_THOUSAND_NAME: &str = "signal_list_100k";
pub const MILLION_NAME: &str = "signal_list_1m";

const ROW_HEIGHT: f32 = 24.0;

#[derive(Debug)]
struct SignalList {
    rows: usize,
    target: usize,
    revision: Signal<u64>,
}

impl SignalList {
    fn new(rows: usize, seed: u64) -> Self {
        Self {
            rows,
            // Keep the mutation visible in the default viewport. The same
            // target is used for both logical lengths.
            target: (seed as usize) % 16,
            revision: Signal::new(0),
        }
    }

    fn view(&self) -> Element<'_, ()> {
        let rows = self.rows;
        let target = self.target;
        let revision = self.revision.clone();
        let source = revision.source_id().routing_token();

        let list = taim_widget::virtual_list::VirtualList::new(rows, ROW_HEIGHT, move |index| {
            if index == target {
                let row: Element<'static, ()> =
                    taim_widget::reactive_value(revision.clone(), move |revision| {
                        container(Space::new().width(Length::Fill).height(ROW_HEIGHT)).style(
                            move |_| {
                                let color = if revision.get().is_multiple_of(2) {
                                    Color::from_rgb(0.2, 0.6, 1.0)
                                } else {
                                    Color::from_rgb(1.0, 0.4, 0.2)
                                };

                                taim_widget::container::Style {
                                    background: Some(color.into()),
                                    ..Default::default()
                                }
                            },
                        )
                    })
                    .into();
                row
            } else {
                Element::from(Space::new().width(Length::Fill).height(ROW_HEIGHT))
            }
        })
        .route_reactive_value(move |candidate| (candidate == source).then_some(target))
        .width(Length::Fill)
        .height(Length::Fill);

        container(list).padding(8).into()
    }

    fn mutate(&self, frame_index: u64) {
        let _ = self.revision.set(frame_index.wrapping_add(1));
    }
}

#[derive(Debug)]
pub struct HundredThousand(SignalList);

impl HundredThousand {
    pub fn new(seed: u64) -> Self {
        Self(SignalList::new(100_000, seed))
    }
}

#[derive(Debug)]
pub struct Million(SignalList);

impl Million {
    pub fn new(seed: u64) -> Self {
        Self(SignalList::new(1_000_000, seed))
    }
}

macro_rules! impl_scene {
    ($type:ty, $name:expr) => {
        impl Scene for $type {
            type Message = ();

            fn name(&self) -> &'static str {
                $name
            }

            fn view(&self) -> Element<'_, Self::Message> {
                self.0.view()
            }

            fn update(&mut self, (): Self::Message) {}

            fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
                Vec::new()
            }

            fn input_events(&self, _frame_index: u64) -> Vec<Event> {
                vec![Event::Reactive(self.0.revision.source_id().routing_token())]
            }

            fn before_update(&self, frame_index: u64) {
                self.0.mutate(frame_index);
            }
        }
    };
}

impl_scene!(HundredThousand, HUNDRED_THOUSAND_NAME);
impl_scene!(Million, MILLION_NAME);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_differ_only_in_logical_row_count() {
        let small = HundredThousand::new(42);
        let large = Million::new(42);

        assert_eq!(small.0.rows, 100_000);
        assert_eq!(large.0.rows, 1_000_000);
        assert_eq!(small.0.target, large.0.target);
    }

    #[test]
    fn every_measured_frame_advances_the_signal_revision() {
        let scene = HundredThousand::new(42);

        scene.before_update(9);

        assert_eq!(scene.0.revision.get(), 10);
        assert_eq!(scene.0.revision.revision().value(), 1);
    }
}
