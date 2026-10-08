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

## [1.0.1] - 2026-10-08

A patch for two engine bugs. No public API change, and every layout an existing document produces at a width up to `isize::MAX` is unchanged.

### Fixed

- **`render(usize::MAX)` broke every group** (ISSUES M64). The target width was converted with `width as isize`, so `usize::MAX` became `-1` (and every width above `isize::MAX` became negative), nothing fit, and every group took its broken form. The width is now clamped to `isize::MAX`; any width at or above it means "unlimited", so `usize::MAX` gives the widest layout.
- **Rendering was quadratic on zero-width content** (ISSUES M65, patch-level part). The "does this group fit?" look-ahead scanned forward until the width ran out or a line break was reached; zero-width content (`nil`, empty `text`, flat `softline`s, nested groups) never uses up the width, so each group could rescan the rest of the document. 100,000 `softline` groups took about 50 s to render. The fit test is now O(1) from width summaries that each node computes when it is built, and rendering is linear in the size of the document at every width. The decisions are identical to the old scan, checked against a line-for-line port of the 1.0.0 renderer on random documents. The per-group `Vec` the old scan allocated is gone with it.
- **The flat benchmarks measured the broken layout.** `benches/bench.rs` used `usize::MAX` as its "fits on one line" width, so because of M64 its `render_flat` numbers (and the README table built from them) timed the fully broken layout. The flat benches now assert that their output really is one line.

### Added

- `tests/equivalence.rs`: 1.0.1 against a port of the 1.0.0 renderer, on random documents (weighted towards zero-width content, with shared subtrees and multi-byte text) at random and extreme widths.
- `tests/scale.rs`: 1,000,000-item zero-width documents render in linear time.
- Properties for the extreme widths 0, 1, `isize::MAX`, `isize::MAX + 1`, and `usize::MAX`; unit tests for the width rule and the M64 regression.
- A `pathological` Criterion group (10k / 100k / 1M items).

### Changed

- Documented exactly how widths are counted: `chars().count()` (Unicode scalar values), not bytes and not terminal display cells, for `text`, the target `width`, and indentation. CJK, emoji, combining marks, and tabs each count as one. Display-width measurement, `Send` documents, and new combinators (`align`, `if_break`, `fill`) are recorded in `dev/ROADMAP.md` for 1.1 / 2.0.
- Internal: each `Doc` node carries its width summary, so a node grows from 32 to 40 bytes (48 to 56 bytes per `Rc` allocation on 64-bit targets). No public type changed.

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

[Unreleased]: https://github.com/jamesgober/pretty-lang/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/jamesgober/pretty-lang/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/jamesgober/pretty-lang/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/jamesgober/pretty-lang/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/pretty-lang/releases/tag/v0.1.0
