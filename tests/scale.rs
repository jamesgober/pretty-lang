//! Scale tests: zero-width-heavy documents at one million items.
//!
//! 1.0.0's look-ahead rescanned the whole zero-width continuation at every
//! group, so these shapes were quadratic: ~50 s at 100k items in a release
//! build, far longer at 1M (ISSUES M65). Since 1.0.1 the fit test is O(1) and
//! each of these completes in a fraction of a second even unoptimized. A
//! quadratic regression would make this file hang rather than pass.

use pretty_lang::Doc;

const N: usize = 1_000_000;

#[test]
fn test_one_million_softline_groups() {
    let doc = Doc::concat((0..N).map(|_| Doc::softline().group()));
    assert_eq!(doc.render(80), "");
}

#[test]
fn test_one_million_nil_groups_in_a_group() {
    let doc = Doc::concat((0..N).map(|_| Doc::nil().group())).group();
    assert_eq!(doc.render(80), "");
}

#[test]
fn test_one_million_empty_text_groups_flat_at_unlimited_width() {
    // Every item is an empty group plus a `line`, so the flat form is N
    // spaces: it fits at an unlimited width (M64) and must be found linearly.
    let doc = Doc::concat((0..N).map(|_| Doc::text("").group().append(Doc::line()))).group();
    let out = doc.render(usize::MAX);
    assert_eq!(out.len(), N);
    assert!(out.bytes().all(|b| b == b' '));
}

#[test]
fn test_one_million_groups_break_correctly_at_narrow_width() {
    // The same shape at width 80 cannot fit flat: the outer group breaks and
    // every `line` becomes a newline.
    let doc = Doc::concat((0..N).map(|_| Doc::text("").group().append(Doc::line()))).group();
    let out = doc.render(80);
    assert_eq!(out.len(), N);
    assert!(out.bytes().all(|b| b == b'\n'));
}
