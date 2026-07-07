//! A compact JSON pretty-printer built on pretty_lang.
//!
//! This is the classic demonstration of a Wadler-style printer: a single
//! `to_doc` walk over a value tree produces a document that renders as dense
//! one-liners where they fit and as indented, one-member-per-line blocks where
//! they do not — with no width logic in the walk itself. The same document is
//! rendered at three widths below to show the reflow.
//!
//! Run with:
//!
//! ```text
//! cargo run --example json
//! ```

use pretty_lang::Doc;

/// A minimal JSON value, enough to show object/array reflow.
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Lower a JSON value to a layout document.
///
/// Objects and arrays wrap their members in a `group` with a `softline` just
/// inside each bracket and a `line` between members, all `nest`ed by two. That
/// single pattern is what gives the "flat if it fits, block if it doesn't"
/// behaviour for free at every level of nesting.
fn to_doc(value: &Json) -> Doc {
    match value {
        Json::Null => Doc::text("null"),
        Json::Bool(true) => Doc::text("true"),
        Json::Bool(false) => Doc::text("false"),
        Json::Number(n) => Doc::text(n.to_string()),
        Json::Str(s) => Doc::text(format!("{s:?}")),
        Json::Array(items) => {
            if items.is_empty() {
                return Doc::text("[]");
            }
            let members = Doc::join(Doc::text(",").append(Doc::line()), items.iter().map(to_doc));
            bracket("[", members, "]")
        }
        Json::Object(members) => {
            if members.is_empty() {
                return Doc::text("{}");
            }
            let members = Doc::join(
                Doc::text(",").append(Doc::line()),
                members.iter().map(|(key, val)| {
                    Doc::text(format!("{key:?}"))
                        .append(Doc::text(": "))
                        .append(to_doc(val))
                }),
            );
            bracket("{", members, "}")
        }
    }
}

/// Wrap `body` between `open` and `close`, indented, so it collapses to
/// `open body close` when it fits and becomes a multi-line block otherwise.
fn bracket(open: &'static str, body: Doc, close: &'static str) -> Doc {
    Doc::text(open)
        .append(Doc::softline().append(body).nest(2))
        .append(Doc::softline())
        .append(Doc::text(close))
        .group()
}

fn main() {
    let value = Json::Object(vec![
        ("name".into(), Json::Str("pretty-lang".into())),
        ("version".into(), Json::Str("0.2.0".into())),
        ("stable".into(), Json::Bool(false)),
        (
            "keywords".into(),
            Json::Array(vec![
                Json::Str("formatter".into()),
                Json::Str("pretty".into()),
                Json::Str("compiler".into()),
            ]),
        ),
        (
            "limits".into(),
            Json::Object(vec![
                ("min".into(), Json::Number(0.0)),
                ("max".into(), Json::Null),
            ]),
        ),
    ]);

    let doc = to_doc(&value);

    for width in [120, 40, 20] {
        println!("── width {width} {}", "─".repeat(30));
        println!("{}\n", doc.render(width));
    }
}
