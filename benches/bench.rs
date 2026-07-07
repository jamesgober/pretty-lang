//! Criterion benchmarks for the layout engine.
//!
//! Two workloads stand in for real formatting: a deep, JSON-like tree of nested
//! objects and arrays, and a wide function call with many arguments. Each is
//! measured both where it fits flat (the fast, no-break path) and where it must
//! break (the newline-and-indent path), plus the cost of building the document
//! separately from rendering it.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pretty_lang::Doc;

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

fn bench_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("json");
    for &(depth, width) in &[(3usize, 4usize), (4, 4), (5, 3)] {
        let doc = json_doc(depth, width);
        let flat = doc.render(usize::MAX);
        let bytes = flat.len() as u64;
        group.throughput(Throughput::Bytes(bytes));

        // Fits on one line: the fast, no-break path.
        group.bench_with_input(
            BenchmarkId::new("render_flat", format!("d{depth}xw{width}")),
            &doc,
            |b, doc| b.iter(|| black_box(doc.render(black_box(usize::MAX)))),
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
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("render_flat", n), &doc, |b, doc| {
            b.iter(|| black_box(doc.render(black_box(usize::MAX))))
        });
        group.bench_with_input(BenchmarkId::new("render_broken", n), &doc, |b, doc| {
            b.iter(|| black_box(doc.render(black_box(20))))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_json, bench_call);
criterion_main!(benches);
