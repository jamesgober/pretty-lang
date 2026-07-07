//! The layout engine: turn a [`Doc`] tree into laid-out text for a target
//! width.
//!
//! The algorithm is Wadler/Lindig's linear-time pretty-printer. Two passes
//! cooperate:
//!
//! * [`layout`] walks the document with an explicit work stack, emitting text
//!   and resolving each [`group`](crate::Doc::group) to *flat* or *broken*.
//! * [`fits`] is the bounded look-ahead that answers "does the rest of this line
//!   fit?" — it scans only as far as the first newline or the first column past
//!   `width`, which is what keeps the whole thing linear.
//!
//! Neither pass recurses on the document, so arbitrarily deep documents render
//! without risking a stack overflow.

use alloc::vec::Vec;
use core::fmt::Write;

use crate::doc::{Doc, Node};

/// Whether a group is being laid out on one line (`Flat`) or broken across
/// several (`Break`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// One pending piece of work: render `doc` at indentation `indent` in `mode`.
/// The engine borrows nodes rather than cloning `Doc` handles on the hot path.
struct Frame<'a> {
    indent: isize,
    mode: Mode,
    node: &'a Node,
}

/// Lay `root` out to `width` columns, writing the result into `out`.
///
/// Returns `out`'s error unchanged if it ever fails mid-write; against an
/// infallible sink (such as `String`) it always returns `Ok`.
pub(crate) fn layout<W: Write>(root: &Doc, width: usize, out: &mut W) -> core::fmt::Result {
    let width = width as isize;
    // Current column, i.e. how many columns of the current line are used.
    let mut col: isize = 0;
    // The work stack, processed top (last) first. The root starts in Break
    // mode: with no enclosing group, every flexible break takes its broken form
    // unless a group later flattens it.
    let mut stack: Vec<Frame<'_>> = Vec::with_capacity(16);
    stack.push(Frame {
        indent: 0,
        mode: Mode::Break,
        node: &root.0,
    });

    while let Some(Frame { indent, mode, node }) = stack.pop() {
        match node {
            Node::Nil => {}
            Node::Text(s, w) => {
                out.write_str(s)?;
                col = col.saturating_add(*w as isize);
            }
            Node::Cat(a, b) => {
                // Push right first so the left child is processed next.
                stack.push(Frame {
                    indent,
                    mode,
                    node: &b.0,
                });
                stack.push(Frame {
                    indent,
                    mode,
                    node: &a.0,
                });
            }
            Node::Nest(j, x) => stack.push(Frame {
                indent: indent.saturating_add(*j),
                mode,
                node: &x.0,
            }),
            Node::Line => match mode {
                Mode::Flat => {
                    out.write_str(" ")?;
                    col = col.saturating_add(1);
                }
                Mode::Break => col = new_line(out, indent)?,
            },
            Node::SoftLine => match mode {
                Mode::Flat => {}
                Mode::Break => col = new_line(out, indent)?,
            },
            // A hardline is always a newline. It reaches here only in Break
            // mode, because `fits` reports any hardline as not fitting, so every
            // enclosing group is forced to break before we get here.
            Node::HardLine => col = new_line(out, indent)?,
            Node::Group(x) => {
                let mode = if fits(width - col, indent, &x.0, &stack) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                stack.push(Frame {
                    indent,
                    mode,
                    node: &x.0,
                });
            }
        }
    }
    Ok(())
}

/// Emit a newline followed by `indent` (clamped at zero) spaces, and return the
/// new column, which equals the indentation.
#[inline]
fn new_line<W: Write>(out: &mut W, indent: isize) -> Result<isize, core::fmt::Error> {
    out.write_str("\n")?;
    let indent = indent.max(0);
    write_spaces(out, indent as usize)?;
    Ok(indent)
}

/// Write `n` spaces, in chunks, without allocating.
#[inline]
fn write_spaces<W: Write>(out: &mut W, mut n: usize) -> core::fmt::Result {
    const SPACES: &str = "                                ";
    while n > 0 {
        let take = n.min(SPACES.len());
        out.write_str(&SPACES[..take])?;
        n -= take;
    }
    Ok(())
}

/// Does the document fit flat in `avail` columns, followed by the already-queued
/// continuation on `stack`?
///
/// Look-ahead stops the moment the answer is known: the width is exhausted
/// (returns `false`) or a line-ending break is reached (returns `true`). A
/// [`Node::HardLine`] in flat context can never fit, which is exactly how a
/// hardline forces its enclosing groups to break. Nested groups are assumed
/// flat here, the standard Wadler approximation that keeps the scan linear.
fn fits(avail: isize, indent: isize, group: &Node, stack: &[Frame<'_>]) -> bool {
    if avail < 0 {
        return false;
    }
    let mut remaining = avail;
    // A small local stack for the group's own contents, expanded flat. When it
    // drains, we continue into the queued continuation from the top of `stack`.
    let mut local: Vec<(isize, Mode, &Node)> = Vec::new();
    local.push((indent, Mode::Flat, group));
    let mut cont = stack.len();

    loop {
        let (i, mode, node) = match local.pop() {
            Some(item) => item,
            None => {
                if cont == 0 {
                    return true;
                }
                cont -= 1;
                let frame = &stack[cont];
                (frame.indent, frame.mode, frame.node)
            }
        };

        match node {
            Node::Nil => {}
            Node::Text(_, w) => {
                remaining -= *w as isize;
                if remaining < 0 {
                    return false;
                }
            }
            Node::Cat(a, b) => {
                local.push((i, mode, &b.0));
                local.push((i, mode, &a.0));
            }
            Node::Nest(j, x) => local.push((i.saturating_add(*j), mode, &x.0)),
            // In flat mode a `Line` is a space and a `SoftLine` is nothing; in
            // break mode either one ends the current line, so the rest fits.
            Node::Line => match mode {
                Mode::Flat => {
                    remaining -= 1;
                    if remaining < 0 {
                        return false;
                    }
                }
                Mode::Break => return true,
            },
            Node::SoftLine => {
                if mode == Mode::Break {
                    return true;
                }
            }
            Node::HardLine => match mode {
                Mode::Flat => return false,
                Mode::Break => return true,
            },
            Node::Group(x) => local.push((i, Mode::Flat, &x.0)),
        }
    }
}

/// `std::io::Write` counterpart of [`layout`]: render `root` at `width` into an
/// I/O sink, propagating the first I/O error.
#[cfg(feature = "std")]
pub(crate) fn layout_io<W: std::io::Write>(
    root: &Doc,
    width: usize,
    out: &mut W,
) -> std::io::Result<()> {
    // Adapt the io sink to `core::fmt::Write`, stashing any io error so the real
    // cause survives (fmt::Error carries no payload).
    struct Adapter<'w, W: std::io::Write> {
        inner: &'w mut W,
        err: Option<std::io::Error>,
    }
    impl<W: std::io::Write> Write for Adapter<'_, W> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            match self.inner.write_all(s.as_bytes()) {
                Ok(()) => Ok(()),
                Err(e) => {
                    self.err = Some(e);
                    Err(core::fmt::Error)
                }
            }
        }
    }

    let mut adapter = Adapter {
        inner: out,
        err: None,
    };
    match layout(root, width, &mut adapter) {
        Ok(()) => Ok(()),
        Err(_) => Err(adapter
            .err
            .unwrap_or_else(|| std::io::Error::other("formatting error"))),
    }
}
