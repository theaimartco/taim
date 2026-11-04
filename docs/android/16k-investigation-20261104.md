# 16 KiB GPU buffer failure: investigation, 2026-11-04

Follow-up to [runtime acceptance 2026-10-28](runtime-acceptance-20261028.md). The independent
`android_gpu_probe` (no Taim code) was extended with:

- a Vulkan-only instance (previously `Instance::default()`, i.e. Vulkan + GLES);
- an optional all-backends pass (`adb shell setprop debug.taim.gpu_probe_all 1`);
- a GLES pass;
- a configurable buffer size (`debug.taim.gpu_probe_size`);
- guest page-size logging.

## Results

| Environment | Runs | Vulkan result |
|---|---|---|
| API 35 `google_apis_ps16k` x86_64 emulator, SwiftShader Vulkan, 16 KiB | 22 processes | **All-or-nothing per process**: about 1 in 4 processes pass 16/16; the rest fail 16/16 |
| Same, 256 B / 64 KiB / 4 MiB buffers | 4 each | No size makes it reliable (4 MiB once failed only 1 check) |
| Same, all-backends vs Vulkan-only instance | 4 + 10 | Same failure rate: EGL/GLES initialisation is not the trigger |
| OPPO CPH2637, Mali-G57 MC2, Android 16, 4 KiB (physical) | 5 | 5/5 pass, Vulkan and GLES |
| API 37 `google_apis_playstore_ps16k` emulator | n/a | No Vulkan device with emulator 36.5.11 (needs `VulkanVirtualQueue`); untestable without an emulator update |

Failing processes read back **all zeros**, including `mapped_at_creation` data written by the guest.
That means the guest mapping and the GPU-visible memory are not the same pages. Once a process
fails, every later allocation in it also fails.

## Conclusion

- The failure is below wgpu and below Taim. It is decided per process, probably by guest virtual
  address alignment of host-visible memory in the emulator's gfxstream/SwiftShader Vulkan path.
- No Taim, renderer or allocator change can make it reliable. Buffer size and backend choice have
  no effect. No workaround has been added: hiding wrong GPU memory would also hide real defects.
- The 10-28 conclusion "16 KiB fails" was based on one probe process per guest. With the
  probability above, that single sample was not representative.
- **Still unknown:** behaviour on physical 16 KiB devices. A physical 16 KiB phone run remains a
  release gate, e.g. Pixel 8 or newer with *Developer options → Boot with 16 KB page size*.
  A newer emulator with API 37 Vulkan is the cheaper first check.

## Reproduce

```sh
python3 scripts/android/counter.py build --app gpu-probe --abi x86_64
adb -s emulator-5594 install -r target/android/x86_64/debug/gpu-probe/taim-gpu-probe.apk
# repeat: force-stop, start, read GPU_PROBE_RESULT
adb -s emulator-5594 shell am force-stop com.theaimartco.taim.gpuprobe
adb -s emulator-5594 shell am start -n com.theaimartco.taim.gpuprobe/android.app.NativeActivity
adb -s emulator-5594 logcat -d -s RustStdoutStderr | grep GPU_PROBE_RESULT
```

`scripts/android/buffer-probe.py` reports one process. Run it repeatedly to measure the rate.
