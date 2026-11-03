# TAIM security policy

Current project version 0.15.0-rc.1 is a release candidate. No production
security support window, patch SLA or supported stable release line
has been announced.

## Private vulnerability reports

Do not publish exploit reproduction details, proof-of-concept payloads,
secrets or sensitive traces in public GitHub issues. Reach the maintainer
via the workspace metadata contact dev@theaimartco.com if deliverable,
or use GitHub private vulnerability reporting if enabled. If no private
route is available, request a private reporting channel without disclosing
the exploit publicly.

Include affected revision and configuration, impact, reproduction,
sanitized traces and suggested mitigations. Maintainers should privately
acknowledge and triage, prepare a regression test, patch, verify affected
devices and coordinate disclosure. Timelines are not guaranteed until
a public support policy is adopted.

## Security review scope

- Secure Wayland surface/privileged compositor lifecycle and trust boundary
- GPU/device-loss recovery, shader validation and bounded allocations
- IME/password/clipboard, screen-reader privacy and screenshot redaction
- Malicious image/font/input/event handling and dependency supply chain
- Iced source lineage, license attribution and reproducible release provenance

Unit tests cannot certify that the complete system is secure.
