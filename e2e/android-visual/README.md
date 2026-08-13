# Android Visual Regression

Physical-device visual regression coverage for ViewIt's Office viewers.

The runner captures the WebView through Chrome DevTools Protocol, excluding
volatile Android status/navigation chrome. Baselines are grouped by device and
render profile because WebView output depends on resolution, density, font
scale, Android/WebView version, and device model.

## Run

```bash
npm run test:visual:android
```

This command clears ViewIt's app data, pushes and installs the canonical local
Office plugin ZIP, and overwrites that ZIP in `/sdcard/Download`. Use
`--no-reset` only when intentionally testing the device's existing app/plugin
state. Direct script invocation requires `--confirm-device-reset`.

Use `ADB_SERIAL` when multiple devices are connected. The default fixtures are:

- `/sdcard/Download/report.docx`
- `/sdcard/Download/deck.pptx`
- `/sdcard/Download/deck.odp`
- `/sdcard/Download/sample.xlsx`

Candidates, stability captures, diffs, and `report.json` are written to
`build/android-visual/`.

## Update Baselines

Only update after reviewing the candidate output and confirming DOM readiness
and capture stability:

```bash
npm run test:visual:android:update
```

The runner captures every case twice and refuses unstable or blank images.
Baseline updates are explicit; a normal run never modifies tracked baselines.
Partial baseline updates with `--only` are rejected so a profile cannot become
an incomplete mixed set.
