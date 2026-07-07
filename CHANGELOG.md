<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>pretty-lang</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [1.0.0] - 2026-07-07

API freeze. The public surface delivered in 0.2.0 is now stable and frozen under Semantic Versioning; it will not change in a breaking way within the `1.x` series. No functional changes from 0.2.0 — this release records the promise.

### Changed

- Marked the public API stable and frozen. `docs/API.md` carries the SemVer promise, per-item; the crate-level docs and README record the same.

---

## [0.2.0] - 2026-07-07

The core release. pretty-lang becomes a working, language-agnostic pretty-printer: a `Doc` layout algebra and a linear-time renderer that reflows any syntax tree to a target width.

### Added

- `Doc` — the reference-counted, cheaply-clonable layout document.
- Constructors: `Doc::nil`, `Doc::text`, `Doc::concat`, `Doc::join`.
- Flexible line breaks: `Doc::line`, `Doc::softline`, `Doc::hardline`.
- Combinators: `Doc::append`, `Doc::nest`, `Doc::group`.
- Rendering: `Doc::render` (to `String`), `Doc::render_into` (any `core::fmt::Write`), and `Doc::render_writer` (any `std::io::Write`, behind the `std` feature).
- Trait impls on `Doc`: `Clone`, `Default`, `Debug` (structural, iterative), `From<&'static str>`, `From<String>`, and an iterative `Drop` that dismantles deep documents without overflowing the stack.
- Wadler/Lindig layout engine: single linear render pass with width-bounded look-ahead, both driven by heap work stacks so deep documents neither render nor drop recursively.
- Property tests (`tests/proptests.rs`): flat-layout oracle, panic-safety at every width, `append` associativity, and `group` idempotence.
- Criterion benchmarks (`benches/bench.rs`): JSON-tree and function-call workloads, flat and broken.
- Runnable examples: `quick_start`, `json`, `rust_signature`.
- `docs/API.md` — full public-API reference with per-item examples.

### Changed

- Removed the unused `serde` feature and dependency from the scaffold: a layout document has no serialization use case. The only feature is now `std` (default), which gates the `io::Write` renderer.
- Fixed the scaffold `Cargo.toml` keyword/category arrays (were unquoted, so the manifest did not parse) and aligned `clippy.toml` `msrv` to the declared 1.85.

---

## [0.1.0] - 2026-06-18

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.85.
- Dual `Apache-2.0 OR MIT` license files.
- `README.md`, `CHANGELOG.md`, and a documentation skeleton.
- `REPS.md` compliance baseline.
- `.github/workflows/ci.yml` CI matrix; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `dev/DIRECTIVES.md` and `dev/ROADMAP.md` (committed engineering standards + plan).

[Unreleased]: https://github.com/jamesgober/pretty-lang/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/pretty-lang/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/jamesgober/pretty-lang/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/pretty-lang/releases/tag/v0.1.0
