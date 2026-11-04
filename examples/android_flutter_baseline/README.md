# Android functional baseline

This source is a counter/text-entry counterpart, not an existing performance result.
Use an installed Flutter SDK to run `flutter create --platforms=android --org com.theaimartco .`, then `flutter pub get` and `flutter build apk --release`. Preserve the supplied lib/main.dart and pubspec.yaml. Record `flutter --version`, engine version, build flags, package/activity and the APK hash with every comparison.

Both apps contain persistent count state, an increment button, Hindi/Arabic/emoji text, a normal field, a password field and multiline notes. Flutter's SingleChildScrollView/Scaffold keyboard handling and Taim's native caret panning have different implementations; this workload compares observable counter/input behavior, not identical rendering pipelines or pixel-identical themes. App-specific pixel coordinates must be selected from settled screenshots.

Use scripts/android/compare.py with release APKs on the same physical device. ADB launch/screenshot intervals are harness observations, not first-photon latency. Neither fixture establishes whole-framework, list/animation, power or accessibility superiority. Expand paired workloads before broad claims.
