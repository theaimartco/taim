# Golden Test Policy

## Approval Process
1. Run `cargo test --test golden -- --bless` locally to generate new golden images.
2. Review diffs manually.
3. Commit both the code and the new golden images.
4. Get PR approval from a code owner.

## Tolerance Classes
- **Exact**: Used for GPU primitives, layout boundaries. No deviations allowed.
- **Approximate**: Used for text rendering. Allows channel diff <= 1 on <= 2% pixels.
- **Effect-Specific**: Used for blurs, shadows, etc. where rendering might differ slightly across GPU drivers.

## Font/Driver/Renderer Pinning
- Tests must run on Ubuntu with specific Mesa driver versions in CI to ensure consistency.
- Fonts must be loaded from local files instead of relying on system fonts.
