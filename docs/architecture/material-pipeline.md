# The material pipeline

> Status: **live for quads.** A `renderer::Quad` carries a `Material`, both
> quad pipelines shade it, and the result is verified against pixels on real
> hardware. `Material::OPAQUE` is the default and is byte-identical to a build
> with no material support, so nothing changed for any existing widget.
> See §6 for what is still not done.

## 1. Why not blur-and-tint

Every "glass" surface in every mainstream user interface today is one
operation: sample what is behind, blur it, tint it, composite. Apple's Liquid
Glass, Windows Mica and Flutter's `BackdropFilter` are all that.

It is a screen-space approximation with:

- no thickness, so it displaces nothing,
- no refraction, so what is behind does not bend,
- no dispersion, so edges do not separate colour,
- no consistent light source, so two panes disagree about where the light is.

It passes in a screenshot and stops passing the moment something moves behind
it. A real pane bends what it transmits; a blur translates it rigidly.

## 2. The contract

```text
Material { roughness, thickness_mm, ior, dispersion, transmission }
      |
      | sanitise            authored values arrive out of range
      v
physical parameters
      |
      +-- normal_reflectance(ior)          F0, derived not authored
      +-- fresnel_schlick(cos, F0)         -> 1.0 at grazing, always
      +-- ggx_distribution(cos, roughness) microfacet density
      +-- refraction_offset_mm(cos, m)     0.0 head-on, grows with angle
      +-- dispersed_ior(m)                 red bends least, blue most
      v
specular(normal, view, light, material)
```

Two implementations, one contract: `core/src/material.rs` and
`wgpu/src/shader/material.wgsl`. `tests/gpu/material_shading.rs` compiles the
shader from `material.wgsl` *itself* with test entry points appended — there is
one source of truth for the physics, not a copy that can rot — and requires the
two to agree across every material at every viewing angle.

## 3. What the numbers mean

| Quantity | Unit | Note |
|---|---|---|
| `roughness` | microfacet width, `0.03..=1.0` | below `0.03` a highlight aliases rather than sharpens |
| `thickness_mm` | millimetres | a real distance; zero refracts nothing whatever the IOR |
| `ior` | refractive index, `1.0..=4.0` | `1.0` bends nothing; glass is `1.52` |
| `dispersion` | IOR difference across the spectrum | applied symmetrically about `ior` |
| `transmission` | `0.0..=1.0` | `0.0` never triggers a backdrop capture |

**`F0` is derived from `ior`, never authored.** A surface that bent light one
way and reflected as though it bent another would be describable otherwise.
Glass comes out at 4.3% reflectance head-on, which is the textbook air-glass
figure and a real check on the derivation rather than a tuned constant.

## 4. The properties that are pinned

These are asserted, not asserted-about:

- Fresnel equals `F0` head-on and reaches `1.0` at grazing incidence, on every
  material, and rises monotonically between. This is the one thing blur-and-tint
  cannot reproduce, and it is why a glass edge is bright whatever its tint.
- The GGX lobe peaks on the normal, never goes negative, and narrows as a
  surface smooths.
- Refraction displaces **nothing** at normal incidence and grows with viewing
  angle. A surface with no thickness refracts nothing however high its IOR.
- Dispersion separates red-least, blue-most.
- No authored material — NaN, negative, absurd — produces a non-finite result
  in any function, on either implementation.

## 5. NaN is not handled by clamping

`f32::clamp` **propagates NaN**, and WGSL leaves `min`/`max` with a NaN operand
implementation-defined, so a NaN can pass through on one driver and not another.
Both implementations therefore use an explicit finite check
(`clamp_finite` / `value == value`) rather than a clamp.

This was a real defect: a NaN roughness produced a NaN microfacet density, which
becomes a NaN pixel that spreads through every blend it touches.

## 6. What is live, and what is not

### Live

A `renderer::Quad` carries a `Material`. Both quad pipelines — solid and
gradient — take it as an instance attribute, derive an analytic bevel normal
from `rounded_box_gradient`, and add a specular term.

`tests/gpu/material_live.rs` renders real quads through the real renderer and
reads the pixels back. It asserts the things no unit test of the physics can:

- `Material::OPAQUE` is **byte-identical** to the previous behaviour, so every
  existing widget is untouched.
- A material provably changes pixels, which fails if the attribute never
  reaches the shader.
- The highlight is confined to the **edge**; the interior moves by at most
  3/255. A constant normal would brighten the whole card.
- A rougher material peaks **lower** than a smooth one, which fails if the GGX
  lobe is inverted or roughness never arrives.
- No material, including deliberately out-of-range ones, blows out or blacks
  the frame.

The shader's early-out is `ior <= 1.0`, which is `Material::OPAQUE`, so the
common path costs one comparison per fragment.

Materials are neutralised entirely when the renderer is not compositing in
linear light: a specular term added to gamma-encoded components is not a
highlight. `wgpu/src/layer.rs` substitutes `OPAQUE` under `web-colors`.

### Not live

- **Refraction and dispersion are computed but unused.** `refraction_offset_mm`
  and `dispersed_ior` are implemented, tested, and agree across CPU and GPU —
  but displacing what is behind a surface requires sampling a backdrop, and the
  quad pipelines bind no backdrop texture. This is the half that makes glass
  look like glass rather than like a lit edge, and it is **not done**.
  `Material::transmission` is not even sent to the GPU.
- **The light is a constant**, matching `SceneLight::KEY`. `MaterialTokens`
  carries a light the design system owns, and nothing plumbs it through.
- **Only quads.** Text, images, and meshes have no material.
- **No G-buffer.** See below.

## Native view integration — 2026-11-01

The limitations in the earlier checkpoint above are historical: live material
backdrop capture/refraction and configurable scene lighting are now implemented.
Native containers expose `.glass()`, `.frosted_glass()` and an explicit opaque
accessibility fallback. Rough transmissive surfaces now scatter the backdrop
through a bounded nine-tap filter. See [glass surfaces](../design/glass-surfaces.md)
for the current contract, GPU cost and platform limitations.

## 7. The deferred G-buffer

The full deferred G-buffer described in `future.md` — per-pixel material id,
normal, roughness, thickness across multiple attachment formats — needs
Pillar I's compute rasteriser to produce coverage and gradients for arbitrary
paths, not just rounded rectangles. The renderer no longer *blocks* it (see
`color-pipeline.md` §5: pipelines resolve per target format), but it is not
started.
