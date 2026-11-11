# Code signing policy

## Current status

**Alpha Desktop submitted an eligibility application to SignPath Foundation on 11 November 2026. It is not yet approved or sponsored, and all currently published v0.1.0 binaries are unsigned.** An application does not mean a certificate has been granted. We do not promise that SmartScreen warnings will disappear immediately.

## Project provenance

Alpha Desktop is an independently maintained MIT-licensed derivative of [Daozai](https://github.com/Renly1994/xiaotang-daozai), with notification work referencing [Fly Island](https://github.com/flyl1u/fly-island). Original copyright and third-party notices are preserved.

The public repository was created from a reviewed source snapshot on 11 November 2026, rather than a GitHub fork network. It is a new public project and does not yet claim established adoption or reputation. Upstream signing status has not been verified. Eligibility under the Foundation's derivative-project rules must be reviewed before any signing is provisioned.

## Proposed roles and release controls

- Maintainer, author and reviewer: [JackChen999888](https://github.com/JackChen999888).
- Proposed release/signing approver: [JackChen999888](https://github.com/JackChen999888).
- External contributions must be reviewed before merging. Future signing requests require explicit maintainer approval.
- Multi-factor authentication is required for source hosting and SignPath before signed releases can be enabled. This page does not assert that the private account setup has already been verified.
- Only binaries built from this repository's reviewed source and build scripts may be proposed for signing. No account data, credentials or unrelated binaries may be included.

## Builds and future signing

The initial v0.1.0 release was built locally. The [Build Windows packages workflow](../.github/workflows/package.yml) supports building and testing Windows installers from public source on GitHub-hosted runners. Artifacts remain unsigned until a signing provider has reviewed the project and the signing integration is configured.

If approved, the intended process is: review a release commit; build with GitHub Actions; verify provenance and product/version metadata; sign the application and helper; sign the uninstaller; build and sign the outer installer; verify all signatures and timestamps; generate checksums from the final signed artifacts; publish only after maintainer approval. The exact workflow and artifact policy will be finalized with the provider, without bypassing its controls.

For an approved SignPath Foundation integration, the required acknowledgement would be: “Free code signing provided by SignPath.io, certificate by SignPath Foundation.” **This is a proposed future acknowledgement, not a statement that the project currently receives this service.** Foundation certificates identify SignPath Foundation as publisher, rather than the maintainer's personal name.

## Privacy and reporting

See the [privacy policy](PRIVACY.md) for local data, account storage, third-party connections, OCR and translation. Signing applications must not publish the maintainer's private contact information in this repository. See [SECURITY.md](../SECURITY.md) to report security issues privately.

Official program rules: https://signpath.org/terms
Application: https://signpath.org/apply.html
