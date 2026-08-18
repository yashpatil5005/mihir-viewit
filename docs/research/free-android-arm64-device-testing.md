# Free Android ARM64 Device Testing

Research date: 2026-08-17

## Conclusion

There is no indefinitely free public service that guarantees real ARM64 devices for every Android release from 7 through 15. The strongest zero-payment approach for ViewIt is:

1. Use Samsung Remote Test Lab (RTL) for recurring free access to real ARM64 Galaxy devices and run the existing ADB-driven conformance script.
2. Use Firebase Test Lab on the no-cost Spark plan only if we later create an Android instrumentation version of the conformance suite.
3. Use borrowed or volunteer-owned older phones for Android 7-era coverage if RTL's changing catalog does not contain them.
4. Use ARM64 emulation only as supplementary compatibility evidence, never as proof of native-device performance, memory use, cancellation containment, or vendor behavior.

This plan does not require a payment card, billing account, paid subscription, or expiring trial.

## Best Fit: Samsung Remote Test Lab

Samsung describes RTL signup as free and provides real Galaxy devices. Reservations consume virtual credits rather than money. Samsung documents that an account can claim 10 credits once per day, making this a recurring free allowance rather than a one-time trial.

Most importantly for ViewIt, Samsung's Remote Debug Bridge (RDB) connects a reserved device to the developer's computer as an ADB device. Samsung states that command-line ADB works as if the physical device were locally connected. Linux is explicitly supported by the RDB client.

This closely matches `scripts/android-media-worker-conformance.py`, which needs APK installation, file push, shell commands, process and memory inspection, MediaStore access, and ADB port forwarding. One capability remains to be proven before spending a full reservation: whether RDB permits forwarding the WebView DevTools `localabstract` socket used by the runner.

Limitations:

- The catalog and availability change; Samsung does not promise Android 7-15 coverage.
- Devices are Samsung-only, so this is platform/API coverage rather than cross-OEM coverage.
- Reservations and daily credits limit throughput.
- The current public workflow is interactive rather than CI-oriented.

Sources:

- [Samsung Remote Test Lab: free signup and real devices](https://developer.samsung.com/remote-test-lab)
- [Samsung RDB: Linux setup, real-device ADB connection, and command-line ADB](https://developer.samsung.com/remote-test-lab/blog/en-us/2022/07/13/connect-to-devices-on-remote-test-lab-using-rdb-in-android-studio)
- [Samsung RTL credits: 10 credits claimable once per day](https://developer.samsung.com/sdp/blog/en-us/2023/11/16/testing-watch-faces-in-remote-test-lab-through-watch-face-studio)

## Firebase Test Lab Spark

Firebase's Spark plan is genuinely no-cost and requires no payment method. Its recurring quota includes 5 physical-device test runs and 10 virtual-device test runs per project per day. Physical instrumentation tests can run for up to 45 minutes.

The important constraint is execution model. Test Lab accepts an app APK plus an instrumentation-test APK; it does not expose an arbitrary interactive host-side ADB session. ViewIt's current Python runner therefore cannot run unchanged. We would need to move fixture setup, plugin installation, bridge invocation, MediaStore checks, process recovery assertions, and result reporting into an Android instrumentation test.

This could become the best recurring automated free option, but the rewrite is meaningful and the live device catalog does not guarantee old Android releases.

Sources:

- [Firebase pricing: Spark needs no payment method](https://firebase.google.com/pricing)
- [Test Lab quotas: 5 physical and 10 virtual runs per day on Spark](https://firebase.google.com/docs/test-lab/usage-quotas-pricing)
- [Instrumentation tests: app/test APK model and physical-device timeout](https://firebase.google.com/docs/test-lab/android/instrumentation-test)
- [Available-device catalog and CLI discovery](https://firebase.google.com/docs/test-lab/android/available-testing-devices)

## Android Device Streaming

Firebase Spark includes 30 no-cost device-streaming minutes per project per month and requires no payment method. It provides real devices through Android Studio, but 30 minutes is too small for dependable repeated full-matrix conformance. The public documentation also does not guarantee unrestricted external ADB forwarding or a historical Android 7-15 catalog.

Use it for a focused smoke test when an appropriate model is available, not as the primary matrix.

Sources:

- [Android Device Streaming quota](https://firebase.google.com/docs/test-lab/usage-quotas-pricing#device-streaming)
- [Android Device Streaming guide](https://firebase.google.com/docs/test-lab/android/android-device-streaming)

## Free Local Emulation

ARM64 Android system images exist for many API levels, but an ARM64 guest on this x86_64 workstation cannot use same-architecture KVM acceleration. It requires full instruction emulation and is substantially slower. Some x86 emulator images can translate ARM application code, but that validates the translator path, not native ARM64 hardware.

For this FFmpeg worker, emulation is useful for API compatibility, packaging, class loading, JNI registration, and deterministic conversion correctness. It is not credible for the 96 MiB PSS gate, latency, process isolation under vendor Android, FFmpeg assembly behavior on real ARM CPUs, or production release confidence.

Sources:

- [Google: full ARM emulation versus ARM translation](https://android-developers.googleblog.com/2020/03/run-arm-apps-on-android-emulator.html)
- [Android Emulator acceleration requirements](https://developer.android.com/studio/run/emulator-acceleration)
- [QEMU ARM system emulation](https://qemu-project.gitlab.io/qemu/system/target-arm.html)

## Options Excluded From "Absolutely Free"

- AWS Device Farm offers one-time trial minutes and requires an AWS account/payment setup; it is not an indefinite free tier.
- BrowserStack and Sauce Labs normally offer trials or paid plans. Their open-source programs require application and approval and do not guarantee unrestricted public-device ADB.
- Kobiton and similar commercial device clouds offer trials, not recurring zero-payment access.
- Firebase Blaze includes no-cost quotas but is billing-enabled; Spark is the appropriate plan when zero charge exposure is mandatory.

These can be reconsidered only if ViewIt is accepted into a clearly documented open-source sponsorship with no billing obligation.

## Proposed Zero-Cost Release Matrix

The ADR currently says "representative arm64 Android 7-15 coverage," not one physical device for every Android version. A defensible risk-based matrix is:

| Platform era | Target | Why |
| --- | --- | --- |
| Android 7-9 | Oldest available real ARM64 device, ideally API 24 | `minSdk`, old linker/ART/WebView, background-service behavior |
| Android 10-12 | One real ARM64 device, ideally API 29 or 31 | Scoped storage and middle-generation process behavior |
| Android 13-15 | One real ARM64 device, ideally API 33 or 35 | Granular media permissions and current target behavior |
| Android 16 | Existing Samsung API 36 device | Current platform, 16 KB and worker conformance already passed |

For each remote device, first run a short compatibility probe:

1. Confirm `arm64-v8a`, API level, model, build fingerprint, and WebView version.
2. Confirm APK installation and launch.
3. Confirm `adb forward` can reach the app WebView DevTools socket.
4. Run one short WMV conversion and inspect output streams.
5. Only then spend reservation time on the complete conformance suite.

If RTL supplies the three missing platform-era targets, the existing release gate can be completed for $0. If it does not supply Android 7-9, that portion remains honestly blocked until an older physical device can be borrowed or a volunteer can run the same artifact and evidence collector.
