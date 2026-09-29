# 0024: the public repo, CI, and security

Status: accepted, 2026-09-29.

## Context

Phase 14 opens deskpress to the public. It needs a remote repository on GitHub, continuous
integration to protect the quality gates, security audits, and standard community files.
Release automation is kept out of CI for now, remaining local until store builds and public
distribution are scheduled.

## Decision

- **Remote:** The repository remote is `git@github.com:sirgrimorum/deskpress.git`.
- **CI (`.github/workflows/ci.yml`):** Runs on push to `main` and pull requests.
  - `gate`: verifies Rust code formatting (`cargo fmt`), strict linting (`cargo clippy -D warnings`),
    and runs the test suite under a 100% coverage gate (`cargo llvm-cov`).
  - `android`: sets up Adoptium JDK 25 and Android SDK/NDK 30, builds native bindings, runs
    host JVM unit tests, verifies the Kover coverage floor, and builds the debug APK.
- **Security & Dependabot:**
  - `.github/workflows/security.yml` runs `cargo audit` on push, pull requests, and weekly.
  - `.github/dependabot.yml` scans Cargo, Gradle, and GitHub Actions dependencies weekly.
- **Community standards:**
  - `CONTRIBUTING.md` documents architectural principles, the Makefile workflow, and quality gates.
  - `CODE_OF_CONDUCT.md` provides community standards.
  - `SECURITY.md` establishes a private vulnerability disclosure channel.
  - Issue and pull request templates enforce reproductions and gate verification.
- **Releases out of CI:** Automated release creation and binary packaging remain out of CI for now,
  keeping CI focused on build verification and regression prevention.

## Consequences

- Every pull request and push to `main` is gated automatically by both the 100% Rust coverage floor
  and the Android host tests.
- Contributors have explicit templates and instructions matching repo house rules.
