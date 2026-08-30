# Vyom-Taim Design System

The Vyom-Taim Design System is a comprehensive set of guidelines, tokens, and components that ensure visual consistency, accessibility, and exceptional user experience across all applications built with the framework.

## 1. What is a Design System?

A design system is more than just a UI kit; it is the single source of truth for the visual language of an application. It defines how colors, typography, spacing, and motion combine to create a cohesive and premium feel.

In Vyom-Taim, the design system is codified into **Tokens**. Tokens are semantic representations of design decisions. Instead of hardcoding a hex color (e.g., `#FF0000`), you use a token (e.g., `ColorTokens::error`). This semantic abstraction allows the framework to seamlessly switch themes, adapt to user preferences, and guarantee accessibility contrast ratios.

## 2. Color Tokens Deep Dive

Vyom-Taim's color system is built on dynamic, semantic roles. The `ColorTokens` struct defines these roles.

### Roles
- **Primary**: The main brand color, used for primary actions and prominent UI elements.
- **OnPrimary**: The color used for text or icons placed on top of a Primary background.
- **Surface**: The background color for cards, sheets, and menus.
- **OnSurface**: The default text color for content on a Surface.
- **OnSurfaceVariant**: A lower-contrast text color for secondary content on a Surface.
- **Background**: The underlying background color of the application window.
- **Error**: Used for destructive actions and error states.
- **Focus**: The color used for keyboard focus rings.

### Schemes
Colors change based on the active `Scheme`:
- `Light`: Standard light mode.
- `Dark`: Standard dark mode, utilizing deeper surfaces to reduce eye strain.
- `HighContrast`: A specialized scheme that forces maximum contrast, often using stark black and white with vibrant primary accents, for visually impaired users.

### WCAG Contrast Code
The framework automatically enforces WCAG (Web Content Accessibility Guidelines) contrast requirements. You can programmatically check contrast:

```rust
// Example of checking contrast ratio programmatically
let background = env.color_tokens().surface;
let text_color = env.color_tokens().on_surface;

let ratio = background.contrast_ratio(&text_color);
if ratio < 4.5 {
    log::warn!("Contrast ratio {} fails WCAG AA standards!", ratio);
}
```

## 3. Typography

The `Typography` system defines a rigid scale of text styles, ensuring readability and hierarchy.

### Sizes and Weights
- **Display**: For massive, expressive text (rarely used).
- **Headline**: For page titles.
- **Title**: For section headings.
- **Body**: The standard text style (regular weight).
- **BodySmall**: For secondary, smaller text.
- **Label**: For buttons, tooltips, and badges (usually medium/bold weight).

### Script-Aware Line Heights
Vyom-Taim's typography engine is script-aware. Different languages and writing systems require different line heights to prevent clipping of ascenders/descenders (e.g., Thai, Arabic, or Devanagari).

| Script Category | Base Line Height Multiplier | Adjustment |
|-----------------|-----------------------------|------------|
| Latin/Cyrillic  | 1.5x                        | Standard   |
| Arabic          | 1.7x                        | +0.2x      |
| Thai            | 1.8x                        | +0.3x      |
| Devanagari      | 1.6x                        | +0.1x      |

## 4. Spacing Scale

Consistent spacing is vital for a clean layout. The `Spacing` token defines a scale based on a 4px/8px grid.

- `Spacing::XS` (4px): For tight coupling (e.g., icon next to text).
- `Spacing::SM` (8px): For small gaps (e.g., items in a list).
- `Spacing::MD` (12px): Standard inner padding for small components.
- `Spacing::LG` (16px): Standard padding and margin (e.g., card padding).
- `Spacing::XL` (24px): Section separation.
- `Spacing::XXL` (32px): Major layout blocks.

*Usage Guideline*: Never use arbitrary pixel values for padding or margins. Always use the spacing scale.

## 5. Radius Scale

Corner radiuses help define the personality of the UI. The `Radius` token provides a consistent set of curvatures.

- `Radius::None` (0px): Sharp corners, used for full-bleed images or brutalist design.
- `Radius::Small` (6px): Subtle rounding for small elements like checkboxes or tooltips.
- `Radius::Medium` (12px): Standard rounding for cards, dialogs, and large buttons.
- `Radius::Large` (20px): Pronounced rounding for bottom sheets or prominent hero cards.
- `Radius::Full` (9999px): Pill-shaped, used for badges or standard buttons in specific themes.

## 6. Elevation Deep Dive

Elevation conveys depth and hierarchy along the Z-axis using shadows.

### Elevation Levels
- `Flat`: On the background, no shadow.
- `Raised`: Slightly elevated, subtle shadow (e.g., cards).
- `Floating`: Higher elevation, distinct shadow (e.g., FABs, menus).
- `Modal`: Highest elevation, strong shadow + backdrop dimming (e.g., dialogs).

### ElevationRecipe and GPU Tier Adaptation
Vyom-Taim uses an `ElevationRecipe` to define shadows. Crucially, the framework adapts to the hardware's GPU capabilities.
- **High-End GPU**: Uses soft, physically-based blur shadows with ambient occlusion.
- **Low-End GPU**: Falls back to simple drop shadows or even solid borders (in extreme cases) to maintain 60/120fps performance.

## 7. Motion Tokens

Motion adds life to the UI. `MotionTokens` define standard durations and easing curves.

- `MotionTokens::Feedback` (120ms): Fast, snappy animations for micro-interactions (e.g., button press).
- `MotionTokens::Hierarchy` (240ms): Standard duration for structural changes (e.g., opening a menu or sheet).
- `MotionTokens::Gesture`: Uses a `SpringConfig` for fluid, interruptible, physics-based gesture responses.

`TransitionKind` defines the *type* of motion:
- Fade
- Slide
- Scale
- SharedAxis

## 8. Component Contracts

Vyom-Taim includes over 18 core component contracts that adhere strictly to the design system. These include:

1. **Button**: Primary, Secondary, Text, Outlined variants.
2. **Card**: Surface container with elevation.
3. **Checkbox**: Boolean selection.
4. **Radio**: Single selection from a group.
5. **Switch**: Immediate boolean toggle.
6. **Slider**: Range selection.
7. **TextInput**: Single/multi-line text entry.
8. **Dialog**: Modal interruption.
9. **Menu**: Contextual actions.
10. **Sheet**: Bottom-anchored panel.
11. **Tooltip**: Hover/focus information.
12. **Badge**: Numeric or status indicator.
13. **ProgressIndicator**: Linear or circular loading state.
14. **Divider**: Visual separator.
15. **Avatar**: User representation.
16. **ListTile**: Standardized row for lists.
17. **Snackbar**: Transient notification.
18. **Tabs**: Segmented navigation.

## 9. Viewport, Density, and Touch Targets

### ViewportClass
The UI adapts based on the `ViewportClass`:
- `Compact`: Phones.
- `Medium`: Tablets.
- `Expanded`: Desktops/Laptops.

### Density
`Density` defines the compactness of the UI:
- `Standard`: Default padding.
- `Comfortable`: Extra whitespace.
- `Compact`: Reduced whitespace for data-heavy applications.

### TouchTargetPolicy
The design system enforces a minimum touch target size (usually 44x44 or 48x48 pixels) to ensure usability on touch devices. If a visual element is smaller, the framework automatically expands its invisible hit area to comply with the `TouchTargetPolicy`.

## 10. Complete Example: Building a Card Component

Here is how you use the design tokens to build a custom component that stays perfectly in sync with the design system.

```rust
use taim::prelude::*;

pub fn custom_profile_card(name: &str, role: &str) -> impl Widget {
    // We access tokens implicitly through framework modifiers,
    // ensuring the component responds to theme changes automatically.

    Container::new()
        .background(ColorRole::Surface)
        .padding(Spacing::LG)
        .corner_radius(Radius::Medium)
        .elevation(Elevation::Raised)
        .content(
            Row::new()
                .spacing(Spacing::MD)
                .push(
                    // Avatar placeholder
                    Container::new()
                        .size(48.0, 48.0)
                        .corner_radius(Radius::Full)
                        .background(ColorRole::Primary)
                )
                .push(
                    Column::new()
                        .push(
                            Text::new(name)
                                .style(TypographyRole::Title)
                                .color(ColorRole::OnSurface)
                        )
                        .push(
                            Text::new(role)
                                .style(TypographyRole::BodySmall)
                                .color(ColorRole::OnSurfaceVariant)
                        )
                )
        )
}
```
This comprehensive guide provides everything you need to understand and effectively utilize the Vyom-Taim Design System.
