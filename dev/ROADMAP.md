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

## v1.0.0 - API freeze
Public surface stable and frozen until 2.0.
- [ ] docs/API.md marked stable; SemVer promise recorded.
- [ ] Full test + benchmark suite green on all three platforms.
