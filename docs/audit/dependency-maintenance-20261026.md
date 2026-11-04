# Temporary codec macro maintenance exception — 2026-10-26

## Engineering disposition

Keep EXR/AVIF functionality and the released codec implementations. Accept
one narrowly bounded **maintenance** exception for `paste` 1.0.15 in this
local candidate. This is an engineering risk disposition, not an independent
security review, removal of the dependency, or a claim that all upstream code
is maintained. It supersedes the earlier decision to leave this maintenance
finding as an unconditional failing gate.

RustSec classifies [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html)
as informational/unmaintained, with no patched version. The macro is needed
by the released `pulp` 0.22.3 (EXR) and `rav1e` 0.8.1 (AVIF) dependency chains.
Changing its dependency name requires upstream releases or maintaining codec
forks. The exception keeps the existing codecs rather than silently removing
formats or introducing local forks that downstream package consumers would
not automatically inherit.

`paste` is a build-time procedural macro; it executes during compilation.
That does **not** imply risk-free code. Its registry artifact is pinned by the
lockfile to SHA-256
`57c0d7b74b563b49d38dae00a0c37d4d6de9b432382b2892f0574ddcae73fd0a`.
Known vulnerability advisories, yanked releases, license policy and all other
maintenance findings remain subject to the existing audit policy.

## Mandatory boundaries

The installed cargo-deny supports advisory ID/reason exceptions; expiry and
consumer restrictions are enforced separately by the required local guard:

```sh
cargo deny check licenses advisories
python3 scripts/audit/check-dependency-policy.py
```

The manual verifier runs the guard **after** cargo-deny refreshes the database.
The guard fails on or after **2027-01-26**, on a changed macro version/source
or checksum, on any runtime-library target, on missing dependency evidence,
or on a new/upgraded direct consumer beyond the two exact reviewed registry
releases above. It also reads the local RustSec record and rejects the
exception if the advisory ceases to be maintenance-only or acquires a CVSS
rating. A missing advisory database fails the guard; it is not assumed safe.
Regression tests exercise these rejection paths.

When the upstream codec releases stop depending on `paste`, update their
versions, remove the exception and guard, and rerun codec/visual tests plus
the complete workspace checks. Review earlier if RustSec or upstream reports
a new problem. Do not extend the deadline automatically. Downstream
applications retain their own advisory policies; this exception does not
propagate into a consumer's policy.

With the documented exception, the dependency policy passes. A policy that
requires **zero unmaintained dependencies without exceptions still fails**.
That distinction must remain visible in release notes and evidence.
