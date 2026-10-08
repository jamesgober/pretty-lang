<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>pretty-lang</b>
    <br>
    <sub><sup>PRETTY PRINTER</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/pretty-lang"><img alt="Crates.io" src="https://img.shields.io/crates/v/pretty-lang"></a>
    <a href="https://crates.io/crates/pretty-lang"><img alt="Downloads" src="https://img.shields.io/crates/d/pretty-lang?color=%230099ff"></a>
    <a href="https://docs.rs/pretty-lang"><img alt="docs.rs" src="https://img.shields.io/docsrs/pretty-lang"></a>
    <a href="https://github.com/jamesgober/pretty-lang/actions"><img alt="CI" src="https://github.com/jamesgober/pretty-lang/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        pretty-lang is the TOOL-tier crate: language-agnostic AST/CST-to-source rendering — a <code>gofmt</code>-style formatter for any language, nearly free. Part of the -lang language-construction family; see _strategy/LANG_COLLECTION.md for the master plan.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: stable.</strong> As of <code>1.0.0</code> the public API is frozen under Semantic Versioning; see <a href="./docs/API.md#stability"><code>docs/API.md</code></a> for the promise and <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a> for the history.
    </blockquote>
</div>

<hr>
<br>

<div align="left">
    <p>
        <strong>pretty-lang</strong> turns a syntax tree into laid-out source text that reflows to a target line width. It is the rendering half of a formatter and knows nothing about grammars: you describe a layout with a handful of combinators and pretty-lang decides where the lines break.
    </p>
    <p>
        You never print strings directly. You build a <a href="./docs/API.md#doc"><code>Doc</code></a> — a lazy description that says <em>these pieces belong together</em>, <em>put a break here that becomes a space or a newline</em>, <em>indent the inside</em>, <em>keep this on one line if it fits</em> — and rendering it against a width chooses concrete line breaks. The same document renders densely at width 100 and stacked at width 20, with no branching in your code.
    </p>
    <p>
        The engine is Wadler's <em>A Prettier Printer</em> in Lindig's imperative form. Rendering is one <code>O(document&nbsp;size)</code> pass at any width: every node carries a width summary computed when it is built, so the "does this group fit?" test is O(1) and never rescans the line. Nothing recurses on the tree, so deeply nested documents cannot overflow the stack. Widths are counted in <code>char</code>s (<code>chars().count()</code>), not bytes and not terminal display cells; see <a href="./docs/API.md#text-width">Widths</a>. The crate is <code>no_std</code> (needs only <code>alloc</code>) and contains no <code>unsafe</code> (<code>#![forbid(unsafe_code)]</code>).
    </p>
</div>

<hr>
<br>

## Performance First

Rendering is a single linear pass over the document with an O(1) fit test at every group — no look-ahead scan, no backtracking, no intermediate materialization, no per-node allocation on the render path. Local Criterion means for 1.0.1 (`cargo bench --bench bench`, Windows x86_64, Rust 1.95 stable, release build):

| Workload | Layout | Time |
|----------|--------|-----:|
| JSON tree, depth 4 × 4 (572 B out) | flat (fits) | ~2.9 µs |
| JSON tree, depth 4 × 4 | broken (reflowed) | ~3.2 µs |
| Call, 128 arguments | flat | ~3.8 µs |
| Call, 128 arguments | broken | ~3.9 µs |
| Call, 8 arguments | flat | ~440 ns |
| 1,000,000 `group(softline)` items | zero-width | ~119 ms |

Numbers vary by CPU and environment; run the suite on your target to establish a baseline. The 1.0.0 README's "flat" rows were measured at `usize::MAX`, which 1.0.0 mis-handled as a negative width, so they timed the broken layout; see the [1.0.1 release notes](./docs/release/v1.0.1.md) for the corrected before/after numbers, including the zero-width shapes that took 1.0.0 about 50 s at 100,000 items. Building a `Doc` is one `Rc` allocation per combinator; rendering reuses a single work stack and writes straight into the output buffer.

<br>
<hr>

## Features

- **Eight combinators, any language** — `text`, `line`, `softline`, `hardline`, `append`, `nest`, `group`, plus `concat` / `join`. Build a `Doc` from any AST and render.
- **Fit-aware reflow** — [`group`](./docs/API.md#group) lays its contents flat when they fit the remaining width and breaks them all-or-nothing when they do not.
- **Linear time, no recursion** — Wadler/Lindig layout with an O(1) fit test (precomputed width summaries), so rendering is linear even on zero-width-heavy documents; deep documents render and drop without overflowing the stack.
- **Stream or collect** — render to a [`String`](./docs/API.md#render), into any [`core::fmt::Write`](./docs/API.md#render_into), or into a [`std::io::Write`](./docs/API.md#render_writer).
- **`no_std`** — needs only `alloc`; the `std` feature adds the I/O renderer.
- **Fully safe** — no `unsafe`, `#![forbid(unsafe_code)]`.
- **Property-tested** — flat-layout, panic-safety, associativity, and idempotence invariants checked across randomized documents with `proptest`.

<br>
<hr>

## Installation

```toml
[dependencies]
pretty-lang = "1"

# no_std (drops the io::Write renderer):
pretty-lang = { version = "1", default-features = false }
```

**MSRV is 1.85+** (Rust 2024 edition).

<hr>
<br>

## Quick Start

```rust
use pretty_lang::Doc;

// Build `f(a, b, c)` as a document that can break one-argument-per-line.
let call = Doc::text("f(")
    .append(
        Doc::softline()
            .append(Doc::join(
                Doc::text(",").append(Doc::line()),
                ["a", "b", "c"].map(Doc::text),
            ))
            .nest(4),
    )
    .append(Doc::softline())
    .append(Doc::text(")"))
    .group();

// Wide: it all fits on one line.
assert_eq!(call.render(80), "f(a, b, c)");

// Narrow: the group breaks and the arguments stack, indented.
assert_eq!(call.render(6), "f(\n    a,\n    b,\n    c\n)");
```

### A JSON pretty-printer, end to end

A single walk over a value tree produces a document that renders as dense one-liners where they fit and indented blocks where they do not — with no width logic in the walk. See [`examples/json.rs`](./examples/json.rs) for the full version.

```rust
use pretty_lang::Doc;

// Wrap a body between brackets so it collapses to `open body close` when it
// fits and becomes a multi-line block otherwise.
fn bracket(open: &'static str, body: Doc, close: &'static str) -> Doc {
    Doc::text(open)
        .append(Doc::softline().append(body).nest(2))
        .append(Doc::softline())
        .append(Doc::text(close))
        .group()
}

let array = bracket(
    "[",
    Doc::join(
        Doc::text(",").append(Doc::line()),
        ["1", "2", "3"].map(Doc::text),
    ),
    "]",
);

assert_eq!(array.render(80), "[1, 2, 3]");
assert_eq!(array.render(4), "[\n  1,\n  2,\n  3\n]");
```

<hr>
<br>

## How the layout engine works

A [`Doc`](./docs/API.md#doc) is a small tree of nodes: literal `text`, flexible breaks (`line`, `softline`, `hardline`), concatenation, `nest` (indentation), and `group` (a layout choice point). Rendering walks the tree once with an explicit work stack, carrying a *mode* — **flat** or **broken** — and the current column.

At each [`group`](./docs/API.md#group) the engine asks a single question: *do the group's contents, laid out flat, together with whatever follows them up to the next line break, fit in the width left on this line?* Every node records, when it is built, how many columns it uses flat and how many it uses before its first line break when broken; every entry on the work stack records how many columns it and everything queued after it use before the line ends. So the question is one addition and one comparison, and the whole render is linear in the size of the document. (Up to 1.0.0 it was a forward scan, which went quadratic on zero-width content; 1.0.1 makes the same decisions in O(1).) If the contents fit, every flexible break inside becomes its flat form — a space or nothing. If they do not — or the group contains a [`hardline`](./docs/API.md#hardline) — every flexible break inside becomes a newline plus the current indentation. The decision is all-or-nothing for the breaks a group directly owns; nested groups are decided independently, so an inner list can stay flat inside an outer one that broke.

Because the render pass uses a heap work stack rather than the call stack, a document nested tens of thousands of levels deep renders without a stack overflow — and is torn down the same way when dropped.

<hr>
<br>

## API Overview

For the complete reference with examples, see [`docs/API.md`](./docs/API.md).

- [`Doc`](./docs/API.md#doc) — the layout document; cheap to clone (`Rc`-backed).
  - **Build:** [`text`](./docs/API.md#text), [`nil`](./docs/API.md#nil), [`concat`](./docs/API.md#concat), [`join`](./docs/API.md#join).
  - **Break:** [`line`](./docs/API.md#line), [`softline`](./docs/API.md#softline), [`hardline`](./docs/API.md#hardline).
  - **Combine:** [`append`](./docs/API.md#append), [`nest`](./docs/API.md#nest), [`group`](./docs/API.md#group).
  - **Render:** [`render`](./docs/API.md#render), [`render_into`](./docs/API.md#render_into), [`render_writer`](./docs/API.md#render_writer) (behind `std`).

<br>

### Feature Flags

| Feature | Default | Description                                                             |
|---------|:-------:|-------------------------------------------------------------------------|
| `std`   | ✅      | Adds [`Doc::render_writer`](./docs/API.md#render_writer) for `io::Write` sinks. The crate core is `no_std` + `alloc`. |

<hr>
<br>

## Testing

```bash
cargo test                 # unit + doctests
cargo test --all-features  # adds the std io::Write tests
cargo test --test proptests # property-based invariants
cargo test --test equivalence # 1.0.1 engine vs the 1.0.0 algorithm
cargo test --test scale     # 1M-item zero-width documents (linear-time check)
cargo bench --bench bench  # Criterion layout benchmarks
```

The property suite in [`tests/proptests.rs`](./tests/proptests.rs) checks the core invariants — the flat layout matches an independently-built oracle, rendering never panics at any width (including 0, 1, and `usize::MAX`), `usize::MAX` lays out like any other unlimited width, `append` is associative, and `group` is idempotent — across randomized documents. [`tests/equivalence.rs`](./tests/equivalence.rs) checks that the O(1) fit test makes exactly the decisions the 1.0.0 scan made, against a line-for-line port of the 1.0.0 renderer.

<hr>
<br>

## Cross-Platform Support

The layout engine is pure computation with no platform-specific code, so it behaves identically everywhere Rust runs. CI covers **Linux**, **macOS**, and **Windows** on both stable and the 1.85 MSRV.

<hr>
<br>

## Contributing

See <a href="./REPS.md"><code>REPS.md</code></a> for the engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
