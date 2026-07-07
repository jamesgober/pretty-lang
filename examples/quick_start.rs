//! The shortest end-to-end use of pretty_lang: build one document, render it at
//! two widths, and watch it reflow.
//!
//! Run with:
//!
//! ```text
//! cargo run --example quick_start
//! ```

use pretty_lang::Doc;

fn main() {
    // A function call whose arguments should sit on one line when they fit and
    // stack one-per-line, indented, when they do not.
    let call = Doc::text("plot(")
        .append(
            Doc::softline()
                .append(Doc::join(
                    Doc::text(",").append(Doc::line()),
                    ["x", "y", "color", "label"].map(Doc::text),
                ))
                .nest(4),
        )
        .append(Doc::softline())
        .append(Doc::text(")"))
        .group();

    println!("At width 80:\n{}\n", call.render(80));
    println!("At width 12:\n{}", call.render(12));
}
