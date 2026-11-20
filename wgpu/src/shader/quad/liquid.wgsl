// Liquid glass: glass a finger presses into and that melts into its
// neighbours.
//
// Shades exactly like a transmissive solid quad (`solid.wgsl`), with three
// additions:
//
// - The pane's shape is the smooth union of its own rounded rectangle and up
//   to two neighbours', so panes that come close flow into one another
//   through a bridge whose edge, rim and refraction are all continuous. Each
//   point of the merged shape is drawn by exactly one pane -- the nearest --
//   so the overlap is never shaded twice.
// - A press is a second light just above the glass: the rim nearest the
//   finger catches it, and what is under the finger is seen magnified, as
//   through a drop of water.
// - Legibility: over a backdrop that would leave text on the pane unreadable
//   (bright glass over a dark photo, smoked glass over a white page), less of
//   the backdrop is let through, smoothly and only where it is needed.

struct LiquidVertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) color: vec4<f32>,
    @location(1) pos: vec2<f32>,
    @location(2) scale: vec2<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_width: f32,
    @location(6) shadow_color: vec4<f32>,
    @location(7) shadow_offset: vec2<f32>,
    @location(8) shadow_blur_radius: f32,
    @location(9) snap: u32,
    @location(10) material: vec4<f32>,
    @location(11) transmission: f32,
    @location(12) touch: vec4<f32>,
    @location(13) neighbour_a: vec4<f32>,
    @location(14) neighbour_b: vec4<f32>,
    @location(15) shape: vec4<f32>,
}

// Everything but the colours is per pane, so flat: twelve inter-stage
// variables, within the fifteen every adapter TAIM accepts provides.
struct LiquidVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(flat) border_color: vec4<f32>,
    // Position and size, in physical pixels.
    @location(2) @interpolate(flat) rect: vec4<f32>,
    @location(3) @interpolate(flat) border_radius: vec4<f32>,
    // Shadow offset, shadow blur radius, border width.
    @location(4) @interpolate(flat) shadow: vec4<f32>,
    @location(5) @interpolate(flat) shadow_color: vec4<f32>,
    @location(6) @interpolate(flat) material: vec4<f32>,
    @location(7) @interpolate(flat) touch: vec4<f32>,
    @location(8) @interpolate(flat) neighbour_a: vec4<f32>,
    @location(9) @interpolate(flat) neighbour_b: vec4<f32>,
    // Neighbour radii, blend distance, legibility.
    @location(10) @interpolate(flat) shape: vec4<f32>,
    @location(11) @interpolate(flat) transmission: f32,
}

@vertex
fn liquid_vs_main(input: LiquidVertexInput) -> LiquidVertexOutput {
    var out: LiquidVertexOutput;

    let merges = input.neighbour_a.z > 0.0 || input.neighbour_b.z > 0.0;
    // A bridge reaches past the pane by up to the blend distance.
    let reach = select(0.0, input.shape.z, merges);

    // Never snapped: a pane in motion must move smoothly, and its
    // neighbours' rectangles must match its own to the bit.
    let rect = taim_snap_rect(
        (input.pos + min(input.shadow_offset, vec2<f32>(0.0)) - input.shadow_blur_radius - reach)
            * globals.scale,
        (input.scale + abs(input.shadow_offset) + (input.shadow_blur_radius + reach) * 2.0)
            * globals.scale,
        false,
    );

    let border_radius = min(input.border_radius, vec4(min(input.scale.x, input.scale.y) / 2.0));

    out.position = taim_quad_vertex(rect, input.vertex_index);
    out.color = premultiply(input.color);
    out.border_color = premultiply(input.border_color);
    out.rect = vec4<f32>(input.pos, input.scale) * globals.scale;
    out.border_radius = border_radius * globals.scale;
    out.shadow = vec4<f32>(
        input.shadow_offset * globals.scale,
        input.shadow_blur_radius * globals.scale,
        input.border_width * globals.scale,
    );
    out.shadow_color = premultiply(input.shadow_color);
    out.material = input.material;
    out.touch = vec4<f32>(input.touch.xy * globals.scale, input.touch.z, input.touch.w * globals.scale);
    out.neighbour_a = input.neighbour_a * globals.scale;
    out.neighbour_b = input.neighbour_b * globals.scale;
    out.shape = vec4<f32>(input.shape.xyz * globals.scale, input.shape.w);
    out.transmission = input.transmission;

    return out;
}

// No pane: a distance no fragment is ever near.
const LIQUID_FAR: f32 = 1.0e6;

// A rounded rectangle's signed distance and outward gradient at `fragment`,
// as `(distance, gradient)`.
fn liquid_box(fragment: vec2<f32>, rect: vec4<f32>, corners: vec4<f32>) -> vec3<f32> {
    if rect.z <= 0.0 || rect.w <= 0.0 {
        return vec3<f32>(LIQUID_FAR, 0.0, 0.0);
    }
    let p = taim_quad_sdf_point(fragment, rect.xy, rect.zw);
    let distance = rounded_box_sdf(p, rect.zw, corners * 2.0) / 2.0;
    let gradient = -rounded_box_gradient(p, rect.zw, corners * 2.0);
    return vec3<f32>(distance, gradient);
}

// The smooth union of two `(distance, gradient)` fields, `k` wide, with its
// exact gradient: the blend weight's own derivative is what turns the
// bridge's edge towards the viewer, so the rim and refraction follow it.
fn liquid_smooth_union(a: vec3<f32>, b: vec3<f32>, k: f32) -> vec3<f32> {
    if k <= 0.0 {
        return select(b, a, a.x <= b.x);
    }
    let h = clamp(0.5 + 0.5 * (b.x - a.x) / k, 0.0, 1.0);
    let distance = mix(b.x, a.x, h) - k * h * (1.0 - h);
    var weight = vec2<f32>(0.0);
    if h > 0.0 && h < 1.0 {
        weight = (b.yz - a.yz) * (0.5 / k);
    }
    let gradient = mix(b.yz, a.yz, h) + (a.x - b.x) * weight - k * (1.0 - 2.0 * h) * weight;
    return vec3<f32>(distance, gradient);
}

// A total order on panes, so every pane of a group agrees on who draws a
// point two of them are equally near, and blends them in the same order.
fn liquid_before(a: vec4<f32>, b: vec4<f32>) -> bool {
    if a.x != b.x {
        return a.x < b.x;
    }
    if a.y != b.y {
        return a.y < b.y;
    }
    if a.z != b.z {
        return a.z < b.z;
    }
    return a.w < b.w;
}

struct LiquidField {
    // `(distance, gradient)` of the merged shape.
    value: vec3<f32>,
    // Whether this pane draws this point of it.
    owner: bool,
}

fn liquid_field(fragment: vec2<f32>, input: LiquidVertexOutput) -> LiquidField {
    let own = liquid_box(fragment, input.rect, input.border_radius);

    let radius_a = min(input.shape.x, min(input.neighbour_a.z, input.neighbour_a.w) * 0.5);
    let radius_b = min(input.shape.y, min(input.neighbour_b.z, input.neighbour_b.w) * 0.5);
    let a = liquid_box(fragment, input.neighbour_a, vec4<f32>(radius_a));
    let b = liquid_box(fragment, input.neighbour_b, vec4<f32>(radius_b));

    // Nearest draws; a tie goes to the pane that sorts first.
    var owner = true;
    if a.x < own.x || (a.x == own.x && liquid_before(input.neighbour_a, input.rect)) {
        owner = false;
    }
    if b.x < own.x || (b.x == own.x && liquid_before(input.neighbour_b, input.rect)) {
        owner = false;
    }

    // The union is not associative, so every pane blends the group in the
    // same order: sorted by rectangle. A missing neighbour is far away and
    // sorts last, and blending with it changes nothing.
    var f0 = own;
    var r0 = input.rect;
    var f1 = a;
    var r1 = select(vec4<f32>(LIQUID_FAR), input.neighbour_a, a.x < LIQUID_FAR);
    var f2 = b;
    var r2 = select(vec4<f32>(LIQUID_FAR), input.neighbour_b, b.x < LIQUID_FAR);
    if liquid_before(r1, r0) {
        let f = f0; f0 = f1; f1 = f;
        let r = r0; r0 = r1; r1 = r;
    }
    if liquid_before(r2, r1) {
        let f = f1; f1 = f2; f2 = f;
        let r = r1; r1 = r2; r2 = r;
    }
    if liquid_before(r1, r0) {
        let f = f0; f0 = f1; f1 = f;
        let r = r0; r0 = r1; r1 = r;
    }

    let k = input.shape.z;
    let merged = liquid_smooth_union(liquid_smooth_union(f0, f1, k), f2, k);

    return LiquidField(merged, owner);
}

fn liquid_luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// How much of the backdrop to let through so text on the pane stays
// readable. Bright glass carries dark ink and must not fall below about 0.3
// in linear luminance (WCAG AA for black text needs 0.175); smoked glass
// carries white ink and must not rise above about 0.09 (AA needs 0.18 or
// less). `around` is the backdrop seen through heavy frost.
fn liquid_legible_transmission(
    transmission: f32,
    tint: vec3<f32>,
    around: vec3<f32>,
    legibility: f32,
) -> f32 {
    let glass = liquid_luminance(tint);
    let behind = liquid_luminance(around);
    var limit = transmission;

    if glass >= 0.5 {
        let lowest = 0.3;
        if behind < lowest {
            limit = (glass - lowest) / max(glass - behind, 1.0e-4);
        }
    } else {
        let highest = 0.09;
        if behind > highest && glass < highest {
            limit = (highest - glass) / max(behind - glass, 1.0e-4);
        }
    }

    return mix(transmission, clamp(min(transmission, limit), 0.0, 1.0), legibility);
}

@fragment
fn liquid_fs_main(input: LiquidVertexOutput) -> @location(0) vec4<f32> {
    let fragment = input.position.xy;
    let field = liquid_field(fragment, input);
    let dist = field.value.x;
    let gradient = field.value.yz;
    let coverage = taim_quad_coverage(dist);

    // The shadow of the merged shape, where the merged shape is not. Drawn
    // by whichever pane is nearest the shadow's own point.
    var shadow = vec4<f32>(0.0);
    if input.shadow_color.a > 0.0 {
        let caster = liquid_field(fragment - input.shadow.xy, input);
        if caster.owner {
            let blur = input.shadow.z;
            let alpha = 1.0 - smoothstep(-blur, blur, max(caster.value.x, 0.0));
            shadow = input.shadow_color * alpha * (1.0 - coverage);
        }
    }

    if !field.owner || coverage <= 0.0 {
        return shadow;
    }

    var fill = input.color;
    let border_width = input.shadow.w;
    if border_width > 0.0 {
        fill = mix(input.color, input.border_color, clamp(0.5 + dist + border_width, 0.0, 1.0));
    }

    // A press: where it is, and how near this fragment is to it.
    let touch = input.touch;
    let pressing = touch.z > 0.0 && touch.w > 0.0;
    var near_touch = 0.0;
    var look_from = fragment;
    if pressing {
        let away = fragment - touch.xy;
        near_touch = exp(-dot(away, away) / (touch.w * touch.w));
        // What is under the finger is seen magnified: sample nearer to it.
        look_from = fragment - away * (0.2 * touch.z * near_touch);
    }

    if input.transmission > 0.0 && has_backdrop() {
        let tint = fill.rgb / max(fill.a, 1.0e-4);
        var transmission = input.transmission;
        if input.shape.w > 0.0 {
            let around = sample_backdrop_lod(fragment, vec2<f32>(0.0), 5.0);
            transmission = liquid_legible_transmission(transmission, tint, around, input.shape.w);
        }

        let behind = quad_transmission(dist, gradient, input.material, transmission, look_from);
        fill = vec4<f32>(
            blend_transmission(fill.rgb, behind * fill.a, input.material, transmission, dist, gradient),
            fill.a,
        );
    }

    var light = 0.0;
    if input.material.z > 1.0 {
        light = quad_specular(dist, gradient, input.material, globals.light.xyz);

        if pressing {
            // The finger is a second light just above the glass: the rim
            // facing it catches it, fading with distance.
            let material = unpack_material(input.material, 0.0);
            let normal = sdf_normal(dist, gradient, max(material.thickness_mm, 1.0));
            let toward = normalize(vec3<f32>(touch.xy - fragment, touch.w * 0.5));
            let away = fragment - touch.xy;
            let reach = exp(-dot(away, away) / (touch.w * touch.w * 2.5));
            light = light
                + specular(normal, vec3<f32>(0.0, 0.0, 1.0), toward, material) * touch.z * reach * 1.1;
        }
    }

    // A soft glow in the body under the finger, as light scatters in it.
    let glow = 0.16 * touch.z * near_touch;

    // Additive light, premultiplied by coverage; a highlight adds light, not
    // opacity.
    let pane = fill * coverage + vec4<f32>(vec3<f32>((light + glow) * coverage), 0.0);
    return pane + shadow;
}
