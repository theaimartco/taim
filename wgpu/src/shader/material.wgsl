// Physically-grounded surface shading.
//
// A line-by-line mirror of `aura_core::material`. The two implementations
// must agree: one decides what a test asserts and the other decides what a
// person looks at, and a divergence between them is invisible until someone
// photographs a screen. `tests/gpu/material_shading.rs` pins them together.
//
// Nothing here is a filter. Every function takes a physical parameter --
// microfacet width, index of refraction, slab thickness -- and returns a
// quantity a lighting pass can use. That is the difference between a surface
// that responds to a light and a blurred rectangle that does not.

const IOR_AIR: f32 = 1.0;
const MIN_ROUGHNESS: f32 = 0.03;
const PI: f32 = 3.14159265358979323846;

// Clamps a value, replacing NaN with `fallback`.
//
// `clamp` does not guard against NaN -- WGSL leaves `min`/`max` with a NaN
// operand implementation-defined, so a NaN can pass straight through on some
// drivers and not others. `value == value` is false only for NaN.
fn clamp_finite(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    let safe = select(fallback, value, value == value);

    return clamp(safe, low, high);
}

// A surface's physical description.
struct Material {
    // Microfacet roughness, clamped to MIN_ROUGHNESS..1.
    roughness: f32,
    // Physical thickness in millimetres.
    thickness_mm: f32,
    // Index of refraction.
    ior: f32,
    // Difference in index of refraction across the spectrum.
    dispersion: f32,
    // How much light the surface transmits, 0..1.
    transmission: f32,
}

// Brings an authored material into the range the model is defined on.
//
// Authored values arrive out of range, and an index of refraction below one
// inverts refraction while a negative roughness gives a negative microfacet
// density. Both present as a black or exploded surface rather than an error.
fn sanitise(material: Material) -> Material {
    var clean: Material;

    clean.roughness = clamp_finite(material.roughness, MIN_ROUGHNESS, 1.0, 1.0);
    clean.thickness_mm = clamp_finite(material.thickness_mm, 0.0, 1000.0, 0.0);
    clean.ior = clamp_finite(material.ior, 1.0, 4.0, IOR_AIR);
    clean.dispersion = clamp_finite(material.dispersion, 0.0, 0.5, 0.0);
    clean.transmission = clamp_finite(material.transmission, 0.0, 1.0, 0.0);

    return clean;
}

// Reflectance at normal incidence, derived from the index of refraction so a
// surface cannot bend light one way and reflect it as though it bent another.
fn normal_reflectance(material: Material) -> f32 {
    let ior = sanitise(material).ior;
    let ratio = (ior - IOR_AIR) / (ior + IOR_AIR);

    return ratio * ratio;
}

// Schlick's approximation to the Fresnel reflectance.
//
// Rises to 1.0 at grazing incidence on every material, which is why a glass
// edge is bright whatever its tint -- and the single thing a blur-and-tint
// approximation cannot reproduce.
fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    let cosine = clamp_finite(cos_theta, 0.0, 1.0, 1.0);
    let reflectance = clamp_finite(f0, 0.0, 1.0, 0.0);

    return reflectance + (1.0 - reflectance) * pow(1.0 - cosine, 5.0);
}

// The Trowbridge-Reitz (GGX) normal distribution function.
fn ggx_distribution(cos_normal_half: f32, roughness: f32) -> f32 {
    let cosine = clamp_finite(cos_normal_half, 0.0, 1.0, 1.0);
    let alpha = clamp_finite(roughness, MIN_ROUGHNESS, 1.0, 1.0);
    let alpha2 = alpha * alpha;

    let denominator = cosine * cosine * (alpha2 - 1.0) + 1.0;

    return alpha2 / max(PI * denominator * denominator, 1.1754944e-38);
}

// Lateral displacement of what is seen through a slab, in millimetres.
//
// Zero looking straight through, which is exactly why a screen-space blur
// passes for glass in a screenshot and stops passing the moment the viewer or
// the content moves.
fn refraction_offset_mm(cos_theta: f32, material: Material) -> f32 {
    let clean = sanitise(material);
    let cosine = clamp_finite(cos_theta, 0.0, 1.0, 1.0);

    if clean.thickness_mm <= 0.0 || clean.ior <= IOR_AIR {
        return 0.0;
    }

    let sin_incident = sqrt(max(1.0 - cosine * cosine, 0.0));
    let sin_refracted = clamp(sin_incident / clean.ior, 0.0, 1.0);

    let tan_incident = sin_incident / max(cosine, 1.1754944e-38);
    let tan_refracted =
        sin_refracted / sqrt(max(1.0 - sin_refracted * sin_refracted, 1.1754944e-38));

    return max(clean.thickness_mm * (tan_incident - tan_refracted), 0.0);
}

// Per-channel indices of refraction, red through blue.
//
// Red bends least and blue most, which is the direction real glass disperses.
// Reversing it produces edges that look wrong without looking obviously wrong.
fn dispersed_ior(material: Material) -> vec3<f32> {
    let clean = sanitise(material);
    let half_spread = clean.dispersion * 0.5;

    return vec3<f32>(
        clean.ior - half_spread,
        clean.ior,
        clean.ior + half_spread,
    );
}

// The surface normal of a signed distance field, from its gradient.
//
// A rounded rectangle has an exact signed distance, so its bevel is analytic
// rather than sampled: `bevel_mm` controls how far in from the edge the
// surface turns to face the viewer. This is what gives an interface element a
// lit edge without any geometry.
fn sdf_normal(distance: f32, gradient: vec2<f32>, bevel_mm: f32) -> vec3<f32> {
    if bevel_mm <= 0.0 {
        return vec3<f32>(0.0, 0.0, 1.0);
    }

    // Inside the shape, `distance` is negative; the bevel is the band just
    // inside the edge.
    let depth = clamp(-distance / bevel_mm, 0.0, 1.0);

    // Face the viewer in the interior, tilt outwards towards the edge.
    let tilt = 1.0 - depth;
    let normal = vec3<f32>(gradient * tilt, sqrt(max(1.0 - tilt * tilt, 0.0)));

    let length_squared = dot(normal, normal);

    if length_squared <= 1.1754944e-38 {
        return vec3<f32>(0.0, 0.0, 1.0);
    }

    return normal / sqrt(length_squared);
}

// One light, one surface, one specular response.
//
// `normal`, `view` and `light` are unit vectors; `view` and `light` point away
// from the surface.
fn specular(normal: vec3<f32>, view: vec3<f32>, light: vec3<f32>, material: Material) -> f32 {
    let half_vector = normalize(view + light);

    let distribution = ggx_distribution(dot(normal, half_vector), material.roughness);
    let fresnel = fresnel_schlick(max(dot(view, half_vector), 0.0), normal_reflectance(material));

    // Cosine falloff: a surface turned away from the light receives less of it.
    let incidence = max(dot(normal, light), 0.0);

    return distribution * fresnel * incidence;
}

// The specular contribution of a quad's bevelled edge, in linear light.
//
// `distance` and `gradient` come from the rounded-rectangle signed distance
// field the quad is already drawn with, so the bevel is analytic: a lit edge
// costs no geometry, no extra attachment, and no extra SDF evaluation.
//
// `packed` is `(roughness, thickness_mm, ior, dispersion)`; `light` is the
// scene's key light direction, normalised, supplied by the caller so every
// surface in the interface responds to the same one.
fn quad_specular(
    distance: f32,
    gradient: vec2<f32>,
    packed: vec4<f32>,
    light: vec3<f32>,
) -> f32 {
    var material: Material;
    material.roughness = packed.x;
    material.thickness_mm = packed.y;
    material.ior = packed.z;
    material.dispersion = packed.w;
    material.transmission = 0.0;

    // A surface that bends no light has no highlight. This is every widget
    // that has not asked for a material, so the common path costs one compare.
    if material.ior <= IOR_AIR {
        return 0.0;
    }

    // The bevel is a band just inside the edge. Bounded below so a square
    // corner still catches a highlight rather than none.
    let bevel = max(material.thickness_mm, 1.0);
    let normal = sdf_normal(distance, gradient, bevel);

    // A 2D interface is viewed head-on.
    let view = vec3<f32>(0.0, 0.0, 1.0);

    return specular(normal, view, light, material);
}
