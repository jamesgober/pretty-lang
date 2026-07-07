//! Format a Rust function signature that reflows its parameter list.
//!
//! This mirrors what a real formatter does with a long signature: keep it on
//! one line while it fits, then break the parameters one-per-line and indent
//! them under the opening parenthesis. The `where` clause is attached with a
//! `line` so it, too, drops onto its own line only when needed.
//!
//! Run with:
//!
//! ```text
//! cargo run --example rust_signature
//! ```

use pretty_lang::Doc;

/// A parameter, e.g. `name: Type`.
struct Param {
    name: &'static str,
    ty: &'static str,
}

/// Build the document for `fn <name>(<params>) -> <ret>`.
fn signature(name: &'static str, params: &[Param], ret: &'static str) -> Doc {
    let params = Doc::join(
        Doc::text(",").append(Doc::line()),
        params.iter().map(|p| {
            Doc::text(p.name)
                .append(Doc::text(": "))
                .append(Doc::text(p.ty))
        }),
    );

    let param_list = Doc::text("(")
        .append(Doc::softline().append(params).nest(4))
        .append(Doc::softline())
        .append(Doc::text(")"))
        .group();

    Doc::text("fn ")
        .append(Doc::text(name))
        .append(param_list)
        .append(Doc::text(" -> "))
        .append(Doc::text(ret))
}

fn main() {
    let doc = signature(
        "render",
        &[
            Param {
                name: "self",
                ty: "&Doc",
            },
            Param {
                name: "width",
                ty: "usize",
            },
            Param {
                name: "out",
                ty: "&mut impl Write",
            },
        ],
        "fmt::Result",
    );

    println!("At width 100:\n{}\n", doc.render(100));
    println!("At width 30:\n{}", doc.render(30));
}
