# Golden re-capture: linear-light compositing

Record of every golden changed by making linear-light compositing the
production default — removing `web-colors` from the `taim` crate's `default`
feature list, and giving the `tiny_skia` backend the same working space as the
`wgpu` backend.

**67 goldens changed. All 67 are classified below.** Only 34 of them
reported `DIFFERED`; the rest changed *within* their configured tolerance and
would have been adopted silently. They are measured here anyway, because "the
harness did not complain" is not a classification.

## An error these numbers nearly hid

The first re-capture of this suite was wrong, and the classification did not
catch it.

Making the CPU backend composite in linear light means encoding the frame on
the way out. But text colour, decoded image samples, colour-emoji bitmaps and
rasterised SVG all reach `tiny_skia` as 8-bit values that never pass through
`into_color` — so they were being treated as already linear and left the
backend far too bright: sRGB 128 text presented as 188.

That error produces per-channel differences in **the same 0..73 range** as the
legitimate blend-space correction, so no summary statistic distinguished them.
What distinguished them was the *shape* of the change: a body error moves a
large contiguous block by one constant delta, while an edge correction leaves a
thin scatter. In `text_sizes_light` the delta-73 bucket is 0.27% of the image
and 9.8% of changed channels are deltas of 1-8 — a fringe, not a block.

Fixed by `decode_channel` and `decode_premultiplied`, pinned by
`authored_bytes_decode_into_the_working_space`, and this table is measured
against the original committed goldens after that fix.

## The two classes

**Expected correction (64 goldens).** Blending in linear light changes any
pixel that is a *blend*: antialiased glyph and shape edges, translucent
surfaces, shadows, gradient interiors. Flat opaque fills do not move.

For coverage `a` of white over black, gamma-space blending yields `a` in
*encoded* units while linear blending yields `a` in *linear* units then encodes
it. `|srgb_encode(a) - a|` peaks at **73.2/255, at coverage 0.244**.

**The maximum worst-channel across all 67 goldens is exactly 73.** Nothing
moved further than the change can physically cause.

**Precision-only (3 goldens).** `tiny_skia` blends in `f32` but stores 8-bit,
so a linear working space is re-quantised on every draw. sRGB `0..=31` maps onto
only four distinct linear codes, and a storage round trip moves a dark value by
at most **6/255** (measured across all 256 values; mean 0.50). These changed
across a large *area* with a tiny *magnitude* — quantisation, not rendering.

## Every changed golden

| Golden | Worst channel | MAE | RMSE | Channels changed | Class |
|---|---:|---:|---:|---:|---|
| `accordion_light` | 73 | 0.642 | 5.842 | 2.6% | expected correction |
| `app_bar_light` | 73 | 0.380 | 4.645 | 0.8% | expected correction |
| `avatar_and_badge_light` | 73 | 0.785 | 5.980 | 3.8% | expected correction |
| `badge_variants_light` | 73 | 0.552 | 5.416 | 1.6% | expected correction |
| `carousel_light` | 73 | 0.329 | 4.003 | 0.9% | expected correction |
| `checkbox_disabled_light` | 73 | 1.001 | 7.541 | 2.6% | expected correction |
| `checkbox_states_light` | 73 | 0.634 | 5.921 | 1.9% | expected correction |
| `container_surface_light` | 73 | 0.795 | 6.795 | 1.7% | expected correction |
| `date_picker_light` | 73 | 0.965 | 7.280 | 2.4% | expected correction |
| `icon_button_light` | 73 | 0.432 | 4.330 | 2.1% | expected correction |
| `icon_row_light` | 73 | 1.506 | 9.521 | 3.2% | expected correction |
| `icon_sizes_light` | 73 | 0.494 | 5.336 | 1.2% | expected correction |
| `list_tile_light` | 73 | 0.676 | 6.136 | 1.5% | expected correction |
| `pull_to_refresh_light` | 73 | 0.233 | 3.649 | 0.5% | expected correction |
| `radio_states_light` | 73 | 0.685 | 5.987 | 2.1% | expected correction |
| `segmented_control_light` | 73 | 0.887 | 6.716 | 3.4% | expected correction |
| `text_input_value_light` | 73 | 0.955 | 7.423 | 4.7% | expected correction |
| `text_sizes_light` | 73 | 1.380 | 8.915 | 3.0% | expected correction |
| `toggler_disabled_light` | 73 | 1.007 | 7.542 | 3.0% | expected correction |
| `toggler_states_light` | 73 | 0.602 | 5.668 | 2.1% | expected correction |
| `tabs_light` | 72 | 1.302 | 8.346 | 3.8% | expected correction |
| `chips_light` | 63 | 0.578 | 5.027 | 2.6% | expected correction |
| `time_picker_light` | 63 | 0.545 | 5.117 | 1.5% | expected correction |
| `bottom_nav_bar_light` | 62 | 0.736 | 5.890 | 2.3% | expected correction |
| `avatar_and_badge_dark` | 61 | 1.107 | 3.398 | 71.8% | expected correction |
| `search_bar_light` | 59 | 0.200 | 2.756 | 2.1% | expected correction |
| `snackbar_light` | 59 | 2.100 | 6.575 | 19.2% | expected correction |
| `stepper_at_limit_light` | 53 | 0.104 | 1.887 | 0.8% | expected correction |
| `radio_states_dark` | 44 | 1.159 | 3.583 | 74.8% | expected correction |
| `date_picker_dark` | 41 | 1.233 | 3.948 | 73.9% | expected correction |
| `accordion_dark` | 40 | 0.957 | 3.035 | 64.3% | expected correction |
| `app_bar_dark` | 40 | 0.942 | 2.614 | 74.8% | expected correction |
| `badge_variants_dark` | 40 | 1.026 | 3.012 | 74.8% | expected correction |
| `carousel_dark` | 40 | 0.962 | 2.808 | 74.9% | expected correction |
| `checkbox_disabled_dark` | 40 | 1.239 | 3.951 | 74.1% | expected correction |
| `checkbox_states_dark` | 40 | 1.050 | 3.120 | 73.7% | expected correction |
| `container_surface_dark` | 40 | 1.126 | 3.489 | 74.9% | expected correction |
| `icon_button_dark` | 40 | 1.008 | 2.861 | 74.0% | expected correction |
| `icon_row_dark` | 40 | 1.417 | 4.667 | 72.0% | expected correction |
| `icon_sizes_dark` | 40 | 0.939 | 2.697 | 71.5% | expected correction |
| `list_tile_dark` | 40 | 1.085 | 3.339 | 74.9% | expected correction |
| `tabs_dark` | 40 | 1.603 | 5.241 | 73.5% | expected correction |
| `text_input_value_dark` | 40 | 1.187 | 3.541 | 74.3% | expected correction |
| `text_sizes_dark` | 40 | 1.363 | 4.434 | 74.3% | expected correction |
| `time_picker_dark` | 40 | 1.057 | 3.544 | 69.1% | expected correction |
| `toggler_disabled_dark` | 40 | 1.214 | 3.872 | 73.5% | expected correction |
| `toggler_states_dark` | 40 | 1.035 | 3.072 | 72.7% | expected correction |
| `button_disabled_dark` | 39 | 2.899 | 8.042 | 75.0% | expected correction |
| `chips_dark` | 39 | 0.914 | 2.480 | 64.0% | expected correction |
| `pull_to_refresh_dark` | 39 | 0.862 | 2.047 | 75.0% | expected correction |
| `segmented_control_dark` | 39 | 1.052 | 3.363 | 60.1% | expected correction |
| `slider_and_progress_dark` | 39 | 0.492 | 0.981 | 47.1% | expected correction |
| `button_enabled_dark` | 38 | 0.800 | 1.967 | 62.7% | expected correction |
| `bottom_nav_bar_dark` | 34 | 0.893 | 2.936 | 52.3% | expected correction |
| `progress_extremes_dark` | 34 | 0.379 | 0.674 | 37.6% | expected correction |
| `search_bar_dark` | 28 | 0.679 | 1.683 | 54.7% | expected correction |
| `stepper_at_limit_dark` | 27 | 0.704 | 1.557 | 62.9% | expected correction |
| `slider_and_progress_light` | 26 | 0.023 | 0.622 | 0.2% | expected correction |
| `button_disabled_light` | 25 | 1.985 | 6.548 | 9.5% | expected correction |
| `button_enabled_light` | 25 | 0.213 | 2.006 | 1.6% | expected correction |
| `snackbar_dark` | 24 | 0.937 | 1.959 | 62.2% | expected correction |
| `text_input_placeholder_dark` | 16 | 0.980 | 1.858 | 73.9% | expected correction |
| `progress_extremes_light` | 15 | 0.004 | 0.202 | 0.1% | expected correction |
| `text_input_placeholder_light` | 9 | 0.148 | 0.934 | 4.8% | expected correction |
| `skeleton_still_dark` | 4 | 0.659 | 0.816 | 65.6% | precision-only |
| `stepper_at_rest_dark` | 4 | 0.634 | 0.811 | 62.6% | precision-only |
| `skeleton_still_light` | 1 | 0.002 | 0.040 | 0.2% | precision-only |

## Totals

- Changed: **67**
- Expected correction: **64**
- Precision-only: **3**
- Genuine regressions: **0**
- Rejected: **0** (one whole re-capture was discarded and redone — see above)
- Resized: **0**

## What was checked before adopting

1. **Nothing exceeds the analytic bound.** Max worst-channel is 73;
   `|srgb_encode(a) - a|` peaks at 73.2/255.
2. **Large-area changes are small-magnitude**, inside the 6/255 storage bound.
3. **Large-magnitude changes are small-area and scattered**, which is what an
   edge correction looks like and what a body error does not.
4. **No golden changed size**, which would mean a layout regression.
5. **The GPU path is pinned independently** by `tests/gpu/color_output.rs` and
   `tests/color/output.rs`, which assert against analytically derived values
   rather than a previously accepted picture. A golden suite cannot validate
   itself, and these were re-captured only after those were green.
