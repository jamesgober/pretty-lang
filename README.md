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
        <strong>Status: pre-1.0, in active development.</strong> The public API is being designed across the 0.x series and frozen at <code>1.0.0</code>. See <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
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
        The engine is Wadler's <em>A Prettier Printer</em> in Lindig's linear-time imperative form: rendering is <code>O(document&nbsp;size)</code>, look-ahead is bounded by the target width, and neither pass recurses on the tree, so deeply nested documents cannot overflow the stack. The crate is <code>no_std</code> (needs only <code>alloc</code>) and contains no <code>unsafe</code> (<code>#![forbid(unsafe_code)]</code>).
    </p>
</div>

<hr>
<br>

## Performance First

Rendering is a single linear pass over the document with width-bounded look-ahead — no backtracking, no intermediate materialization, no per-node allocation on the render path. Latest local Criterion means (`cargo bench --bench bench`, Linux x86_64 / WSL2, Rust stable, release build):

| Workload | Layout | Time |
|----------|--------|-----:|
| JSON tree (~85 nodes, ~340 B out) | flat (fits) | ~1.8 µs (**~550 MiB/s**) |
| JSON tree (~85 nodes) | broken (reflowed) | ~4.0 µs |
| Call, 128 arguments | flat | ~1.9 µs |
| Call, 128 arguments | broken | ~2.7 µs |
| Call, 8 arguments | flat | ~190 ns |

Numbers vary by CPU and environment; run the suite on your target to establish a baseline. Building a `Doc` is one `Rc` allocation per combinator; rendering reuses a single work stack and writes straight into the output buffer.

<br>
<hr>

## Features

- **Eight combinators, any language** — `text`, `line`, `softline`, `hardline`, `append`, `nest`, `group`, plus `concat` / `join`. Build a `Doc` from any AST and render.
- **Fit-aware reflow** — [`group`](./docs/API.md#group) lays its contents flat when they fit the remaining width and breaks them all-or-nothing when they do not.
- **Linear time, no recursion** — Wadler/Lindig layout with bounded look-ahead; deep documents render and drop without overflowing the stack.
- **Stream or collect** — render to a [`String`](./docs/API.md#render), into any [`core::fmt::Write`](./docs/API.md#render_into), or into a [`std::io::Write`](./docs/API.md#render_writer).
- **`no_std`** — needs only `alloc`; the `std` feature adds the I/O renderer.
- **Fully safe** — no `unsafe`, `#![forbid(unsafe_code)]`.
- **Property-tested** — flat-layout, panic-safety, associativity, and idempotence invariants checked across randomized documents with `proptest`.

<br>
<hr>

## Installation

```toml
[dependencies]
pretty-lang = "0.2"

# no_std (drops the io::Write renderer):
pretty-lang = { version = "0.2", default-features = false }
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

At each [`group`](./docs/API.md#group) the engine asks a single question: *do the group's contents, laid out flat, fit in the width left on this line?* The look-ahead that answers it scans only as far as the first newline or the first column past the target width, which is what keeps the whole algorithm linear. If the contents fit, every flexible break inside becomes its flat form — a space or nothing. If they do not — or the group contains a [`hardline`](./docs/API.md#hardline) — every flexible break inside becomes a newline plus the current indentation. The decision is all-or-nothing for the breaks a group directly owns; nested groups are decided independently, so an inner list can stay flat inside an outer one that broke.

Because both the render pass and the look-ahead use heap work stacks rather than the call stack, a document nested tens of thousands of levels deep renders without a stack overflow — and is torn down the same way when dropped.

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
cargo bench --bench bench  # Criterion layout benchmarks
```

The property suite in [`tests/proptests.rs`](./tests/proptests.rs) checks the core invariants — the flat layout matches an independently-built oracle, rendering never panics at any width, `append` is associative, and `group` is idempotent — across randomized documents.

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
