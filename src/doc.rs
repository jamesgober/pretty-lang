//! The [`Doc`] document algebra: a small set of combinators for describing how
//! source text should be laid out, independent of any concrete width.
//!
//! A [`Doc`] is a lazy description, not a string. You build it from an AST with
//! [`text`](Doc::text), [`line`](Doc::line), [`nest`](Doc::nest),
//! [`group`](Doc::group), and [`append`](Doc::append); the concrete layout is
//! decided later by [`render`](Doc::render) against a target width. The design
//! follows Wadler's *A Prettier Printer* and Lindig's *Strictly Pretty*.

use alloc::borrow::Cow;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

/// An immutable, cheaply-clonable description of a document's layout.
///
/// A `Doc` records *intent* — "these pieces belong together", "break here if the
/// line is too long", "indent the inside by four" — and leaves the choice of
/// concrete line breaks to [`render`](Doc::render), which fits the document to a
/// target width. The same `Doc` renders differently at width 40 and width 120
/// with no change to how it was built.
///
/// # Cloning
///
/// `Doc` is a thin handle around a reference-counted node ([`Rc`]), so
/// [`Clone`] is a pointer-count bump, not a deep copy. Sharing a sub-document in
/// several places costs one `Rc` clone each. `Doc` is single-threaded by design
/// (it is not `Send`/`Sync`); a formatter builds and renders on one thread.
///
/// # Examples
///
/// ```
/// use pretty_lang::Doc;
///
/// // `[1, 2, 3]` — flat because it fits.
/// let list = Doc::text("[")
///     .append(Doc::join(
///         Doc::text(",").append(Doc::line()),
///         ["1", "2", "3"].map(Doc::text),
///     ))
///     .append(Doc::text("]"))
///     .group();
///
/// assert_eq!(list.render(80), "[1, 2, 3]");
/// ```
#[derive(Clone)]
pub struct Doc(pub(crate) Rc<Node>);

/// The internal node kinds behind a [`Doc`]. Kept `pub(crate)`: the public
/// surface is the combinator API on [`Doc`], never the node shape.
pub(crate) enum Node {
    /// The empty document. Renders to nothing.
    Nil,
    /// Literal text with its precomputed width, counted as
    /// `str::chars().count()` (Unicode scalar values).
    /// The text MUST NOT contain a newline; use [`Doc::hardline`] for those.
    Text(Cow<'static, str>, usize),
    /// A space when the enclosing group is flat, a newline when it is broken.
    Line,
    /// Nothing when the enclosing group is flat, a newline when it is broken.
    SoftLine,
    /// Always a newline; forces every enclosing group to break.
    HardLine,
    /// Concatenation of two documents, laid out left then right, with the
    /// pair's combined [`Fit`] summary.
    Cat(Doc, Doc, Fit),
    /// Adds `isize` columns of indentation to line breaks inside `Doc`. The
    /// [`Fit`] is the child's (indentation never affects a fit decision).
    Nest(isize, Doc, Fit),
    /// A layout choice point: render the inside flat if it fits the remaining
    /// width on the current line, otherwise break every flexible line in it.
    /// The `usize` is the inside's flat width ([`Fit::flat`]).
    Group(Doc, usize),
}

/// Width standing for "wider than any target": the flat width of a hardline,
/// and the value every width sum saturates at.
///
/// The renderer clamps its target width to `isize::MAX`, which is strictly
/// below this, so a saturated or hardline-bearing width never fits. Treating
/// "contains a hardline in flat mode" as infinitely wide is exact: a flat
/// hardline makes the look-ahead fail no matter what surrounds it, and so does
/// an infinite width.
pub(crate) const INF: usize = usize::MAX;

/// Sentinel for [`Fit::brk`]: in break mode the node ends no line itself.
///
/// It shares its value with [`INF`] on purpose. A node whose break-mode
/// prefix saturated to `INF` also has `flat == INF` (the prefix is part of
/// the flat form, measured identically), and an `INF` width never fits
/// whether or not a line break follows it, so the two readings agree.
pub(crate) const NO_BREAK: usize = usize::MAX;

/// Look-ahead summary of a subtree, computed once when the node is built.
///
/// The renderer's fit test asks: laid out from here, how many columns are
/// used before the current line ends? For a subtree that answer depends only
/// on the mode it is laid out in, so it is cached here and composed in O(1)
/// per node. This is what makes the fit test constant-time instead of a scan
/// over the rest of the line, which was quadratic on zero-width content
/// (ISSUES M65). Indentation is not tracked because it never affects a fit:
/// the look-ahead only counts columns up to the first line break.
#[derive(Clone, Copy)]
pub(crate) struct Fit {
    /// Columns used when the subtree is laid out flat: text widths, one per
    /// `line`, zero per `softline`, and [`INF`] if it contains a hardline.
    /// Saturates at [`INF`].
    pub(crate) flat: usize,
    /// Columns used in break mode before the first line break the subtree
    /// itself owns (a `line`, `softline`, or `hardline` not inside a nested
    /// group), or [`NO_BREAK`] if it owns none. Nested groups count as flat.
    /// Whenever this is a real width, it is `<= flat`.
    pub(crate) brk: usize,
}

impl Fit {
    /// Summary of a sequence: `a` laid out, then `b`.
    #[inline]
    const fn cat(a: Fit, b: Fit) -> Fit {
        let brk = if a.brk != NO_BREAK {
            // `a` ends the line itself; nothing in `b` is reached.
            a.brk
        } else if b.brk != NO_BREAK {
            // In break mode `a` passed through unchanged, so it was laid out
            // exactly as in flat mode.
            a.flat.saturating_add(b.brk)
        } else {
            NO_BREAK
        };
        Fit {
            flat: a.flat.saturating_add(b.flat),
            brk,
        }
    }
}

impl Node {
    /// This node's look-ahead summary, in O(1): leaves are constants and
    /// internal nodes carry theirs.
    #[inline]
    pub(crate) fn fit(&self) -> Fit {
        match self {
            Node::Nil => Fit {
                flat: 0,
                brk: NO_BREAK,
            },
            Node::Text(_, w) => Fit {
                flat: *w,
                brk: NO_BREAK,
            },
            Node::Line => Fit { flat: 1, brk: 0 },
            Node::SoftLine => Fit { flat: 0, brk: 0 },
            Node::HardLine => Fit { flat: INF, brk: 0 },
            Node::Cat(_, _, fit) | Node::Nest(_, _, fit) => *fit,
            // A group seen from outside is always measured flat: the
            // look-ahead assumes nested groups stay flat (Wadler's rule).
            Node::Group(_, flat) => Fit {
                flat: *flat,
                brk: NO_BREAK,
            },
        }
    }
}

impl Doc {
    /// The empty document. It renders to nothing and is the identity for
    /// [`append`](Doc::append).
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// assert_eq!(Doc::nil().render(80), "");
    /// assert_eq!(Doc::text("x").append(Doc::nil()).render(80), "x");
    /// ```
    #[inline]
    #[must_use]
    pub fn nil() -> Doc {
        Doc(Rc::new(Node::Nil))
    }

    /// A literal piece of text.
    ///
    /// The argument is anything that converts into a `Cow<'static, str>`, so a
    /// string literal is stored without allocating and an owned `String` is
    /// moved in. The width is measured once, here, as `s.chars().count()`: the
    /// number of Unicode scalar values. It is not the byte length and not the
    /// terminal display width: a CJK character or an emoji (two cells in a
    /// terminal) counts as one, a combining mark (zero cells) counts as one,
    /// and a tab counts as one. If you need display-accurate layout, measure
    /// and pad your text yourself.
    ///
    /// # Panics
    ///
    /// Never panics. The text is treated as a single unbreakable unit; it MUST
    /// NOT contain a `'\n'` (embed line breaks with [`line`](Doc::line),
    /// [`softline`](Doc::softline), or [`hardline`](Doc::hardline) so the layout
    /// engine can account for them). A newline inside `text` is rendered
    /// verbatim but throws the width accounting off.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// // A static literal — no allocation.
    /// assert_eq!(Doc::text("let x").render(80), "let x");
    ///
    /// // An owned, computed string.
    /// let name = format!("v{}", 42);
    /// assert_eq!(Doc::text(name).render(80), "v42");
    /// ```
    #[inline]
    #[must_use]
    pub fn text(s: impl Into<Cow<'static, str>>) -> Doc {
        let s = s.into();
        let width = s.chars().count();
        Doc(Rc::new(Node::Text(s, width)))
    }

    /// A flexible break that is a single space when its group is laid out flat
    /// and a newline (plus the current indentation) when the group breaks.
    ///
    /// This is the workhorse separator: put it between items that should sit on
    /// one line when they fit and stack one-per-line when they do not.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("a").append(Doc::line()).append(Doc::text("b")).group();
    /// assert_eq!(doc.render(80), "a b");   // fits: space
    /// assert_eq!(doc.render(1), "a\nb");   // too narrow: newline
    /// ```
    #[inline]
    #[must_use]
    pub fn line() -> Doc {
        Doc(Rc::new(Node::Line))
    }

    /// A flexible break that is *nothing* when its group is flat and a newline
    /// (plus indentation) when the group breaks. Use it where a broken layout
    /// wants a line break but a flat layout wants no space at all — for example
    /// right after an opening bracket.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("(")
    ///     .append(Doc::softline())
    ///     .append(Doc::text("x"))
    ///     .group();
    /// assert_eq!(doc.render(80), "(x");  // flat: no gap
    /// ```
    #[inline]
    #[must_use]
    pub fn softline() -> Doc {
        Doc(Rc::new(Node::SoftLine))
    }

    /// A break that is *always* a newline, and forces every group that contains
    /// it to break. Use it for constructs that must never be collapsed onto one
    /// line, such as line comments or statement separators in block bodies.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("a").append(Doc::hardline()).append(Doc::text("b")).group();
    /// // Even though "a b" would fit at width 80, the hardline forces a break.
    /// assert_eq!(doc.render(80), "a\nb");
    /// ```
    #[inline]
    #[must_use]
    pub fn hardline() -> Doc {
        Doc(Rc::new(Node::HardLine))
    }

    /// Concatenate `self` with `other`, laid out left then right. This is the
    /// fundamental way to build a document up from parts.
    ///
    /// [`nil`](Doc::nil) is the identity: `a.append(Doc::nil())` and
    /// `Doc::nil().append(a)` both render exactly as `a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("fn ").append(Doc::text("main")).append(Doc::text("()"));
    /// assert_eq!(doc.render(80), "fn main()");
    /// ```
    #[inline]
    #[must_use]
    pub fn append(self, other: Doc) -> Doc {
        let fit = Fit::cat(self.0.fit(), other.0.fit());
        Doc(Rc::new(Node::Cat(self, other, fit)))
    }

    /// Increase the indentation applied to every line break *inside* `self` by
    /// `indent` columns. Indentation is relative and nests: an inner `nest(4)`
    /// inside an outer `nest(4)` indents broken lines by eight.
    ///
    /// `indent` is an `isize`; a negative value dedents. The effective
    /// indentation never goes below zero (it is clamped at the point a newline
    /// is emitted).
    ///
    /// Only line breaks that actually happen are affected — `nest` on a document
    /// that stays flat has no visible effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let body = Doc::text("{")
    ///     .append(
    ///         Doc::line()
    ///             .append(Doc::text("stmt;"))
    ///             .nest(4),
    ///     )
    ///     .append(Doc::line())
    ///     .append(Doc::text("}"))
    ///     .group();
    ///
    /// assert_eq!(body.render(4), "{\n    stmt;\n}");
    /// ```
    #[inline]
    #[must_use]
    pub fn nest(self, indent: isize) -> Doc {
        let fit = self.0.fit();
        Doc(Rc::new(Node::Nest(indent, self, fit)))
    }

    /// Mark `self` as a layout choice point.
    ///
    /// When the renderer reaches a group it first asks whether the group's
    /// contents fit, laid out flat, in the width remaining on the current line.
    /// If they do, every flexible break inside becomes its flat form (a space or
    /// nothing). If they do not — or the group contains a
    /// [`hardline`](Doc::hardline) — every flexible break inside becomes a
    /// newline. The decision is all-or-nothing for the breaks *directly* owned
    /// by this group; nested groups are decided independently.
    ///
    /// Grouping is what turns one document into "one line if it fits, otherwise
    /// stacked". A document with no groups always uses the broken form of every
    /// break.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let call = Doc::text("f(")
    ///     .append(
    ///         Doc::softline()
    ///             .append(Doc::join(
    ///                 Doc::text(",").append(Doc::line()),
    ///                 ["alpha", "beta", "gamma"].map(Doc::text),
    ///             ))
    ///             .nest(4),
    ///     )
    ///     .append(Doc::softline())
    ///     .append(Doc::text(")"))
    ///     .group();
    ///
    /// assert_eq!(call.render(80), "f(alpha, beta, gamma)");
    /// assert_eq!(
    ///     call.render(10),
    ///     "f(\n    alpha,\n    beta,\n    gamma\n)"
    /// );
    /// ```
    #[inline]
    #[must_use]
    pub fn group(self) -> Doc {
        let flat = self.0.fit().flat;
        Doc(Rc::new(Node::Group(self, flat)))
    }

    /// Concatenate every document produced by `docs`, in order. Returns
    /// [`nil`](Doc::nil) for an empty iterator.
    ///
    /// This is a left fold of [`append`](Doc::append) and allocates one internal
    /// node per item.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::concat(["a", "b", "c"].map(Doc::text));
    /// assert_eq!(doc.render(80), "abc");
    ///
    /// assert_eq!(Doc::concat(core::iter::empty()).render(80), "");
    /// ```
    #[must_use]
    pub fn concat(docs: impl IntoIterator<Item = Doc>) -> Doc {
        let mut iter = docs.into_iter();
        let mut acc = match iter.next() {
            Some(first) => first,
            None => return Doc::nil(),
        };
        for doc in iter {
            acc = acc.append(doc);
        }
        acc
    }

    /// Concatenate every document produced by `docs`, placing a clone of `sep`
    /// between consecutive items (but not before the first or after the last).
    /// Returns [`nil`](Doc::nil) for an empty iterator.
    ///
    /// This is the idiomatic way to render comma-separated lists, `&&`-joined
    /// conditions, `::`-joined paths, and the like — pair it with
    /// [`group`](Doc::group) so the whole list collapses onto one line when it
    /// fits.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let path = Doc::join(Doc::text("::"), ["std", "collections", "HashMap"].map(Doc::text));
    /// assert_eq!(path.render(80), "std::collections::HashMap");
    ///
    /// // With a flexible separator, the list reflows under a group.
    /// let args = Doc::join(
    ///     Doc::text(",").append(Doc::line()),
    ///     ["x", "y"].map(Doc::text),
    /// )
    /// .group();
    /// assert_eq!(args.render(80), "x, y");
    /// ```
    #[must_use]
    pub fn join(sep: Doc, docs: impl IntoIterator<Item = Doc>) -> Doc {
        let mut iter = docs.into_iter();
        let mut acc = match iter.next() {
            Some(first) => first,
            None => return Doc::nil(),
        };
        for doc in iter {
            acc = acc.append(sep.clone()).append(doc);
        }
        acc
    }

    /// Render this document to an owned [`String`], choosing line breaks so that
    /// no line exceeds `width` columns where the document allows a choice.
    ///
    /// `width` is the target line length in `char`s, counted the same way as
    /// [`text`](Doc::text) widths (`chars().count()`, so not display cells).
    /// Lines can still exceed it when a single unbreakable
    /// [`text`](Doc::text) is wider than `width`, or where the document offers
    /// no break — the renderer never invents break points that were not
    /// described.
    ///
    /// Every width at or above `isize::MAX` is "unlimited" and behaves exactly
    /// like `isize::MAX`, so `usize::MAX` is a safe way to ask for the
    /// widest layout. (Before 1.0.1, widths above `isize::MAX` wrapped to a
    /// negative width and broke every group.)
    ///
    /// Rendering takes time linear in the size of the document, whatever the
    /// width.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("a").append(Doc::line()).append(Doc::text("b")).group();
    /// assert_eq!(doc.render(80), "a b");
    /// assert_eq!(doc.render(1), "a\nb");
    ///
    /// // Unlimited width: every group that can be flat is flat.
    /// assert_eq!(doc.render(usize::MAX), "a b");
    /// ```
    #[must_use]
    pub fn render(&self, width: usize) -> String {
        let mut out = String::new();
        // Writing into a String is infallible, so the fmt::Result is discarded.
        let _ = crate::render::layout(self, width, &mut out);
        out
    }

    /// Render this document into any [`core::fmt::Write`] sink, choosing line
    /// breaks for the target `width`. Use this to stream directly into a caller
    /// -owned buffer and avoid the intermediate [`String`] that
    /// [`render`](Doc::render) allocates. `width` means exactly what it means
    /// for [`render`](Doc::render) (counted in `char`s; `isize::MAX` and above
    /// is unlimited), and the output is identical.
    ///
    /// # Errors
    ///
    /// Returns [`core::fmt::Error`] if and only if the underlying `out` returns
    /// an error while being written to.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::fmt::Write;
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("hello").append(Doc::text(" world"));
    /// let mut buf = String::new();
    /// doc.render_into(80, &mut buf).unwrap();
    /// assert_eq!(buf, "hello world");
    /// ```
    pub fn render_into<W: core::fmt::Write>(&self, width: usize, out: &mut W) -> core::fmt::Result {
        crate::render::layout(self, width, out)
    }

    /// Render this document into a [`std::io::Write`] sink, choosing line breaks
    /// for the target `width`. This is the streaming counterpart to
    /// [`render`](Doc::render) for files, sockets, and stdout. `width` means
    /// exactly what it means for [`render`](Doc::render), and the bytes written
    /// are the UTF-8 of the string `render` would return.
    ///
    /// # Errors
    ///
    /// Propagates the first [`std::io::Error`] returned by `out`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("written to stdout");
    /// let mut buf: Vec<u8> = Vec::new();
    /// doc.render_writer(80, &mut buf).unwrap();
    /// assert_eq!(buf, b"written to stdout");
    /// ```
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn render_writer<W: std::io::Write>(
        &self,
        width: usize,
        out: &mut W,
    ) -> std::io::Result<()> {
        crate::render::layout_io(self, width, out)
    }
}

/// The empty document — same as [`Doc::nil`].
impl Default for Doc {
    #[inline]
    fn default() -> Self {
        Doc::nil()
    }
}

/// Build a text document from a static string slice, without allocating.
impl From<&'static str> for Doc {
    #[inline]
    fn from(s: &'static str) -> Self {
        Doc::text(s)
    }
}

/// Build a text document from an owned string.
impl From<String> for Doc {
    #[inline]
    fn from(s: String) -> Self {
        Doc::text(s)
    }
}

impl Drop for Doc {
    /// Dismantle the document iteratively when this is its last owner.
    ///
    /// The document is a tree of reference-counted nodes, so the derived drop
    /// glue would recurse one call frame per level and overflow the stack on a
    /// deeply nested document (a long chain of binary expressions, say). This
    /// impl walks a uniquely-owned spine with an explicit heap work list
    /// instead, keeping the actual node drops shallow. Leaves and shared nodes
    /// take a branch-only fast path that allocates nothing.
    fn drop(&mut self) {
        // A leaf owns no child nodes: nothing to recurse into.
        if matches!(
            &*self.0,
            Node::Nil | Node::Text(..) | Node::Line | Node::SoftLine | Node::HardLine
        ) {
            return;
        }
        // A shared internal node stays alive after this handle goes away, so
        // dropping it will not recurse into its children.
        if Rc::get_mut(&mut self.0).is_none() {
            return;
        }
        // Uniquely-owned internal node: take its children onto a work list and
        // dismantle the spine level by level. One `Nil` sentinel, cloned by
        // reference count, stands in for every child slot we empty.
        let nil = Rc::new(Node::Nil);
        let mut stack: Vec<Rc<Node>> = Vec::new();
        take_children(&mut self.0, &nil, &mut stack);
        while let Some(mut node) = stack.pop() {
            take_children(&mut node, &nil, &mut stack);
        }
    }
}

/// Move the child nodes of a uniquely-owned internal node onto `stack`,
/// replacing each slot with the shared `nil` sentinel so the node itself then
/// drops without recursing. A shared node (`get_mut` is `None`) is left alone.
fn take_children(rc: &mut Rc<Node>, nil: &Rc<Node>, stack: &mut Vec<Rc<Node>>) {
    let Some(node) = Rc::get_mut(rc) else { return };
    match node {
        Node::Cat(a, b, _) => {
            stack.push(core::mem::replace(&mut a.0, nil.clone()));
            stack.push(core::mem::replace(&mut b.0, nil.clone()));
        }
        Node::Nest(_, x, _) | Node::Group(x, _) => {
            stack.push(core::mem::replace(&mut x.0, nil.clone()));
        }
        Node::Nil | Node::Text(..) | Node::Line | Node::SoftLine | Node::HardLine => {}
    }
}

impl core::fmt::Debug for Doc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // A structural view of the tree, useful when debugging a builder.
        // Written iteratively so a deep document cannot overflow the stack.
        enum Step {
            Node(Doc),
            Str(&'static str),
        }
        let mut stack = Vec::from([Step::Node(self.clone())]);
        while let Some(step) = stack.pop() {
            match step {
                Step::Str(s) => f.write_str(s)?,
                Step::Node(doc) => match &*doc.0 {
                    Node::Nil => f.write_str("Nil")?,
                    Node::Text(s, _) => write!(f, "Text({s:?})")?,
                    Node::Line => f.write_str("Line")?,
                    Node::SoftLine => f.write_str("SoftLine")?,
                    Node::HardLine => f.write_str("HardLine")?,
                    Node::Cat(a, b, _) => {
                        f.write_str("Cat(")?;
                        stack.push(Step::Str(")"));
                        stack.push(Step::Node(b.clone()));
                        stack.push(Step::Str(", "));
                        stack.push(Step::Node(a.clone()));
                    }
                    Node::Nest(i, x, _) => {
                        write!(f, "Nest({i}, ")?;
                        stack.push(Step::Str(")"));
                        stack.push(Step::Node(x.clone()));
                    }
                    Node::Group(x, _) => {
                        f.write_str("Group(")?;
                        stack.push(Step::Str(")"));
                        stack.push(Step::Node(x.clone()));
                    }
                },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Doc, INF, NO_BREAK};

    #[test]
    fn test_fit_leaves() {
        assert_eq!(Doc::nil().0.fit().flat, 0);
        assert_eq!(Doc::text("ab").0.fit().flat, 2);
        assert_eq!(Doc::text("ab").0.fit().brk, NO_BREAK);
        assert_eq!(Doc::line().0.fit().flat, 1);
        assert_eq!(Doc::line().0.fit().brk, 0);
        assert_eq!(Doc::softline().0.fit().flat, 0);
        assert_eq!(Doc::hardline().0.fit().flat, INF);
        assert_eq!(Doc::hardline().0.fit().brk, 0);
    }

    #[test]
    fn test_fit_cat_stops_at_first_owned_break() {
        let doc = Doc::text("abc")
            .append(Doc::line())
            .append(Doc::text("defgh"));
        let fit = doc.0.fit();
        assert_eq!(fit.flat, 9);
        assert_eq!(fit.brk, 3);
    }

    #[test]
    fn test_fit_group_hides_breaks_from_break_mode() {
        // Seen from outside, a group is measured flat and owns no break.
        let inner = Doc::text("a").append(Doc::line()).append(Doc::text("b"));
        let fit = inner.clone().group().append(Doc::text("c")).0.fit();
        assert_eq!(fit.flat, 4);
        assert_eq!(fit.brk, NO_BREAK);
        // Nest is transparent.
        let nested = inner.nest(4).0.fit();
        assert_eq!((nested.flat, nested.brk), (3, 1));
    }

    #[test]
    fn test_fit_hardline_in_group_is_infinitely_wide() {
        let fit = Doc::text("a").append(Doc::hardline()).group().0.fit();
        assert_eq!(fit.flat, INF);
    }

    #[test]
    fn test_fit_saturates_on_shared_blowup() {
        // Sharing makes the tree 2^70 leaves wide while the DAG stays tiny;
        // the width must saturate at INF instead of wrapping.
        let mut doc = Doc::text("x");
        for _ in 0..70 {
            doc = doc.clone().append(doc);
        }
        let fit = doc.0.fit();
        assert_eq!(fit.flat, INF);
        assert_eq!(fit.brk, NO_BREAK);
        let with_break = doc.append(Doc::line()).0.fit();
        assert_eq!(with_break.flat, INF);
        // The break-mode prefix saturated too; INF is the shared encoding.
        assert_eq!(with_break.brk, INF);
    }
}
