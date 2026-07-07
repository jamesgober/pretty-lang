<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>pretty-lang</b><br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="../CHANGELOG.md" title="Changelog"><b>CHANGELOG</b></a>
    </sup>
</div>
<br>

A language-agnostic pretty-printer. The entire public surface is one type —
[`Doc`](#doc) — with a handful of constructors, three combinators, and three
render methods. You build a `Doc` from any syntax tree and render it against a
target width; the engine chooses where the lines break.

- **Version:** 1.0.0
- **MSRV:** Rust 1.85 (2024 edition)
- **`no_std`:** yes (needs `alloc`; the `std` feature adds the `io::Write` renderer)
- **Unsafe:** none — `#![forbid(unsafe_code)]`
- **Stability:** stable — the surface below is frozen (see [Stability](#stability)).

## Table of Contents

- **[Stability](#stability)**
- **[Installation](#installation)**
- **[Quick Start](#quick-start)**
- **[Mental model](#mental-model)**
- **[Public API](#public-api)**
  - [`Doc`](#doc)
    - [Constructors](#constructors)
      - [`Doc::nil`](#nil)
      - [`Doc::text`](#text)
      - [`Doc::concat`](#concat)
      - [`Doc::join`](#join)
    - [Line breaks](#line-breaks)
      - [`Doc::line`](#line)
      - [`Doc::softline`](#softline)
      - [`Doc::hardline`](#hardline)
    - [Combinators](#combinators)
      - [`Doc::append`](#append)
      - [`Doc::nest`](#nest)
      - [`Doc::group`](#group)
    - [Rendering](#rendering)
      - [`Doc::render`](#render)
      - [`Doc::render_into`](#render_into)
      - [`Doc::render_writer`](#render_writer)
    - [Trait implementations](#trait-implementations)
- **[Feature flags](#feature-flags)**
- **[Recipes](#recipes)**
- **[Design notes](#design-notes)**
  - [Why the width fits are linear](#why-linear)
  - [Text width is measured in Unicode scalars](#text-width)
  - [Single-threaded by design](#single-threaded)

<br>

## Stability

As of **1.0.0** the public API documented here is **stable and frozen**. The
crate follows [Semantic Versioning](https://semver.org):

- Nothing in the frozen surface — the [`Doc`](#doc) type, its constructors
  ([`nil`](#nil), [`text`](#text), [`concat`](#concat), [`join`](#join)), line
  breaks ([`line`](#line), [`softline`](#softline), [`hardline`](#hardline)),
  combinators ([`append`](#append), [`nest`](#nest), [`group`](#group)), render
  methods ([`render`](#render), [`render_into`](#render_into),
  [`render_writer`](#render_writer)), the [trait
  implementations](#trait-implementations), and the `std` feature flag — will be
  removed or changed in a breaking way within the `1.x` series. A breaking change
  means a new major version.
- `1.x` releases may **add** to the surface (new combinators, new render sinks,
  new trait impls) without breaking existing code.
- The **rendered output** of a given document at a given width is part of the
  contract: a `1.x` release will not change the layout an existing document
  produces, except to fix a documented bug. New layout behaviour arrives through
  new combinators, not by re-interpreting old ones.
- The internal node representation behind `Doc` is not part of the surface; it
  stays private and may change freely.

MSRV (Rust 1.85) is treated as a compatibility surface: a raise is a minor,
documented change, never a patch.

<br>

## Installation

```toml
[dependencies]
pretty-lang = "1"

# no_std (drops the io::Write renderer):
pretty-lang = { version = "1", default-features = false }
```

<br>

## Quick Start

```rust
use pretty_lang::Doc;

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

assert_eq!(call.render(80), "f(a, b, c)");
assert_eq!(call.render(6), "f(\n    a,\n    b,\n    c\n)");
```

<br>

## Mental model

A `Doc` is a *description*, not a string. It records intent — group these
pieces, break here, indent that — and the concrete line breaks are chosen later
by a render call against a width.

Three ideas cover everything:

1. **Flexible breaks** ([`line`](#line), [`softline`](#softline)) render as a
   space or nothing when their group is *flat*, and as a newline when it is
   *broken*. A [`hardline`](#hardline) is always a newline and forces breaking.
2. **[`group`](#group)** is the choice point: its contents lay out flat if they
   fit the width remaining on the current line, otherwise every flexible break
   in the group becomes a newline — all or nothing.
3. **[`nest`](#nest)** sets the indentation those newlines land at.

Everything else is composition with [`append`](#append), [`concat`](#concat),
and [`join`](#join).

<br>

## Public API

### `Doc`

```rust
pub struct Doc(/* private */);
```

An immutable, cheaply-clonable description of a document's layout. `Doc` is a
thin handle around a reference-counted node, so [`Clone`](#trait-implementations)
is a pointer-count bump, not a deep copy. It is single-threaded by design (not
`Send`/`Sync`); see [Design notes](#single-threaded).

All constructors and combinators are pure: they build and return a new `Doc` and
never mutate in place. Combinators take `self` by value so builder chains read
top to bottom.

<br>

### Constructors

<h4 id="nil"><code>Doc::nil</code></h4>

```rust
pub fn nil() -> Doc
```

The empty document. Renders to nothing and is the identity for
[`append`](#append) — `a.append(Doc::nil())` and `Doc::nil().append(a)` both
render exactly as `a`. [`Doc::default()`](#trait-implementations) is the same
thing.

**Returns:** an empty `Doc`.

```rust
use pretty_lang::Doc;

assert_eq!(Doc::nil().render(80), "");
assert_eq!(Doc::text("x").append(Doc::nil()).render(80), "x");

// Useful as a base case: an optional trailing comma.
fn maybe_comma(last: bool) -> Doc {
    if last { Doc::nil() } else { Doc::text(",") }
}
assert_eq!(Doc::text("item").append(maybe_comma(true)).render(80), "item");
```

<br>

<h4 id="text"><code>Doc::text</code></h4>

```rust
pub fn text(s: impl Into<Cow<'static, str>>) -> Doc
```

A literal, unbreakable piece of text.

**Parameters:**

- `s` — anything convertible into `Cow<'static, str>`. A string literal
  (`&'static str`) is stored without allocating; an owned `String` is moved in.
  The display width is measured once, here, as the number of Unicode scalar
  values (see [Text width](#text-width)).

**Returns:** a `Doc` that renders `s` verbatim.

**Contract:** `s` MUST NOT contain a `'\n'`. Text is treated as a single unit
the engine never breaks; embed line breaks with [`line`](#line),
[`softline`](#softline), or [`hardline`](#hardline) so they are accounted for. A
newline inside `text` is printed as-is but throws off width tracking.

```rust
use pretty_lang::Doc;

// A static literal — no allocation.
assert_eq!(Doc::text("let x").render(80), "let x");

// An owned, computed string.
let name = format!("tmp_{}", 7);
assert_eq!(Doc::text(name).render(80), "tmp_7");

// Non-ASCII text is measured by scalar count, not bytes.
assert_eq!(Doc::text("café").render(80), "café");
```

<br>

<h4 id="concat"><code>Doc::concat</code></h4>

```rust
pub fn concat(docs: impl IntoIterator<Item = Doc>) -> Doc
```

Concatenate every document from `docs`, in order. A left fold of
[`append`](#append).

**Parameters:**

- `docs` — an iterator of documents to lay out one after another.

**Returns:** the concatenation, or [`nil`](#nil) for an empty iterator.

```rust
use pretty_lang::Doc;

let doc = Doc::concat(["a", "b", "c"].map(Doc::text));
assert_eq!(doc.render(80), "abc");

assert_eq!(Doc::concat(core::iter::empty()).render(80), "");
```

<br>

<h4 id="join"><code>Doc::join</code></h4>

```rust
pub fn join(sep: Doc, docs: impl IntoIterator<Item = Doc>) -> Doc
```

Concatenate every document from `docs`, placing a clone of `sep` between
consecutive items — but not before the first or after the last.

**Parameters:**

- `sep` — the separator inserted between items. Cloning it is an `Rc` bump.
- `docs` — the items to join.

**Returns:** the interspersed document, or [`nil`](#nil) for an empty iterator.

This is the idiomatic way to render comma-separated lists, `::`-joined paths,
and `&&`-joined conditions. Pair it with [`group`](#group) so the whole list
collapses onto one line when it fits.

```rust
use pretty_lang::Doc;

// A path — fixed separator.
let path = Doc::join(Doc::text("::"), ["std", "fmt", "Write"].map(Doc::text));
assert_eq!(path.render(80), "std::fmt::Write");

// A reflowing argument list — flexible separator under a group.
let args = Doc::join(Doc::text(",").append(Doc::line()), ["x", "y", "z"].map(Doc::text)).group();
assert_eq!(args.render(80), "x, y, z");
assert_eq!(args.render(3), "x,\ny,\nz");

// A single item carries no separator.
let one = Doc::join(Doc::text(","), core::iter::once(Doc::text("solo")));
assert_eq!(one.render(80), "solo");
```

<br>

### Line breaks

<h4 id="line"><code>Doc::line</code></h4>

```rust
pub fn line() -> Doc
```

A flexible break: a single space when its enclosing [`group`](#group) is flat, a
newline plus the current indentation when the group breaks. The workhorse
separator between items that should sit on one line when they fit and stack when
they do not.

**Returns:** a flexible line break.

```rust
use pretty_lang::Doc;

let doc = Doc::text("a").append(Doc::line()).append(Doc::text("b")).group();
assert_eq!(doc.render(80), "a b");  // fits: space
assert_eq!(doc.render(1), "a\nb");  // too narrow: newline
```

<br>

<h4 id="softline"><code>Doc::softline</code></h4>

```rust
pub fn softline() -> Doc
```

A flexible break that is *nothing* when its group is flat and a newline plus
indentation when the group breaks. Use it where a broken layout wants a line
break but a flat layout wants no gap at all — for example just inside an opening
bracket.

**Returns:** a flexible, zero-width line break.

```rust
use pretty_lang::Doc;

let doc = Doc::text("(").append(Doc::softline()).append(Doc::text("x")).group();
assert_eq!(doc.render(80), "(x");   // flat: no gap
assert_eq!(doc.render(1), "(\nx");  // broken: newline
```

<br>

<h4 id="hardline"><code>Doc::hardline</code></h4>

```rust
pub fn hardline() -> Doc
```

A break that is *always* a newline, and forces every [`group`](#group)
containing it to break. Use it for constructs that must never collapse onto one
line: line comments, statement separators in a block body, blank lines.

**Returns:** an unconditional line break.

```rust
use pretty_lang::Doc;

let doc = Doc::text("a").append(Doc::hardline()).append(Doc::text("b")).group();
// "a b" would fit at width 80, but the hardline forces the break.
assert_eq!(doc.render(80), "a\nb");
```

A hardline propagates outward: any group that contains one — directly or through
a nested group — is forced to break.

```rust
use pretty_lang::Doc;

let inner = Doc::text("x").append(Doc::line()).append(Doc::text("y")).group();
let outer = inner.append(Doc::hardline()).append(Doc::text("z")).group();
assert_eq!(outer.render(80), "x y\nz");
```

<br>

### Combinators

<h4 id="append"><code>Doc::append</code></h4>

```rust
pub fn append(self, other: Doc) -> Doc
```

Concatenate `self` with `other`, laid out left then right. The fundamental way
to build a document from parts.

**Parameters:**

- `self` — the left document (consumed).
- `other` — the right document.

**Returns:** the concatenation. `append` is associative under rendering:
`a.append(b).append(c)` and `a.append(b.append(c))` render identically.

```rust
use pretty_lang::Doc;

let doc = Doc::text("fn ").append(Doc::text("main")).append(Doc::text("()"));
assert_eq!(doc.render(80), "fn main()");
```

<br>

<h4 id="nest"><code>Doc::nest</code></h4>

```rust
pub fn nest(self, indent: isize) -> Doc
```

Increase the indentation applied to every line break *inside* `self` by `indent`
columns.

**Parameters:**

- `self` — the document whose inner breaks are indented (consumed).
- `indent` — columns to add. Indentation is relative and nests additively: a
  `nest(2)` inside a `nest(2)` indents inner breaks by four. A negative value
  dedents; the effective indentation is clamped at zero when a newline is
  emitted.

**Returns:** the nested document. `nest` on a document that stays flat has no
visible effect — only line breaks that actually happen are indented.

```rust
use pretty_lang::Doc;

let body = Doc::text("{")
    .append(Doc::line().append(Doc::text("stmt;")).nest(4))
    .append(Doc::line())
    .append(Doc::text("}"))
    .group();

assert_eq!(body.render(80), "{ stmt; }");            // flat: nest unused
assert_eq!(body.render(4), "{\n    stmt;\n}");        // broken: indented by 4
```

<br>

<h4 id="group"><code>Doc::group</code></h4>

```rust
pub fn group(self) -> Doc
```

Mark `self` as a layout choice point.

When the renderer reaches a group it asks whether the group's contents fit, laid
out flat, in the width remaining on the current line. If they do, every flexible
break inside becomes its flat form (a space or nothing). If they do not — or the
group contains a [`hardline`](#hardline) — every flexible break inside becomes a
newline. The decision is all-or-nothing for the breaks this group *directly*
owns; nested groups are decided independently.

**Parameters:**

- `self` — the document to treat as one fit-or-break unit (consumed).

**Returns:** the grouped document. Grouping is idempotent:
`d.group().group()` renders the same as `d.group()`.

A document with no groups always uses the broken form of every break — grouping
is what turns a document into "one line if it fits, otherwise stacked".

```rust
use pretty_lang::Doc;

let list = Doc::text("[")
    .append(
        Doc::softline()
            .append(Doc::join(Doc::text(",").append(Doc::line()), ["1", "2", "3"].map(Doc::text)))
            .nest(2),
    )
    .append(Doc::softline())
    .append(Doc::text("]"))
    .group();

assert_eq!(list.render(80), "[1, 2, 3]");
assert_eq!(list.render(4), "[\n  1,\n  2,\n  3\n]");
```

<br>

### Rendering

<h4 id="render"><code>Doc::render</code></h4>

```rust
pub fn render(&self, width: usize) -> String
```

Render to an owned [`String`], choosing line breaks so no line exceeds `width`
columns where the document allows a choice.

**Parameters:**

- `self` — the document (borrowed; rendering does not consume it, so one `Doc`
  can be rendered at several widths).
- `width` — the target line length, in Unicode scalars.

**Returns:** the laid-out text. Lines can still exceed `width` where a single
unbreakable [`text`](#text) is wider than `width`, or where the document offers
no break — the renderer never invents break points that were not described.

```rust
use pretty_lang::Doc;

let doc = Doc::text("a").append(Doc::line()).append(Doc::text("b")).group();
assert_eq!(doc.render(80), "a b");
assert_eq!(doc.render(1), "a\nb");
```

<br>

<h4 id="render_into"><code>Doc::render_into</code></h4>

```rust
pub fn render_into<W: core::fmt::Write>(&self, width: usize, out: &mut W) -> core::fmt::Result
```

Render into any [`core::fmt::Write`] sink, for the target `width`. Streams
directly into a caller-owned buffer and avoids the intermediate `String` that
[`render`](#render) allocates.

**Parameters:**

- `self` — the document.
- `width` — the target line length.
- `out` — the sink to write into (a `String`, a formatter, a custom buffer).

**Returns:** `Ok(())`, or `Err(core::fmt::Error)` if and only if `out` returns
an error while being written to.

```rust
use pretty_lang::Doc;

let doc = Doc::text("hello").append(Doc::text(" world"));
let mut buf = String::new();
doc.render_into(80, &mut buf).unwrap();
assert_eq!(buf, "hello world");
```

<br>

<h4 id="render_writer"><code>Doc::render_writer</code></h4>

```rust
// Requires the `std` feature (enabled by default).
pub fn render_writer<W: std::io::Write>(&self, width: usize, out: &mut W) -> std::io::Result<()>
```

Render into a [`std::io::Write`] sink, for the target `width` — the streaming
counterpart of [`render`](#render) for files, sockets, and stdout.

**Parameters:**

- `self` — the document.
- `width` — the target line length.
- `out` — the I/O sink.

**Returns:** `Ok(())`, or the first [`std::io::Error`] returned by `out`
(propagated unchanged, not flattened to a formatting error).

```rust
use pretty_lang::Doc;

let doc = Doc::text("written to a byte sink");
let mut buf: Vec<u8> = Vec::new();
doc.render_writer(80, &mut buf).unwrap();
assert_eq!(buf, b"written to a byte sink");
```

<br>

### Trait implementations

| Trait | Behaviour |
|-------|-----------|
| `Clone` | Pointer-count bump on the backing `Rc`; O(1), no deep copy. |
| `Default` | Returns [`Doc::nil()`](#nil). |
| `Debug` | A structural view of the node tree (`Group(Cat(Text("a"), Line))`), rendered iteratively so a deep document does not overflow the stack. |
| `From<&'static str>` | `Doc::text` for a static slice — `let d: Doc = "x".into();`. |
| `From<String>` | `Doc::text` for an owned string. |
| `Drop` | Dismantles a uniquely-owned deep document iteratively, so dropping a document nested tens of thousands of levels deep cannot overflow the stack. Leaves and shared nodes take a branch-only fast path that allocates nothing. |

```rust
use pretty_lang::Doc;

let a: Doc = "static".into();
let b: Doc = String::from("owned").into();
assert_eq!(a.render(80), "static");
assert_eq!(b.render(80), "owned");
assert_eq!(Doc::default().render(80), "");
```

<br>

## Feature flags

| Feature | Default | Description |
|---------|:-------:|-------------|
| `std`   | ✅      | Adds [`Doc::render_writer`](#render_writer) for `io::Write` sinks. With it off, the crate is `no_std` and needs only `alloc`. |

<br>

## Recipes

**Bracket a body so it collapses when it fits.** The single most useful pattern —
a `softline` just inside each bracket, the body `nest`ed, the whole thing
`group`ed:

```rust
use pretty_lang::Doc;

fn bracket(open: &'static str, body: Doc, close: &'static str) -> Doc {
    Doc::text(open)
        .append(Doc::softline().append(body).nest(2))
        .append(Doc::softline())
        .append(Doc::text(close))
        .group()
}

let obj = bracket(
    "{",
    Doc::join(Doc::text(",").append(Doc::line()), ["a: 1", "b: 2"].map(Doc::text)),
    "}",
);
// `softline` puts no gap inside the braces when flat; swap it for `line` in
// `bracket` if you want `{ a: 1, b: 2 }` with inner spaces.
assert_eq!(obj.render(80), "{a: 1, b: 2}");
assert_eq!(obj.render(6), "{\n  a: 1,\n  b: 2\n}");
```

**A block that always breaks.** Swap the `softline`s for [`hardline`](#hardline)s
when the body must never collapse (a function body, say):

```rust
use pretty_lang::Doc;

let block = Doc::text("{")
    .append(Doc::hardline().append(Doc::text("body();")).nest(4))
    .append(Doc::hardline())
    .append(Doc::text("}"));
assert_eq!(block.render(80), "{\n    body();\n}");
```

**Stream to stdout without an intermediate string** (needs `std`):

```rust
# fn main() -> std::io::Result<()> {
use pretty_lang::Doc;

let doc = Doc::text("line one").append(Doc::hardline()).append(Doc::text("line two"));
doc.render_writer(80, &mut std::io::stdout())?;
# Ok(())
# }
```

<br>

## Design notes

<h3 id="why-linear">Why the width fits are linear</h3>

At each [`group`](#group) the renderer runs a look-ahead — *does the flat form
fit the width left on this line?* — that stops the moment the answer is known:
the width is exhausted, or a newline is reached. Because it never scans past the
end of the current line, the total work across all groups is proportional to the
document size, not quadratic. Both the render pass and the look-ahead use heap
work stacks rather than recursion, so neither the depth nor the breadth of a
document can overflow the call stack.

<h3 id="text-width">Text width is measured in Unicode scalars</h3>

[`text`](#text) measures width as `chars().count()` — the number of Unicode
scalar values — not bytes and not grapheme clusters. For source code this is the
right, cheap default: `"café"` is four columns wide, matching how it displays in
a fixed-width editor, even though it is five bytes. Combining marks and East
Asian wide characters are counted as one column each; if you need
terminal-accurate width for CJK or emoji, pre-measure and pad your `text` nodes
yourself.

<h3 id="single-threaded">Single-threaded by design</h3>

`Doc` is backed by `Rc`, not `Arc`, so it is not `Send`/`Sync`. A formatter
builds and renders a document on one thread; paying for atomic reference counts
on every clone would be waste. Build documents per-thread; share the rendered
`String` across threads if you need to.

<br>
<hr>

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
