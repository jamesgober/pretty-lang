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
    /// Literal text with its precomputed display width (in Unicode scalars).
    /// The text MUST NOT contain a newline; use [`Doc::hardline`] for those.
    Text(Cow<'static, str>, usize),
    /// A space when the enclosing group is flat, a newline when it is broken.
    Line,
    /// Nothing when the enclosing group is flat, a newline when it is broken.
    SoftLine,
    /// Always a newline; forces every enclosing group to break.
    HardLine,
    /// Concatenation of two documents, laid out left then right.
    Cat(Doc, Doc),
    /// Adds `isize` columns of indentation to line breaks inside `Doc`.
    Nest(isize, Doc),
    /// A layout choice point: render the inside flat if it fits the remaining
    /// width on the current line, otherwise break every flexible line in it.
    Group(Doc),
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
    /// moved in. The display width is measured once, here, as the number of
    /// Unicode scalar values.
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
        Doc(Rc::new(Node::Cat(self, other)))
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
        Doc(Rc::new(Node::Nest(indent, self)))
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
        Doc(Rc::new(Node::Group(self)))
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
    /// `width` is the target line length in Unicode scalars. Lines can still
    /// exceed it when a single unbreakable [`text`](Doc::text) is wider than
    /// `width`, or where the document offers no break — the renderer never
    /// invents break points that were not described.
    ///
    /// # Examples
    ///
    /// ```
    /// use pretty_lang::Doc;
    ///
    /// let doc = Doc::text("a").append(Doc::line()).append(Doc::text("b")).group();
    /// assert_eq!(doc.render(80), "a b");
    /// assert_eq!(doc.render(1), "a\nb");
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
    /// [`render`](Doc::render) allocates.
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
    /// [`render`](Doc::render) for files, sockets, and stdout.
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
        Node::Cat(a, b) => {
            stack.push(core::mem::replace(&mut a.0, nil.clone()));
            stack.push(core::mem::replace(&mut b.0, nil.clone()));
        }
        Node::Nest(_, x) | Node::Group(x) => {
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
                    Node::Cat(a, b) => {
                        f.write_str("Cat(")?;
                        stack.push(Step::Str(")"));
                        stack.push(Step::Node(b.clone()));
                        stack.push(Step::Str(", "));
                        stack.push(Step::Node(a.clone()));
                    }
                    Node::Nest(i, x) => {
                        write!(f, "Nest({i}, ")?;
                        stack.push(Step::Str(")"));
                        stack.push(Step::Node(x.clone()));
                    }
                    Node::Group(x) => {
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
