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
//! The engine is Wadler's *A Prettier Printer* in Lindig's imperative form.
//! Rendering is one `O(document size)` pass: each node carries a width summary
//! computed when it is built, so the "does this group fit?" test is O(1) and
//! never rescans the rest of the line. Nothing recurses on the tree, so deeply
//! nested documents cannot overflow the stack.
//!
//! ## Widths
//!
//! Widths are counted in `char`s: a [`Doc::text`] is `str::chars().count()`
//! columns wide (Unicode scalar values, not bytes, not grapheme clusters, not
//! terminal display cells), a flat [`Doc::line`] is one column, and each space
//! of indentation is one column. The `width` given to a render method is
//! measured the same way. Any width at or above `isize::MAX` means
//! "unlimited".
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
    // Only the `std` io-sink tests collect into a byte vector.
    #[cfg(feature = "std")]
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

    /// A grouped argument list, shaped like the crate's own flat bench.
    fn call(n: usize) -> Doc {
        Doc::text("call(")
            .append(
                Doc::softline()
                    .append(Doc::join(
                        Doc::text(",").append(Doc::line()),
                        (0..n).map(|i| Doc::text(alloc::format!("arg{i}"))),
                    ))
                    .nest(4),
            )
            .append(Doc::softline())
            .append(Doc::text(")"))
            .group()
    }

    #[test]
    fn test_render_usize_max_is_flat() {
        // M64 regression: 1.0.0 computed `usize::MAX as isize == -1`, so every
        // group broke at the "unlimited" width.
        let doc = call(8);
        let out = doc.render(usize::MAX);
        assert!(!out.contains('\n'), "{out}");
        assert_eq!(out, doc.render(1_000));
    }

    #[test]
    fn test_widths_above_isize_max_behave_as_isize_max() {
        let doc = call(4).append(Doc::line()).append(call(3)).group();
        let expected = doc.render(isize::MAX as usize);
        for width in [
            isize::MAX as usize + 1,
            usize::MAX / 2 + 2,
            usize::MAX - 1,
            usize::MAX,
        ] {
            assert_eq!(doc.render(width), expected, "width {width}");
            let mut buf = String::new();
            doc.render_into(width, &mut buf).unwrap();
            assert_eq!(buf, expected);
        }
    }

    #[test]
    fn test_hardline_still_breaks_at_usize_max() {
        // An unlimited width does not make a hardline fit.
        let doc = Doc::text("a")
            .append(Doc::line())
            .append(Doc::text("b"))
            .append(Doc::hardline())
            .append(Doc::text("c"))
            .group();
        assert_eq!(doc.render(usize::MAX), "a\nb\nc");
    }

    #[test]
    fn test_zero_width_groups_fit_at_width_zero() {
        // Zero-width flat content fits in zero remaining columns.
        assert_eq!(Doc::softline().group().render(0), "");
        assert_eq!(Doc::text("").group().render(0), "");
        // A `line` is one column flat, so it does not.
        assert_eq!(Doc::line().group().render(0), "\n");
        assert_eq!(Doc::line().group().render(1), " ");
    }

    #[test]
    fn test_group_fit_counts_the_rest_of_the_line() {
        // The group itself fits in 3 columns, but the text glued after it
        // (before the next break) does not, so the group breaks.
        let item = Doc::text("a").append(Doc::line()).append(Doc::text("b"));
        let doc = item
            .group()
            .append(Doc::text("tail"))
            .append(Doc::line())
            .append(Doc::text("z"));
        assert_eq!(doc.render(6), "a\nbtail\nz");
        assert_eq!(doc.render(7), "a btail\nz");
    }

    #[test]
    fn test_width_is_char_count_not_display_width() {
        // Documented 1.x behaviour: widths are `chars().count()`. A CJK
        // character (two terminal columns) and a combining mark (zero) each
        // count as one.
        let wide = Doc::text("日本")
            .append(Doc::line())
            .append(Doc::text("x"))
            .group();
        assert_eq!(wide.render(4), "日本 x");
        let combining = Doc::text("e\u{301}")
            .append(Doc::line())
            .append(Doc::text("x"))
            .group();
        assert_eq!(combining.render(4), "e\u{301} x");
        assert_eq!(combining.render(3), "e\u{301}\nx");
        // An emoji (two cells) and a tab each count as one; the bytes do not
        // matter.
        let emoji_tab = Doc::text("\u{1F600}\t")
            .append(Doc::line())
            .append(Doc::text("x"))
            .group();
        assert_eq!(emoji_tab.render(4), "\u{1F600}\t x");
        assert_eq!(emoji_tab.render(3), "\u{1F600}\t\nx");
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
