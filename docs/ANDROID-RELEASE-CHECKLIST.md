# Android Production Release Checklist

Production readiness is not inferred from `android-release.sh`, an AAB, or a Gradle `release` variant. The standard local script defaults to the `device-test` profile.

## Current Status

Production publication is blocked until the architecture renewal completes transactional updates, canonical contracts, provider lifecycle/health, trust review, and the final production dry run.

## Required Profile And Signing

- [ ] `VIEWIT_APP_PROFILE=production` is used.
- [ ] `VIEWIT_PRODUCTION_SIGNING=1` is used.
- [ ] Production keystore variables are supplied from secure storage.
- [ ] Build fails when production profile uses debug signing.
- [ ] WebView debugging is disabled.
- [ ] Debug intents and local plugin installation are disabled.
- [ ] Only signed HTTPS catalogs are accepted.

## Artifact

- [ ] Full quality gate passes.
- [ ] APK ≤ published size budget.
- [ ] APK 16KB verification passes.
- [ ] APK/AAB signatures verify with the intended upload key.
- [ ] Version code/name are approved.
- [ ] Clean rebuild reproducibility is measured and differences explained.
- [ ] SHA-256 values are recorded.

## Extension Runtime

- [ ] Canonical signed metadata reaches installer unchanged.
- [ ] Failed update preserves last-known-good package.
- [ ] Startup recovery and rollback pass.
- [ ] Contract compatibility and output validation pass.
- [ ] Provider health/quarantine and built-in fallback pass.
- [ ] Trust classes and host grants match implementation.
- [ ] Hosted catalog/artifacts pass exact checksum validation.
- [ ] Key rotation/replay response is documented.

## Privacy And Security

- [ ] Plugin disclosure says trusted in-process where applicable; no false sandbox claim.
- [ ] Network use and custom catalog policy are disclosed.
- [ ] Document handling and temporary storage are documented.
- [ ] Threat model and focused security review are complete.
- [ ] Vulnerability and signing-key compromise response are documented.

## QA

- [ ] Fresh production-profile install on supported physical devices.
- [ ] Core format matrix.
- [ ] Every published provider family install/open/update/remove/restart.
- [ ] Failure, timeout, cancellation, rollback, and low-storage scenarios.
- [ ] Upgrade from the last publicly distributed production version, when one exists.

## Distribution

- [ ] User explicitly approves remote publication.
- [ ] Catalog/plugin publication is completed and health-checked.
- [ ] Release notes include app hash, catalog signature/key identity, known limitations, and rollback instructions.
- [ ] Play Console rollout is explicitly approved.
