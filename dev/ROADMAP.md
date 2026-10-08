# pretty-lang - Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
> Master plan: ../../_strategy/LANG_COLLECTION.md
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

## v0.1.0 - Scaffold (DONE)
Compiles, CI green, structure correct, no domain logic.
- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt.

## v0.2.0 - Core (THE HARD PART, NOT DEFERRED) (DONE)
AST/CST-to-source rendering - a gofmt-style formatter for every language nearly free.
Shipped as a self-contained `Doc` layout algebra + linear-time renderer. No AST
or syntax dependency was wired: the `Doc` combinators are the reusable interface,
and a formatter builds a `Doc` from its own tree, so nothing in ast-lang/syntax
is needed at this tier. Wiring stays available for a later phase if a concrete
AST adapter is added.
Exit criteria:
- [x] Every public item has rustdoc + a runnable example.
- [x] Core invariants property-tested (full API authored + documented at this stage).

## v1.0.0 - API freeze (DONE)
Public surface stable and frozen until 2.0. No functional change from 0.2.0.
- [x] docs/API.md marked stable; SemVer promise recorded (inline crate docs + README too).
- [x] Full test + benchmark suite green on all three platforms (CI matrix; verified locally on Win + WSL2 + MSRV 1.85).

## v1.0.1 - Patch: unlimited width and linear fit test (DONE)
Fixes ISSUES M64 and the patch-level parts of M65. No public API change; every
layout an existing document produces at a width up to `isize::MAX` is
unchanged (checked against a port of the 1.0.0 renderer).

Delivered:
- **M64:** the target width is clamped into `isize` (`isize::try_from`, else
  `isize::MAX`) instead of `width as isize`, which turned `usize::MAX` into
  `-1` and broke every group. Every width at or above `isize::MAX` is now
  "unlimited". The benches used `usize::MAX` as their flat path and so had
  measured the broken layout; each flat bench now asserts its output is one
  line. Regression tests for `usize::MAX`, `isize::MAX + 1`, and hardlines at
  unlimited width; properties over widths 0, 1, and `usize::MAX`.
- **M65 (patch parts):** the fit test is O(1). Each node stores a `Fit`
  summary (flat width; break-mode width before its first owned line break),
  built in O(1) by the combinators, and each work-stack frame stores the
  columns it and everything after it use before the line ends. A group fits
  exactly when its flat width plus that total is within the columns left. This
  makes the decisions the 1.0.0 forward scan made (proven by
  `tests/equivalence.rs` against a line-for-line port of the 1.0.0 renderer,
  plus mutation checks), without the scan, which was quadratic on zero-width
  content (`nil`, empty text, `softline`, groups). The per-group `Vec`
  allocation in the old scan is gone with the scan itself: the render path
  allocates only its work stack.
- Widths are documented precisely as `chars().count()` everywhere a width
  appears (rustdoc, API.md, README).
- Scale tests at 1M items (`tests/scale.rs`) and a `pathological` bench group
  at 10k / 100k / 1M items.

Dependency wiring: unchanged. Still zero dependencies (runtime or first-party);
dev-dependencies are `criterion` and `proptest` only.

## Planned beyond 1.0.1 (recorded per the anti-deferral rule)
These change layouts or the public surface, so they cannot ship in a patch.
None is blocked by difficulty; each is held only by SemVer.
- **Unicode display width (M65).** `text` width is `chars().count()`, so CJK
  and emoji (2 cells) and combining marks (0 cells) are mis-measured for
  terminal layout. Switching the default measure changes the layout existing
  documents produce, which the 1.x contract forbids, so the default changes in
  **2.0** (UAX #11 widths via the first-party `unicode-lang`). An opt-in path
  is additive and can land in **1.1**: a constructor that takes a
  caller-supplied width (for example `Doc::text_with_width`), so callers can
  measure with their own display-width function today.
- **`Send`/`Sync` documents (M65).** `Doc` is `Rc`-backed and `!Send`. Moving
  to `Arc` (or making it selectable) changes auto-trait impls and costs atomic
  counts on every clone; decided once, in **2.0**.
- **New combinators (1.1, additive):** `align` (indent to the current
  column), `if_break` / flat-alternative (different content when flat versus
  broken, e.g. trailing commas), and `fill` (pack as many items per line as
  fit). Each needs its own `Fit` rule so the fit test stays O(1); `fill` in
  particular must keep its per-item decisions linear.
