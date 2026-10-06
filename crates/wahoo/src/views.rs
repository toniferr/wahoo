//! Human- and machine-readable views of each phase: token tables, syntax trees and diagnostics, as text (for the
//! CLI) or JSON (for the playground). JSON is written by hand: the core crate has no dependencies.

use crate::ast::{self, ExprKind, Item, StmtKind};
use crate::diagnostics::{Diagnostic, Lang, Severity};
use crate::lexer::Token;
use crate::span::{Span, line_col};

/// A generic labelled tree, built from the AST, that both printers walk.
pub struct Node {
    pub label: String,
    pub class: &'static str,
    pub span: Span,
    pub children: Vec<Node>,
}

fn node(label: impl Into<String>, class: &'static str, span: Span, children: Vec<Node>) -> Node {
    Node { label: label.into(), class, span, children }
}

pub fn ast_tree(p: &ast::Program) -> Node {
    let items = p
        .items
        .iter()
        .map(|it| match it {
            Item::Pipe(f) => {
                let params: Vec<String> = f.params.iter().map(|p| format!("{}: {}", p.name.name, p.ty.name)).collect();
                let ret = f.ret.as_ref().map(|t| format!(" -> {}", t.name)).unwrap_or_default();
                let label = format!("pipe {}({}){ret}", f.name.name, params.join(", "));
                node(label, "item", f.name.span.to(f.body.span), block(&f.body))
            }
            Item::World(w) => node(format!("world {}", w.label()), "item", w.span.to(w.body.span), block(&w.body)),
        })
        .collect();
    let span = p.items.first().map(|_| Span::default()).unwrap_or_default();
    node("program", "root", span, items)
}

fn block(b: &ast::Block) -> Vec<Node> {
    b.stmts.iter().map(stmt).collect()
}

fn stmt(s: &ast::Stmt) -> Node {
    match &s.kind {
        StmtKind::Coin { name, ty, value } => {
            let t = ty.as_ref().map(|t| format!(": {}", t.name)).unwrap_or_default();
            node(format!("coin {}{t}", name.name), "stmt", s.span, vec![expr(value)])
        }
        StmtKind::Assign { name, value } => node(format!("{} =", name.name), "stmt", s.span, vec![expr(value)]),
        StmtKind::Counter { up, name, by } => {
            let kw = if *up { "power_up" } else { "damage" };
            node(format!("{kw} {}", name.name), "stmt", s.span, by.iter().map(expr).collect())
        }
        StmtKind::Question { cond, then, otherwise } => {
            let mut kids = vec![expr(cond), node("then", "block", then.span, block(then))];
            if let Some(o) = otherwise {
                kids.push(node("else", "block", o.span, block(o)));
            }
            node("question", "stmt", s.span, kids)
        }
        StmtKind::Bounce { cond, body } => {
            node("bounce", "stmt", s.span, vec![expr(cond), node("body", "block", body.span, block(body))])
        }
        StmtKind::Run { var, from, to, body } => node(
            format!("run {}", var.name),
            "stmt",
            s.span,
            vec![
                node("from", "block", from.span, vec![expr(from)]),
                node("to", "block", to.span, vec![expr(to)]),
                node("body", "block", body.span, block(body)),
            ],
        ),
        StmtKind::Flag { value } => node("flag", "stmt", s.span, value.iter().map(expr).collect()),
        StmtKind::Call(e) => expr(e),
    }
}

fn expr(e: &ast::Expr) -> Node {
    match &e.kind {
        ExprKind::Int(n) => node(n.to_string(), "lit", e.span, vec![]),
        ExprKind::Text(t) => node(format!("{t:?}"), "lit", e.span, vec![]),
        ExprKind::Bool(b) => node(if *b { "star" } else { "goomba" }, "lit", e.span, vec![]),
        ExprKind::Name(n) => node(n.clone(), "name", e.span, vec![]),
        ExprKind::Unary(op, x) => node(op.symbol(), "op", e.span, vec![expr(x)]),
        ExprKind::Binary(op, a, b) => node(op.symbol(), "op", e.span, vec![expr(a), expr(b)]),
        ExprKind::Call { callee, args } => {
            node(format!("{}()", callee.name), "call", e.span, args.iter().map(expr).collect())
        }
    }
}

/// `├─` / `└─` drawing of a tree.
pub fn tree_text(root: &Node) -> String {
    fn walk(n: &Node, prefix: &str, last: bool, top: bool, out: &mut String) {
        if top {
            out.push_str(&n.label);
        } else {
            out.push_str(prefix);
            out.push_str(if last { "└─ " } else { "├─ " });
            out.push_str(&n.label);
        }
        out.push('\n');
        let child_prefix = if top { String::new() } else { format!("{prefix}{}", if last { "   " } else { "│  " }) };
        for (i, c) in n.children.iter().enumerate() {
            walk(c, &child_prefix, i + 1 == n.children.len(), false, out);
        }
    }
    let mut out = String::new();
    walk(root, "", true, true, &mut out);
    out
}

pub fn tokens_text(src: &str, tokens: &[Token]) -> String {
    let mut out = String::new();
    for t in tokens {
        let lc = line_col(src, t.span.start);
        let text = &src[t.span.start..t.span.end];
        out.push_str(&format!(
            "{:>4}:{:<3} {:<8} {}\n",
            lc.line,
            lc.col,
            t.kind.class(),
            if text.is_empty() { "␄" } else { text }
        ));
    }
    out
}

// ------------------------------------------------------------------ JSON

pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Character offset (what JavaScript strings index by, for text without astral characters).
fn char_pos(src: &str, byte: usize) -> usize {
    src[..byte.min(src.len())].chars().count()
}

fn span_json(src: &str, span: Span) -> String {
    let a = line_col(src, span.start);
    format!(
        "\"from\":{},\"to\":{},\"line\":{},\"col\":{}",
        char_pos(src, span.start),
        char_pos(src, span.end),
        a.line,
        a.col
    )
}

pub fn tokens_json(src: &str, tokens: &[Token]) -> String {
    let items: Vec<String> = tokens
        .iter()
        .map(|t| {
            format!(
                "{{\"class\":{},\"text\":{},{}}}",
                json_str(t.kind.class()),
                json_str(&src[t.span.start..t.span.end]),
                span_json(src, t.span)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

pub fn tree_json(src: &str, n: &Node) -> String {
    let kids: Vec<String> = n.children.iter().map(|c| tree_json(src, c)).collect();
    format!(
        "{{\"label\":{},\"class\":{},{},\"children\":[{}]}}",
        json_str(&n.label),
        json_str(n.class),
        span_json(src, n.span),
        kids.join(",")
    )
}

pub fn diagnostics_json(src: &str, diags: &[Diagnostic], lang: Lang) -> String {
    let items: Vec<String> = diags
        .iter()
        .map(|d| {
            let (title, label, hint) = d.texts(lang);
            let end = line_col(src, d.span.end);
            format!(
                "{{\"severity\":{},\"code\":{},\"title\":{},\"label\":{},\"hint\":{},{},\"endLine\":{},\"endCol\":{},\"rendered\":{}}}",
                json_str(if d.severity == Severity::Error { "error" } else { "warning" }),
                json_str(d.code()),
                json_str(&title),
                json_str(&label),
                hint.map_or("null".into(), |h| json_str(&h)),
                span_json(src, d.span),
                end.line,
                end.col,
                json_str(&d.render(src, "playground.wahoo", lang)),
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}
