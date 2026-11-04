//! Standalone Android Vulkan buffer-integrity probe with no Taim dependency.
//!
//! Tests mapped-at-creation, queue writes and staging-belt writes independently
//! of any window, renderer, view tree or surface. Results are written to logcat.

/// Runs the independent probe on the native activity application thread.
// SAFETY: android-activity requires the uniquely exported Rust-ABI entry point.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(_app: android_activity::AndroidApp) {
    // The default (Vulkan) run keeps its historical log format for buffer-probe.py. A second,
    // independent GLES run isolates the Vulkan driver path from guest memory transport.
    // Diagnostic: an instance with every backend (the historical default) before Vulkan-only.
    // Enable with `adb shell setprop debug.taim.gpu_probe_all 1`.
    if property("debug.taim.gpu_probe_all").as_deref() == Some("1") {
        futures::executor::block_on(run(wgpu::Backends::all(), "_ALL"));
    }
    futures::executor::block_on(run(wgpu::Backends::VULKAN, ""));
    futures::executor::block_on(run(wgpu::Backends::GL, "_GL"));
}

#[cfg(target_os = "android")]
async fn run(backends: wgpu::Backends, tag: &str) {
    use wgpu::util::DeviceExt;
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let Ok(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        else {
            eprintln!("GPU_PROBE{tag}_RESULT unavailable");
            return;
        };
        let page = unsafe { libc_page_size() };
        let size: usize = property("debug.taim.gpu_probe_size")
            .and_then(|v| v.parse().ok())
            .filter(|n: &usize| *n >= 256 && n % 256 == 0 && *n <= 1 << 24)
            .unwrap_or(256);
        eprintln!(
            "GPU_PROBE{tag}_ADAPTER page={page} size={size} {:?}",
            adapter.get_info()
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await
            .unwrap();
        let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &vec![42; size],
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut passed = true;
        for iteration in 0..8 {
            let expected = if iteration == 0 {
                vec![42; size]
            } else {
                (0..size)
                    .map(|i| ((i + iteration * 7) % 256) as u8)
                    .collect::<Vec<_>>()
            };
            if iteration > 0 {
                queue.write_buffer(&source, 0, &expected);
            }
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            encoder.copy_buffer_to_buffer(&source, 0, &output, 0, size as u64);
            let index = queue.submit([encoder.finish()]);
            let slice = output.slice(..);
            slice.map_async(wgpu::MapMode::Read, |result| {
                eprintln!("GPU_PROBE_MAP {result:?}")
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: None,
                })
                .unwrap();
            let actual = slice.get_mapped_range();
            let ok = actual.as_ref() == expected.as_slice();
            passed &= ok;
            eprintln!(
                "GPU_PROBE{tag} iteration={iteration} ok={ok} expected={:?} actual={:?}",
                &expected[..16],
                &actual[..16]
            );
            drop(actual);
            output.unmap();
        }
        let mut belt = wgpu::util::StagingBelt::new(device.clone(), (size as u64).max(100 * 1024));
        for iteration in 0..8 {
            let expected = (0..size)
                .map(|i| ((i + iteration * 13) % 256) as u8)
                .collect::<Vec<_>>();
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            belt.write_buffer(
                &mut encoder,
                &source,
                0,
                std::num::NonZeroU64::new(size as u64).unwrap(),
            )
            .copy_from_slice(&expected);
            encoder.copy_buffer_to_buffer(&source, 0, &output, 0, size as u64);
            belt.finish();
            let index = queue.submit([encoder.finish()]);
            belt.recall();
            let slice = output.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: None,
                })
                .unwrap();
            let actual = slice.get_mapped_range();
            let ok = actual.as_ref() == expected.as_slice();
            passed &= ok;
            eprintln!(
                "GPU_PROBE{tag}_BELT iteration={iteration} ok={ok} expected={:?} actual={:?}",
                &expected[..16],
                &actual[..16]
            );
            drop(actual);
            output.unmap();
        }
        eprintln!("GPU_PROBE{tag}_RESULT passed={passed}");
    }
}

#[cfg(target_os = "android")]
unsafe extern "C" {
    fn getpagesize() -> i32;
    fn __system_property_get(name: *const std::ffi::c_char, value: *mut std::ffi::c_char) -> i32;
}
#[cfg(target_os = "android")]
fn property(name: &str) -> Option<String> {
    let name = std::ffi::CString::new(name).ok()?;
    let mut value = [0 as std::ffi::c_char; 92]; // PROP_VALUE_MAX
    // SAFETY: both pointers are valid; the buffer is PROP_VALUE_MAX bytes as the API requires.
    let len = unsafe { __system_property_get(name.as_ptr(), value.as_mut_ptr()) };
    (len > 0).then(|| {
        unsafe { std::ffi::CStr::from_ptr(value.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}
#[cfg(target_os = "android")]
unsafe fn libc_page_size() -> i32 {
    // SAFETY: getpagesize has no preconditions.
    unsafe { getpagesize() }
}
