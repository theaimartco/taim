# Historical pre-migration snapshot

This inventory records the repository before the current TAIM ownership pass.
It is intentionally retained as evidence, not as a statement of current state;
see `dependenciestaim.md` and the live independence audit for current numbers.

Against the recorded iced fork commit 56b706fc, only one entire top-level folder is literally identical:
- .cargo/
These nested folders are also byte-for-byte identical:
- core/src/theme/
- docs/graphs/
- futures/src/backend/
- futures/src/subscription/
- src/window/
- wgpu/src/image/atlas/
- wgpu/src/shader/color/
- wgpu/src/shader/triangle/
- widget/assets/
- widget/src/canvas/
- widget/src/image/
- widget/src/shader/
- widget/src/text/
- winit/src/window/


Using a strict threshold of roughly 80% unchanged Rust source compared with the iced fork point:
Folder	Approx. unchanged	Changes
program/	99.1%	+6 / −6 lines
futures/	97.8%	+30 / −24
winit/	94.0%	+260 / −66
src/	93.1%	+120 / −146
benches/	93.4%	+25 / −25
renderer/	83.1%	+138 / −21
tiny_skia/	81.0%	+715 / −72


Special case:
- debug/: almost every line that remains is iced code, but approximately 43% of the original folder was removed. Therefore, it is a reduced iced folder rather than an almost identical copy.
The large engine folders preserve nearly all their original iced code but have expanded substantially:
Folder	Iced share of current folder
core/	64.3%
wgpu/	58.0%
graphics/	57.5%
widget/	57.3%
runtime/	27.0%
