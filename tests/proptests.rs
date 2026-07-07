//! Property-based tests for the layout engine.
//!
//! These exercise the invariants that must hold for *every* document across a
//! wide input space, using an independently-built oracle for the flat layout so
//! the check does not just re-run the code under test.

#![allow(clippy::unwrap_used)]

use pretty_lang::Doc;
use proptest::prelude::*;

/// A width wider than any generated document's flat form, so a fully grouped
/// document is guaranteed to lay out flat at this width.
const WIDE: usize = 1_000_000;

/// Generate a document together with its known flat rendering.
///
/// Building the expected flat string alongside the document gives the flat-path
/// properties a real oracle: the string is assembled from the leaves directly,
/// not by rendering. `hardline` is deliberately excluded here — it would break
/// even a fully grouped, arbitrarily wide layout, which is the opposite of what
/// the flat-form properties assert.
fn arb_doc_and_flat() -> impl Strategy<Value = (Doc, String)> {
    let leaf = prop_oneof![
        Just((Doc::nil(), String::new())),
        "[a-z0-9]{0,6}".prop_map(|s| (Doc::text(s.clone()), s)),
        Just((Doc::line(), String::from(" "))),
        Just((Doc::softline(), String::new())),
    ];

    leaf.prop_recursive(6, 128, 4, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|((a, fa), (b, fb))| (a.append(b), fa + &fb)),
            (-4isize..8, inner.clone()).prop_map(|(n, (d, f))| (d.nest(n), f)),
            inner.prop_map(|(d, f)| (d.group(), f)),
        ]
    })
}

/// A separate generator that *may* include hardlines, for the panic-safety and
/// consistency properties that must survive forced breaks too.
fn arb_doc() -> impl Strategy<Value = Doc> {
    let leaf = prop_oneof![
        Just(Doc::nil()),
        "[a-z0-9]{0,6}".prop_map(Doc::text),
        Just(Doc::line()),
        Just(Doc::softline()),
        Just(Doc::hardline()),
    ];

    leaf.prop_recursive(6, 128, 4, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(a, b)| a.append(b)),
            (-4isize..8, inner.clone()).prop_map(|(n, d)| d.nest(n)),
            inner.prop_map(Doc::group),
        ]
    })
}

proptest! {
    /// A fully grouped document with no hardlines lays out flat at a wide width,
    /// matching the independently-built flat string exactly.
    #[test]
    fn prop_grouped_wide_equals_flat_oracle((doc, flat) in arb_doc_and_flat()) {
        prop_assert_eq!(doc.group().render(WIDE), flat);
    }

    /// The flat layout of a hardline-free document contains no newlines.
    #[test]
    fn prop_grouped_wide_has_no_newline((doc, _flat) in arb_doc_and_flat()) {
        prop_assert!(!doc.group().render(WIDE).contains('\n'));
    }

    /// Rendering never panics for any document at any width, including width 0
    /// and documents full of forced breaks.
    #[test]
    fn prop_render_never_panics(doc in arb_doc(), width in 0usize..120) {
        let _ = doc.render(width);
    }

    /// `render_into` produces exactly what `render` returns, for every document
    /// and width.
    #[test]
    fn prop_render_into_matches_render(doc in arb_doc(), width in 0usize..120) {
        let mut buf = String::new();
        doc.render_into(width, &mut buf).unwrap();
        prop_assert_eq!(buf, doc.render(width));
    }

    /// Grouping is idempotent: wrapping an already-grouped document in another
    /// group changes nothing, because the outer group's fit decision is settled
    /// identically by the inner one.
    #[test]
    fn prop_group_is_idempotent(doc in arb_doc(), width in 0usize..80) {
        let once = doc.clone().group().render(width);
        let twice = doc.group().group().render(width);
        prop_assert_eq!(once, twice);
    }

    /// Concatenation is associative under rendering: how the appends are nested
    /// does not change the output.
    #[test]
    fn prop_append_is_associative(
        a in arb_doc(), b in arb_doc(), c in arb_doc(), width in 0usize..80
    ) {
        let left = a.clone().append(b.clone()).append(c.clone()).render(width);
        let right = a.append(b.append(c)).render(width);
        prop_assert_eq!(left, right);
    }
}
