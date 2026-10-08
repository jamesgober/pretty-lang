//! The layout engine: turn a [`Doc`] tree into laid-out text for a target
//! width.
//!
//! The algorithm is Wadler/Lindig's pretty-printer: [`layout`] walks the
//! document with an explicit work stack, emitting text and resolving each
//! [`group`](crate::Doc::group) to *flat* or *broken*. A group is flat when its
//! contents, laid out flat, plus whatever follows on the same line, fit in the
//! columns left on the current line.
//!
//! That fit test is O(1). Every node carries a [`Fit`] summary computed when it
//! was built, and every frame on the work stack carries `rest`: the columns
//! that frame and everything queued after it will use before the current line
//! ends. A group fits exactly when its flat width plus the `rest` of the frame
//! below it is within the columns left. Rendering is therefore one linear pass
//! with no look-ahead scan at all.
//!
//! The decisions are identical to the classic bounded scan (which walked the
//! continuation until a line break or until the width ran out). That scan was
//! quadratic whenever the continuation was zero-width, because nothing ever
//! exhausted the width (ISSUES M65); the summaries compute the same answer
//! without walking. `tests/equivalence.rs` checks the two against each other
//! on random documents.
//!
//! Nothing here recurses on the document, so arbitrarily deep documents render
//! without risking a stack overflow, and the render path allocates only the
//! work stack.

use alloc::vec::Vec;
use core::fmt::Write;

use crate::doc::{Doc, Fit, NO_BREAK, Node};

/// Whether a group is being laid out on one line (`Flat`) or broken across
/// several (`Break`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// One pending piece of work: render `node` at indentation `indent` in `mode`.
/// The engine borrows nodes rather than cloning `Doc` handles on the hot path.
struct Frame<'a> {
    indent: isize,
    mode: Mode,
    node: &'a Node,
    /// Columns used from the start of this frame, through every frame below
    /// it on the stack (which are laid out after it), up to the first line
    /// break that ends the current line, or to the end of the document.
    /// Saturates at [`crate::doc::INF`], which never fits.
    rest: usize,
}

/// Columns a node laid out in `mode` uses before the line ends, given that
/// `below` columns follow it up to the line's end.
#[inline]
fn rest_width(fit: Fit, mode: Mode, below: usize) -> usize {
    match mode {
        // Flat mode never ends a line (a flat hardline is `INF` wide), so the
        // following content is always reached.
        Mode::Flat => fit.flat.saturating_add(below),
        Mode::Break if fit.brk == NO_BREAK => fit.flat.saturating_add(below),
        // The node ends the line itself; what follows is on a later line.
        Mode::Break => fit.brk,
    }
}

/// Push a frame, computing its `rest` from the frame it lands on.
#[inline]
fn push<'a>(stack: &mut Vec<Frame<'a>>, indent: isize, mode: Mode, node: &'a Node) {
    let below = stack.last().map_or(0, |f| f.rest);
    stack.push(Frame {
        indent,
        mode,
        node,
        rest: rest_width(node.fit(), mode, below),
    });
}

/// Clamp a caller's target width into the engine's signed column space.
///
/// Columns are `isize` because indentation can be negative. Any width above
/// `isize::MAX` already exceeds every reachable column count, so clamping
/// changes nothing except that such widths no longer wrap to a negative value
/// (ISSUES M64: `width as isize` turned `usize::MAX` into `-1`, which broke
/// every group).
#[inline]
fn clamp_width(width: usize) -> isize {
    isize::try_from(width).unwrap_or(isize::MAX)
}

/// Lay `root` out to `width` columns, writing the result into `out`.
///
/// Returns `out`'s error unchanged if it ever fails mid-write; against an
/// infallible sink (such as `String`) it always returns `Ok`.
pub(crate) fn layout<W: Write>(root: &Doc, width: usize, out: &mut W) -> core::fmt::Result {
    let width = clamp_width(width);
    // Current column, i.e. how many columns of the current line are used.
    // Never negative: it starts at 0, only grows, and a newline resets it to
    // a clamped-at-zero indentation.
    let mut col: isize = 0;
    // The work stack, processed top (last) first. The root starts in Break
    // mode: with no enclosing group, every flexible break takes its broken form
    // unless a group later flattens it.
    let mut stack: Vec<Frame<'_>> = Vec::with_capacity(16);
    push(&mut stack, 0, Mode::Break, &root.0);

    while let Some(Frame {
        indent, mode, node, ..
    }) = stack.pop()
    {
        match node {
            Node::Nil => {}
            Node::Text(s, w) => {
                out.write_str(s)?;
                col = col.saturating_add(isize::try_from(*w).unwrap_or(isize::MAX));
            }
            Node::Cat(a, b, _) => {
                // Push right first so the left child is processed next.
                push(&mut stack, indent, mode, &b.0);
                push(&mut stack, indent, mode, &a.0);
            }
            Node::Nest(j, x, _) => push(&mut stack, indent.saturating_add(*j), mode, &x.0),
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
            // mode, because a hardline makes its group's flat width `INF`,
            // which never fits, so every enclosing group breaks first.
            Node::HardLine => col = new_line(out, indent)?,
            Node::Group(x, flat) => {
                // `width - col` cannot overflow: width is in [0, isize::MAX]
                // and col is never negative.
                let below = stack.last().map_or(0, |f| f.rest);
                let mode = if fits(width - col, *flat, below) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                push(&mut stack, indent, mode, &x.0);
            }
        }
    }
    Ok(())
}

/// Does a group whose contents are `flat` columns wide when laid out flat fit
/// in `avail` columns, given that `below` more columns follow it before the
/// current line ends?
///
/// This is the whole look-ahead. It answers exactly what a scan of the group
/// (flat) and then the queued continuation would: the scan stops at the first
/// line break in a broken frame (fits if the width held out), fails on a
/// hardline in flat context (`INF` here), and otherwise fails as soon as the
/// running width exceeds `avail`. Widths are non-negative, so "never exceeded
/// along the way" is the same as "the total up to the stop is within
/// `avail`".
#[inline]
fn fits(avail: isize, flat: usize, below: usize) -> bool {
    match usize::try_from(avail) {
        Ok(avail) => flat.saturating_add(below) <= avail,
        // The line is already over-full.
        Err(_) => false,
    }
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
