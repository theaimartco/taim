# Release Matrix

## Device Coverage
- Desktop (Windows, macOS, Linux)
- Android: ARM64/x86_64 builds; API 35/4 KiB emulator smoke passes; tested 16 KiB GLES smoke passes; Vulkan corruption remains guarded; physical acceptance pending ([evidence](../android/native-host-20261029.md))
- iOS: unsupported; native verification pending

## Language Coverage
- LTR (English, Spanish)
- RTL (Arabic, Hebrew)
- CJK (Chinese, Japanese, Korean)

## Input Coverage
- Mouse + Keyboard
- Touch
- Gamepad

## Platform Coverage
- Wayland / X11
- WGL / EGL
- Metal / Vulkan / DX12

## Recovery Testing
- Context loss recovery
- OOM handling
