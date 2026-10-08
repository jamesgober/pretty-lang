//! Criterion benchmarks for the layout engine.
//!
//! Two workloads stand in for real formatting: a deep, JSON-like tree of nested
//! objects and arrays, and a wide function call with many arguments. Each is
//! measured both where it fits flat (the no-break path) and where it must break
//! (the newline-and-indent path), plus the cost of building the document
//! separately from rendering it.
//!
//! A third group, `pathological`, renders documents made almost entirely of
//! zero-width content (`softline` groups, `nil` groups, empty-text groups) at
//! 10k–1M items. These made the 1.0.0 look-ahead quadratic (ISSUES M65); since
//! 1.0.1 they render in one linear pass.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pretty_lang::Doc;

/// Target width for the "fits on one line" path. Any width at or above
/// `isize::MAX` is unlimited; before 1.0.1 `usize::MAX` wrapped to `-1` and
/// broke every group (ISSUES M64), so these benches measured the broken
/// layout. Each flat bench now asserts its output really is one line.
const FLAT: usize = usize::MAX;

/// Build a JSON-like document `depth` levels deep with `width` entries per
/// object. Each object is `{ "kN": <value>, ... }`, values alternate between a
/// nested object and a short scalar.
fn json_doc(depth: usize, width: usize) -> Doc {
    if depth == 0 {
        return Doc::text("42");
    }
    let entries = (0..width).map(|i| {
        let key = Doc::text(format!("\"k{i}\""));
        let value = if i % 2 == 0 {
            json_doc(depth - 1, width)
        } else {
            Doc::text("42")
        };
        key.append(Doc::text(": ")).append(value)
    });

    let inner = Doc::softline()
        .append(Doc::join(Doc::text(",").append(Doc::line()), entries))
        .nest(2);

    Doc::text("{")
        .append(inner)
        .append(Doc::softline())
        .append(Doc::text("}"))
        .group()
}

/// Build a `f(arg0, arg1, ...)` call with `n` arguments.
fn call_doc(n: usize) -> Doc {
    let args = (0..n).map(|i| Doc::text(format!("argument_{i}")));
    Doc::text("call(")
        .append(
            Doc::softline()
                .append(Doc::join(Doc::text(",").append(Doc::line()), args))
                .nest(4),
        )
        .append(Doc::softline())
        .append(Doc::text(")"))
        .group()
}

/// Render at [`FLAT`], refusing to benchmark if the result is not one line.
fn assert_flat(doc: &Doc) -> String {
    let flat = doc.render(FLAT);
    assert!(
        !flat.contains('\n'),
        "the flat bench must measure a flat layout"
    );
    flat
}

fn bench_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("json");
    for &(depth, width) in &[(3usize, 4usize), (4, 4), (5, 3)] {
        let doc = json_doc(depth, width);
        let flat = assert_flat(&doc);
        let bytes = flat.len() as u64;
        group.throughput(Throughput::Bytes(bytes));

        // Fits on one line: the no-break path.
        group.bench_with_input(
            BenchmarkId::new("render_flat", format!("d{depth}xw{width}")),
            &doc,
            |b, doc| b.iter(|| black_box(doc.render(black_box(FLAT)))),
        );

        // Forced to break at every level.
        group.bench_with_input(
            BenchmarkId::new("render_broken", format!("d{depth}xw{width}")),
            &doc,
            |b, doc| b.iter(|| black_box(doc.render(black_box(40)))),
        );

        // Build the document from scratch, then render broken.
        group.bench_with_input(
            BenchmarkId::new("build_and_render", format!("d{depth}xw{width}")),
            &(depth, width),
            |b, &(depth, width)| b.iter(|| black_box(json_doc(depth, width).render(black_box(40)))),
        );
    }
    group.finish();
}

fn bench_call(c: &mut Criterion) {
    let mut group = c.benchmark_group("call");
    for &n in &[8usize, 32, 128] {
        let doc = call_doc(n);
        let _ = assert_flat(&doc);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("render_flat", n), &doc, |b, doc| {
            b.iter(|| black_box(doc.render(black_box(FLAT))))
        });
        group.bench_with_input(BenchmarkId::new("render_broken", n), &doc, |b, doc| {
            b.iter(|| black_box(doc.render(black_box(20))))
        });
    }
    group.finish();
}

/// `n` top-level `group(softline)` items: every group's continuation is the
/// whole (zero-width) rest of the document.
fn softline_groups(n: usize) -> Doc {
    Doc::concat((0..n).map(|_| Doc::softline().group()))
}

/// `n` `group(nil)` items inside one outer group.
fn nil_groups(n: usize) -> Doc {
    Doc::concat((0..n).map(|_| Doc::nil().group())).group()
}

/// `n` `group(text(""))` items inside one outer group.
fn empty_text_groups(n: usize) -> Doc {
    Doc::concat((0..n).map(|_| Doc::text("").group())).group()
}

/// A named document builder for one pathological shape.
type Shape = (&'static str, fn(usize) -> Doc);

fn bench_pathological(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathological");
    group.sample_size(10);
    let shapes: [Shape; 3] = [
        ("softline_groups", softline_groups),
        ("nil_groups", nil_groups),
        ("empty_text_groups", empty_text_groups),
    ];
    for (name, build) in shapes {
        for &n in &[10_000usize, 100_000, 1_000_000] {
            let doc = build(n);
            group.throughput(Throughput::Elements(n as u64));
            group.bench_with_input(BenchmarkId::new(name, n), &doc, |b, doc| {
                b.iter(|| black_box(doc.render(black_box(80))))
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_json, bench_call, bench_pathological);
criterion_main!(benches);
