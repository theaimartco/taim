# `taim_winit`

The TAIM desktop shell: runs a TAIM application on top of [`winit`].

The shell has two halves that talk through a small message protocol:

- the **host** is the `winit` event-loop handler. It creates native windows,
  sets the wait policy and exits, and never touches application state;
- the **engine** owns the application, its windows, renderers and async
  runtime, and runs as one cooperative task the host polls.

Applications start with `run_app` (native TAIM `App`s) or `run` (the
compatibility `Program` path, adapted onto the same engine). The
`conversion` module translates `winit` input and window types into TAIM's
vocabulary for anyone writing their own event loop, and `frame_gate`
exposes the per-window frame-production policy.

<p align="center">
  <img alt="The native target" src="../docs/graphs/native.svg" width="80%">
</p>

[`winit`]: https://github.com/rust-windowing/winit
