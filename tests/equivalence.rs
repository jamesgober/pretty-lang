//! Layout equivalence: the 1.0.1 engine against the 1.0.0 algorithm.
//!
//! 1.0.1 replaced the look-ahead scan with O(1) precomputed fit summaries
//! (ISSUES M65). The rendered output is part of the 1.x contract, so this file
//! keeps a verbatim port of the 1.0.0 renderer (`reference::render`) over an
//! independent mirror tree, and checks that the shipped engine produces
//! byte-identical output for random documents at random and extreme widths.
//!
//! The only intended difference is the M64 fix: 1.0.0 computed `width as
//! isize`, so widths above `isize::MAX` wrapped negative. The reference applies
//! the same clamp the fix does; everything else is the 1.0.0 code unchanged.

#![allow(clippy::unwrap_used)]

use std::rc::Rc;

use pretty_lang::Doc;
use proptest::prelude::*;

/// The 1.0.0 layout engine, ported line for line onto a mirror tree.
mod reference {
    use std::fmt::Write;
    use std::rc::Rc;

    /// Mirror of the 1.0.0 internal node shape (`Debug` for proptest reports;
    /// generated trees are shallow, so the derived recursion is fine here).
    #[derive(Debug)]
    pub enum R {
        Nil,
        Text(String, usize),
        Line,
        SoftLine,
        HardLine,
        Cat(Rc<R>, Rc<R>),
        Nest(isize, Rc<R>),
        Group(Rc<R>),
    }

    impl R {
        pub fn text(s: &str) -> Rc<R> {
            Rc::new(R::Text(s.to_owned(), s.chars().count()))
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Flat,
        Break,
    }

    struct Frame<'a> {
        indent: isize,
        mode: Mode,
        node: &'a R,
    }

    /// 1.0.0 `layout`, with the M64 clamp in place of `width as isize`.
    pub fn render(root: &R, width: usize) -> String {
        let mut out = String::new();
        let width = isize::try_from(width).unwrap_or(isize::MAX);
        let mut col: isize = 0;
        let mut stack: Vec<Frame<'_>> = vec![Frame {
            indent: 0,
            mode: Mode::Break,
            node: root,
        }];
        while let Some(Frame { indent, mode, node }) = stack.pop() {
            match node {
                R::Nil => {}
                R::Text(s, w) => {
                    out.push_str(s);
                    col = col.saturating_add(*w as isize);
                }
                R::Cat(a, b) => {
                    stack.push(Frame {
                        indent,
                        mode,
                        node: b,
                    });
                    stack.push(Frame {
                        indent,
                        mode,
                        node: a,
                    });
                }
                R::Nest(j, x) => stack.push(Frame {
                    indent: indent.saturating_add(*j),
                    mode,
                    node: x,
                }),
                R::Line => match mode {
                    Mode::Flat => {
                        out.push(' ');
                        col = col.saturating_add(1);
                    }
                    Mode::Break => col = new_line(&mut out, indent),
                },
                R::SoftLine => match mode {
                    Mode::Flat => {}
                    Mode::Break => col = new_line(&mut out, indent),
                },
                R::HardLine => col = new_line(&mut out, indent),
                R::Group(x) => {
                    let mode = if fits(width - col, indent, x, &stack) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    stack.push(Frame {
                        indent,
                        mode,
                        node: x,
                    });
                }
            }
        }
        out
    }

    fn new_line(out: &mut String, indent: isize) -> isize {
        out.push('\n');
        let indent = indent.max(0);
        write!(out, "{:width$}", "", width = indent as usize).unwrap();
        indent
    }

    /// 1.0.0 `fits`: the bounded scan over the group and the continuation.
    fn fits(avail: isize, indent: isize, group: &R, stack: &[Frame<'_>]) -> bool {
        if avail < 0 {
            return false;
        }
        let mut remaining = avail;
        let mut local: Vec<(isize, Mode, &R)> = vec![(indent, Mode::Flat, group)];
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
                R::Nil => {}
                R::Text(_, w) => {
                    remaining -= *w as isize;
                    if remaining < 0 {
                        return false;
                    }
                }
                R::Cat(a, b) => {
                    local.push((i, mode, b));
                    local.push((i, mode, a));
                }
                R::Nest(j, x) => local.push((i.saturating_add(*j), mode, x)),
                R::Line => match mode {
                    Mode::Flat => {
                        remaining -= 1;
                        if remaining < 0 {
                            return false;
                        }
                    }
                    Mode::Break => return true,
                },
                R::SoftLine => {
                    if mode == Mode::Break {
                        return true;
                    }
                }
                R::HardLine => match mode {
                    Mode::Flat => return false,
                    Mode::Break => return true,
                },
                R::Group(x) => local.push((i, Mode::Flat, x)),
            }
        }
    }
}

use reference::R;

type Pair = (Doc, Rc<R>);

/// Leaves, weighted towards the zero-width cases that made 1.0.0 quadratic
/// (`nil`, empty text, `softline`), plus multi-byte text so the char-count
/// width is exercised.
fn arb_leaf() -> impl Strategy<Value = Pair> {
    prop_oneof![
        3 => Just((Doc::nil(), Rc::new(R::Nil))),
        3 => Just((Doc::text(""), R::text(""))),
        4 => "[a-z0-9]{1,8}".prop_map(|s| (Doc::text(s.clone()), R::text(&s))),
        1 => "[é日a]{1,4}".prop_map(|s| (Doc::text(s.clone()), R::text(&s))),
        3 => Just((Doc::line(), Rc::new(R::Line))),
        3 => Just((Doc::softline(), Rc::new(R::SoftLine))),
        1 => Just((Doc::hardline(), Rc::new(R::HardLine))),
    ]
}

/// Random documents built in lockstep with their mirror tree, including
/// shared subtrees (the same `Doc` appended to itself).
fn arb_pair() -> impl Strategy<Value = Pair> {
    arb_leaf().prop_recursive(8, 256, 4, |inner| {
        prop_oneof![
            3 => (inner.clone(), inner.clone()).prop_map(|((a, ra), (b, rb))| {
                (a.append(b), Rc::new(R::Cat(ra, rb)))
            }),
            1 => (-4isize..8, inner.clone())
                .prop_map(|(n, (d, r))| (d.nest(n), Rc::new(R::Nest(n, r)))),
            3 => inner.clone().prop_map(|(d, r)| (d.group(), Rc::new(R::Group(r)))),
            1 => inner.prop_map(|(d, r)| {
                (d.clone().append(d), Rc::new(R::Cat(r.clone(), r)))
            }),
        ]
    })
}

/// Widths that matter: small (where most groups break), mid-range, and the
/// extremes around the `isize`/`usize` boundary.
fn arb_width() -> impl Strategy<Value = usize> {
    prop_oneof![
        4 => 0usize..24,
        2 => 24usize..200,
        1 => Just(0usize),
        1 => Just(1usize),
        1 => Just(isize::MAX as usize),
        1 => Just(isize::MAX as usize + 1),
        1 => Just(usize::MAX),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// The 1.0.1 engine renders every document exactly as 1.0.0 did (with
    /// the M64 clamp), at every width.
    #[test]
    fn prop_matches_1_0_0_layout((doc, r) in arb_pair(), width in arb_width()) {
        prop_assert_eq!(doc.render(width), reference::render(&r, width));
    }

    /// Same check with the whole document wrapped in a group, so the root
    /// decision is a real choice rather than always-broken.
    #[test]
    fn prop_matches_1_0_0_layout_grouped((doc, r) in arb_pair(), width in arb_width()) {
        prop_assert_eq!(
            doc.group().render(width),
            reference::render(&R::Group(r), width)
        );
    }
}

/// Fixed shapes from the M65 report, small enough for the quadratic reference.
#[test]
fn test_pathological_shapes_match_reference() {
    let n = 2_000;
    let shapes: Vec<Pair> = vec![
        (
            Doc::concat((0..n).map(|_| Doc::softline().group())),
            (0..n).fold(Rc::new(R::Nil), |acc, _| {
                Rc::new(R::Cat(acc, Rc::new(R::Group(Rc::new(R::SoftLine)))))
            }),
        ),
        (
            Doc::concat((0..n).map(|_| Doc::nil().group())).group(),
            Rc::new(R::Group((0..n).fold(Rc::new(R::Nil), |acc, _| {
                Rc::new(R::Cat(acc, Rc::new(R::Group(Rc::new(R::Nil)))))
            }))),
        ),
        (
            Doc::concat((0..n).map(|_| Doc::text("").group().append(Doc::line()))).group(),
            Rc::new(R::Group((0..n).fold(Rc::new(R::Nil), |acc, _| {
                let item = Rc::new(R::Cat(Rc::new(R::Group(R::text(""))), Rc::new(R::Line)));
                Rc::new(R::Cat(acc, item))
            }))),
        ),
    ];
    for (doc, r) in &shapes {
        for width in [0, 1, 80, 1_000, 5_000, isize::MAX as usize, usize::MAX] {
            assert_eq!(
                doc.render(width),
                reference::render(r, width),
                "width {width}"
            );
        }
    }
}
