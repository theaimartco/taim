# Recovery

## Recovery Ladder
In the event of a fault or GPU loss, the system escalates through these recovery steps:
1. **Stop prefetch**: Halt background resource loading.
2. **Evict caches**: Drop non-essential textures and glyph caches.
3. **Reduce quality**: Disable complex shaders and anti-aliasing.
4. **Rebuild GPU**: Re-initialize the wgpu instance and swapchain.
5. **Restart renderer**: Full teardown and rebuild of the render pipeline.
6. **Safe shell**: Fallback to basic software rendering if GPU fails entirely.

## Crash Loop Detection and Escalation
- Track restarts within a 60-second window.
- If >3 restarts occur, drop to Safe Shell and alert the system supervisor.
