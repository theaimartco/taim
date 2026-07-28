//! How this looks on something else.
//!
//! # Simulation, not preview
//!
//! This does not scale a picture. It **relays the target out** at the
//! chosen surface, through the real layout pass, and every other panel
//! then shows that layout.
//!
//! The distinction is the whole point. A phone is not a desktop with
//! fewer pixels: a `row` becomes a `column`, a sidebar becomes a sheet,
//! text wraps where it did not. A preview that scaled a desktop render
//! down would show a developer a picture that no phone will ever produce,
//! and they would ship it.
//!
//! # What a profile is, and is not
//!
//! A viewport and a scale factor. That is genuinely all Aura's layout
//! consumes, so it is all a profile can honestly set.
//!
//! `phase.md` also asks for "capability simulation" — pretending a device
//! lacks a GPU feature, or has a slower one. Nothing here does that, and
//! it would be a lie to imply otherwise: the target runs on this machine's
//! renderer whatever profile is selected. See [`MISSING`].

use aura::widget::{button, column, container, row, text};
use aura::{Element, Length, Size};

/// What a profile cannot simulate, and why.
pub const MISSING: &str = "Capabilities are not simulated. A profile sets a viewport and a scale \
                           factor \u{2014} which is all Aura's layout consumes \u{2014} and the target still \
                           runs on this machine's renderer. Selecting \u{201c}watch\u{201d} does not give you a \
                           watch's GPU, and nothing here pretends it does.";

/// A device to lay the target out as.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Profile {
    /// What to call it.
    pub name: &'static str,
    /// Logical size.
    pub bounds: Size,
    /// Scale factor.
    pub scale: f32,
}

impl Profile {
    /// The profiles `phase.md` names: phone, tablet, desktop, watch.
    ///
    /// Real sizes rather than round numbers. A 360x780 phone is a shape
    /// layouts actually break on; a "400x800" one is a shape nobody ships
    /// and nothing breaks on, which would make the panel decorative.
    pub const ALL: [Profile; 4] = [
        Profile {
            name: "phone",
            bounds: Size {
                width: 360.0,
                height: 780.0,
            },
            scale: 3.0,
        },
        Profile {
            name: "tablet",
            bounds: Size {
                width: 834.0,
                height: 1112.0,
            },
            scale: 2.0,
        },
        Profile {
            name: "desktop",
            bounds: Size {
                width: 1440.0,
                height: 900.0,
            },
            scale: 1.0,
        },
        // The one that finds bugs. A watch is small enough that anything
        // assuming room breaks, and its scale factor is high enough that
        // anything confusing logical with physical pixels breaks too.
        Profile {
            name: "watch",
            bounds: Size {
                width: 184.0,
                height: 224.0,
            },
            scale: 2.0,
        },
    ];

    /// Physical pixels this profile renders.
    pub fn physical(&self) -> Size {
        Size::new(
            self.bounds.width * self.scale,
            self.bounds.height * self.scale,
        )
    }

    /// Whether `bounds`/`scale` match this profile.
    pub fn matches(&self, bounds: Size, scale: f32) -> bool {
        self.bounds == bounds && (self.scale - scale).abs() < f32::EPSILON
    }
}

/// Renders the profile picker.
pub fn view<'a, Message: 'a + Clone>(
    current: Size,
    scale: f32,
    on_select: impl Fn(Profile) -> Message,
) -> Element<'a, Message> {
    let buttons = Profile::ALL
        .iter()
        .fold(row![].spacing(6), |buttons, profile| {
            let selected = profile.matches(current, scale);

            buttons.push(
                button(text(profile.name).size(12))
                    .on_press(on_select(*profile))
                    .style(if selected {
                        button::primary
                    } else {
                        button::secondary
                    }),
            )
        });

    let rows = Profile::ALL
        .iter()
        .fold(column![].spacing(2), |rows, profile| {
            rows.push(
                row![
                    text(format!("{:<9}", profile.name)).size(12),
                    text(format!(
                        "{:>6.0}\u{00d7}{:<6.0} @{}x",
                        profile.bounds.width, profile.bounds.height, profile.scale
                    ))
                    .size(12),
                    text(format!(
                        "= {:.0}\u{00d7}{:.0} physical",
                        profile.physical().width,
                        profile.physical().height
                    ))
                    .size(11),
                ]
                .spacing(10),
            )
        });

    container(
        column![
            text(format!(
                "target: {:.0}\u{00d7}{:.0} @{scale}x",
                current.width, current.height
            ))
            .size(14),
            buttons,
            rows,
            text(MISSING).size(11),
        ]
        .spacing(10),
    )
    .padding(12)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_the_plan_names_is_present() {
        // `phase.md`: "phone/tablet/desktop/watch viewport and capability
        // simulation".
        let names: Vec<_> = Profile::ALL.iter().map(|profile| profile.name).collect();

        assert_eq!(names, vec!["phone", "tablet", "desktop", "watch"]);
    }

    #[test]
    fn physical_pixels_are_logical_times_scale() {
        // The distinction anything confusing the two breaks on -- which is
        // why the watch profile exists.
        let watch = Profile::ALL[3];

        assert_eq!(watch.physical(), Size::new(368.0, 448.0));
    }

    #[test]
    fn the_watch_is_small_enough_to_break_things() {
        // A profile nothing breaks on is a profile that finds nothing.
        let watch = Profile::ALL[3];

        assert!(watch.bounds.width < 200.0);
        assert!(watch.bounds.height < 250.0);
    }

    #[test]
    fn profiles_are_real_sizes_not_round_numbers() {
        // A "400x800" phone is a shape nobody ships and nothing breaks on,
        // which would make the panel decorative.
        let phone = Profile::ALL[0];

        assert_eq!(phone.bounds, Size::new(360.0, 780.0));
        assert_eq!(phone.scale, 3.0);
    }

    #[test]
    fn a_profile_recognises_itself() {
        for profile in Profile::ALL {
            assert!(
                profile.matches(profile.bounds, profile.scale),
                "{} must recognise its own geometry",
                profile.name
            );
        }
    }

    #[test]
    fn profiles_do_not_recognise_each_other() {
        // Two profiles matching the same geometry would light two buttons
        // and tell a developer nothing.
        let phone = Profile::ALL[0];
        let desktop = Profile::ALL[2];

        assert!(!phone.matches(desktop.bounds, desktop.scale));
    }

    #[test]
    fn scale_is_part_of_identity() {
        // The same viewport at a different scale is a different device,
        // and matching on bounds alone would confuse them.
        let phone = Profile::ALL[0];

        assert!(!phone.matches(phone.bounds, 1.0));
    }

    #[test]
    fn the_panel_says_capabilities_are_not_simulated() {
        // Selecting "watch" does not give you a watch's GPU, and nothing
        // here should imply it does.
        assert!(MISSING.contains("Capabilities"), "{MISSING}");
        assert!(MISSING.contains("renderer"), "{MISSING}");
    }
}
