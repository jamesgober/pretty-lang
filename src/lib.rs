//! # pretty_lang
//!
//! A language-agnostic pretty-printer: turn any syntax tree into laid-out source
//! text that reflows to a target line width. It is the rendering half of a
//! formatter — a `gofmt`-style tool for any language, nearly free — and knows
//! nothing about grammars. You describe the layout with a handful of combinators
//! and pretty_lang decides where the lines break.
//!
//! ## The idea
//!
//! You do not print strings directly. Instead you build a [`Doc`]: a lazy
//! description that says *these pieces belong together*, *put a break here that
//! becomes a space or a newline*, *indent the inside by four*, *keep this on one
//! line if it fits*. Rendering a [`Doc`] against a width then chooses concrete
//! line breaks. The same document renders compactly at width 100 and stacked at
//! width 20, with no branching in your code.
//!
//! The engine is Wadler's *A Prettier Printer* in Lindig's linear-time
//! imperative form: rendering is `O(document size)`, look-ahead is bounded by
//! the target width, and neither pass recurses on the tree, so deeply nested
//! documents cannot overflow the stack.
//!
//! ## Quick start
//!
//! ```
//! use pretty_lang::Doc;
//!
//! // Build `f(a, b, c)` as a document that can break into one-argument-per-line.
//! let call = Doc::text("f(")
//!     .append(
//!         Doc::softline()
//!             .append(Doc::join(
//!                 Doc::text(",").append(Doc::line()),
//!                 ["a", "b", "c"].map(Doc::text),
//!             ))
//!             .nest(4),
//!     )
//!     .append(Doc::softline())
//!     .append(Doc::text(")"))
//!     .group();
//!
//! // Wide: it all fits on one line.
//! assert_eq!(call.render(80), "f(a, b, c)");
//!
//! // Narrow: the group breaks and the arguments stack, indented.
//! assert_eq!(call.render(6), "f(\n    a,\n    b,\n    c\n)");
//! ```
//!
//! ## The combinators
//!
//! | Build with | Meaning |
//! |------------|---------|
//! | [`Doc::text`] | literal, unbreakable text |
//! | [`Doc::line`] | space when flat, newline when broken |
//! | [`Doc::softline`] | nothing when flat, newline when broken |
//! | [`Doc::hardline`] | always a newline; forces enclosing groups to break |
//! | [`Doc::append`] | put one document after another |
//! | [`Doc::concat`] / [`Doc::join`] | fold / intersperse a sequence |
//! | [`Doc::nest`] | indent the line breaks inside a document |
//! | [`Doc::group`] | lay flat if it fits, otherwise break every flexible line |
//!
//! Render with [`Doc::render`] (to a [`String`]), [`Doc::render_into`] (into any
//! [`core::fmt::Write`]), or [`Doc::render_writer`] (into a [`std::io::Write`],
//! behind the `std` feature).
//!
//! ## `no_std`
//!
//! The crate is `no_std` and needs only `alloc`. The default `std` feature adds
//! the [`Doc::render_writer`] I/O sink. There is no `unsafe` anywhere
//! (`#![forbid(unsafe_code)]`).
//!
//! ## Stability
//!
//! As of `1.0.0` the public surface — the [`Doc`] type, its constructors,
//! combinators, render methods, and trait implementations, together with the
//! `std` feature flag — is stable and frozen under [Semantic Versioning]. It
//! will not change in a breaking way within the `1.x` series; `1.x` releases may
//! only add. See `docs/API.md` for the full promise.
//!
//! [Semantic Versioning]: https://semver.org

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod doc;
mod render;

pub use doc::Doc;

#[cfg(test)]
mod tests {
    use super::Doc;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[test]
    fn test_nil_renders_empty() {
        assert_eq!(Doc::nil().render(80), "");
        assert_eq!(Doc::default().render(80), "");
    }

    #[test]
    fn test_text_renders_verbatim() {
        assert_eq!(Doc::text("hello").render(80), "hello");
        assert_eq!(Doc::text(String::from("owned")).render(80), "owned");
    }

    #[test]
    fn test_append_is_left_to_right() {
        let doc = Doc::text("a").append(Doc::text("b")).append(Doc::text("c"));
        assert_eq!(doc.render(80), "abc");
    }

    #[test]
    fn test_nil_is_append_identity() {
        let a = Doc::text("x");
        assert_eq!(a.clone().append(Doc::nil()).render(80), "x");
        assert_eq!(Doc::nil().append(a).render(80), "x");
    }

    #[test]
    fn test_line_flat_is_space_broken_is_newline() {
        let doc = Doc::text("a").append(Doc::line()).append(Doc::text("b"));
        // Grouped and fitting: flat, so the line is a space.
        assert_eq!(doc.clone().group().render(80), "a b");
        // Grouped and too narrow: broken, so the line is a newline.
        assert_eq!(doc.clone().group().render(1), "a\nb");
        // Ungrouped: the root is always broken.
        assert_eq!(doc.render(80), "a\nb");
    }

    #[test]
    fn test_softline_flat_is_empty() {
        let doc = Doc::text("(")
            .append(Doc::softline())
            .append(Doc::text("x"))
            .group();
        assert_eq!(doc.clone().render(80), "(x");
        assert_eq!(doc.render(1), "(\nx");
    }

    #[test]
    fn test_hardline_forces_break_even_when_it_fits() {
        let doc = Doc::text("a")
            .append(Doc::hardline())
            .append(Doc::text("b"))
            .group();
        assert_eq!(doc.render(80), "a\nb");
    }

    #[test]
    fn test_hardline_forces_all_enclosing_groups() {
        // An inner group that would fit is still broken because the outer group
        // is forced by the hardline it also contains.
        let inner = Doc::text("x")
            .append(Doc::line())
            .append(Doc::text("y"))
            .group();
        let doc = inner.append(Doc::hardline()).append(Doc::text("z")).group();
        assert_eq!(doc.render(80), "x y\nz");
    }

    #[test]
    fn test_nest_indents_broken_lines_only() {
        let doc = Doc::text("{")
            .append(Doc::line().append(Doc::text("body")).nest(4))
            .append(Doc::line())
            .append(Doc::text("}"))
            .group();
        assert_eq!(doc.clone().render(80), "{ body }");
        assert_eq!(doc.render(4), "{\n    body\n}");
    }

    #[test]
    fn test_nest_nests_additively() {
        let doc = Doc::text("a")
            .append(
                Doc::line()
                    .append(Doc::text("b"))
                    .append(Doc::line().append(Doc::text("c")).nest(2))
                    .nest(2),
            )
            .group();
        assert_eq!(doc.render(1), "a\n  b\n    c");
    }

    #[test]
    fn test_negative_nest_clamps_at_zero() {
        let doc = Doc::text("a")
            .append(Doc::line().append(Doc::text("b")).nest(-10))
            .group();
        assert_eq!(doc.render(1), "a\nb");
    }

    #[test]
    fn test_group_all_or_nothing() {
        // Two breaks in one group break together, never one-of-two.
        let doc = Doc::join(Doc::line(), ["a", "b", "c"].map(Doc::text)).group();
        assert_eq!(doc.clone().render(80), "a b c");
        assert_eq!(doc.render(3), "a\nb\nc");
    }

    #[test]
    fn test_concat_folds_in_order() {
        let doc = Doc::concat(["1", "2", "3"].map(Doc::text));
        assert_eq!(doc.render(80), "123");
    }

    #[test]
    fn test_concat_empty_is_nil() {
        assert_eq!(Doc::concat(core::iter::empty()).render(80), "");
    }

    #[test]
    fn test_join_intersperses_separator() {
        let doc = Doc::join(Doc::text("::"), ["a", "b", "c"].map(Doc::text));
        assert_eq!(doc.render(80), "a::b::c");
    }

    #[test]
    fn test_join_single_item_has_no_separator() {
        let doc = Doc::join(Doc::text(","), core::iter::once(Doc::text("solo")));
        assert_eq!(doc.render(80), "solo");
    }

    #[test]
    fn test_join_empty_is_nil() {
        assert_eq!(
            Doc::join(Doc::text(","), core::iter::empty()).render(80),
            ""
        );
    }

    #[test]
    fn test_render_into_matches_render() {
        let doc = Doc::text("a")
            .append(Doc::line())
            .append(Doc::text("b"))
            .group();
        let mut buf = String::new();
        doc.render_into(80, &mut buf).unwrap();
        assert_eq!(buf, doc.render(80));
    }

    #[test]
    fn test_wide_text_overflows_when_no_break_offered() {
        // The renderer never invents break points: an unbreakable word wider
        // than the target width is emitted as-is.
        let doc = Doc::text("unbreakable");
        assert_eq!(doc.render(3), "unbreakable");
    }

    #[test]
    fn test_from_impls() {
        let a: Doc = "static".into();
        let b: Doc = String::from("owned").into();
        assert_eq!(a.render(80), "static");
        assert_eq!(b.render(80), "owned");
    }

    #[test]
    fn test_unicode_width_counts_scalars_not_bytes() {
        // "café" is 5 bytes but 4 columns; at width 4 it still fits flat.
        let doc = Doc::text("café")
            .append(Doc::line())
            .append(Doc::text("x"))
            .group();
        assert_eq!(doc.render(4), "café\nx");
    }

    #[test]
    fn test_deeply_nested_does_not_overflow_stack() {
        // Build a left-leaning spine far deeper than the call stack allows for
        // recursion; the iterative engine must handle it.
        let mut doc = Doc::text("end");
        for _ in 0..100_000 {
            doc = Doc::text("x").append(doc);
        }
        let out = doc.render(80);
        assert!(out.ends_with("end"));
        assert_eq!(out.len(), 100_000 + 3);
    }

    #[test]
    fn test_debug_is_structural() {
        let doc = Doc::text("a").append(Doc::line()).group();
        let s = alloc::format!("{doc:?}");
        assert_eq!(s, "Group(Cat(Text(\"a\"), Line))");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_render_writer_to_vec() {
        let doc = Doc::text("io").append(Doc::text(" sink"));
        let mut buf: Vec<u8> = Vec::new();
        doc.render_writer(80, &mut buf).unwrap();
        assert_eq!(buf, b"io sink");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_render_writer_propagates_io_error() {
        // A sink that fails on first write must surface its error, not a bare
        // formatting error.
        struct Failing;
        impl std::io::Write for Failing {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "nope"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let doc = Doc::text("data");
        let err = doc.render_writer(80, &mut Failing).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    }
}
