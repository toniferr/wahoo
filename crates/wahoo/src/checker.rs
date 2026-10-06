//! Semantic analysis: AST → typed HIR.
//!
//! Two passes. The first collects every pipe's signature, so pipes can call each other in any order (and
//! themselves: recursion). The second walks each body with a stack of scopes (the symbol table): a `coin`
//! declares a name in the innermost block, a name is looked up from the innermost block outwards, and every
//! expression gets a type that is checked against where it is used.

use std::collections::HashMap;

use crate::ast::{self, BinOp, ExprKind as A, StmtKind as S, UnOp};
use crate::diagnostics::{Ctx, Diagnostic, Kind, suggest};
use crate::hir::{self, Expr, ExprKind as H, Stmt, StmtKind};
use crate::span::Span;
use crate::types::{TYPE_NAMES, Ty};

pub const PRINT: &str = "wahoo";

pub fn check(program: &ast::Program, src: &str) -> (hir::Program, Vec<Diagnostic>) {
    let mut c = Checker {
        lines: LineIndex::new(src),
        diags: Vec::new(),
        sigs: HashMap::new(),
        strings: Vec::new(),
        string_ids: HashMap::new(),
    };
    let funcs = c.program(program);
    (hir::Program { funcs, strings: c.strings }, c.diags)
}

struct Sig {
    id: u32,
    params: Vec<Ty>,
    ret: Option<Ty>,
}

struct Checker {
    lines: LineIndex,
    diags: Vec<Diagnostic>,
    sigs: HashMap<String, Sig>,
    strings: Vec<String>,
    string_ids: HashMap<String, u32>,
}

/// One function being checked: its locals and the scopes that name them.
struct FnCtx {
    name: String,
    world: bool,
    ret: Option<Ty>,
    locals: Vec<hir::Local>,
    scopes: Vec<Vec<Binding>>,
}

struct Binding {
    name: String,
    local: u32,
    used: bool,
    span: Span,
}

/// Byte offsets where lines start, to turn spans into line numbers quickly.
struct LineIndex(Vec<usize>);

impl LineIndex {
    fn new(src: &str) -> Self {
        LineIndex(std::iter::once(0).chain(src.match_indices('\n').map(|(i, _)| i + 1)).collect())
    }

    fn line(&self, offset: usize) -> usize {
        self.0.partition_point(|&start| start <= offset)
    }
}

/// Spellings from other languages, mapped to the Wahoo type they most likely mean.
const TYPE_ALIASES: &[(&str, &str)] = &[
    ("int", "coins"),
    ("i32", "coins"),
    ("integer", "coins"),
    ("number", "coins"),
    ("coin", "coins"),
    ("bool", "switch"),
    ("boolean", "switch"),
    ("string", "text"),
    ("str", "text"),
    ("String", "text"),
];

impl Checker {
    fn error(&mut self, kind: Kind, span: Span) {
        self.diags.push(Diagnostic::error(kind, span));
    }

    fn warning(&mut self, kind: Kind, span: Span) {
        self.diags.push(Diagnostic::warning(kind, span));
    }

    fn resolve_type(&mut self, t: &ast::Ident) -> Ty {
        if let Some(ty) = Ty::from_name(&t.name) {
            return ty;
        }
        let suggestion = TYPE_ALIASES
            .iter()
            .find(|(alias, _)| *alias == t.name)
            .map(|(_, ty)| ty.to_string())
            .or_else(|| suggest(&t.name, TYPE_NAMES));
        self.error(Kind::UnknownType { name: t.name.clone(), suggestion }, t.span);
        Ty::Error
    }

    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.string_ids.get(s) {
            return id;
        }
        let id = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.string_ids.insert(s.to_string(), id);
        id
    }

    // ------------------------------------------------------------------ program

    fn program(&mut self, program: &ast::Program) -> Vec<hir::Func> {
        // Pass 1: signatures.
        let mut pipes = Vec::new();
        for item in &program.items {
            let ast::Item::Pipe(p) = item else { continue };
            let params: Vec<Ty> = p.params.iter().map(|prm| self.resolve_type(&prm.ty)).collect();
            let ret = p.ret.as_ref().map(|t| self.resolve_type(t));
            if p.name.name == PRINT {
                self.error(Kind::ReservedName(p.name.name.clone()), p.name.span);
                continue;
            }
            if self.sigs.contains_key(&p.name.name) {
                self.error(Kind::DuplicatePipe(p.name.name.clone()), p.name.span);
                continue;
            }
            let id = pipes.len() as u32;
            self.sigs.insert(p.name.name.clone(), Sig { id, params: params.clone(), ret });
            pipes.push((p, params, ret));
        }

        let mut worlds: Vec<&ast::WorldDecl> = Vec::new();
        for item in &program.items {
            let ast::Item::World(w) = item else { continue };
            if worlds.iter().any(|o| (o.major, o.minor) == (w.major, w.minor)) {
                self.error(Kind::DuplicateWorld(w.label()), w.span);
                continue;
            }
            worlds.push(w);
        }
        if worlds.is_empty() {
            self.error(Kind::NoWorld, Span::new(0, 0));
        }
        worlds.sort_by_key(|w| (w.major, w.minor));

        // Pass 2: bodies.
        let mut funcs = Vec::new();
        for (p, params, ret) in pipes {
            let mut f =
                FnCtx { name: p.name.name.clone(), world: false, ret, locals: Vec::new(), scopes: vec![Vec::new()] };
            for (prm, ty) in p.params.iter().zip(&params) {
                if f.scopes[0].iter().any(|b| b.name == prm.name.name) {
                    self.error(Kind::Redeclared(prm.name.name.clone()), prm.name.span);
                }
                declare(&mut f, &prm.name, *ty);
            }
            let body = self.block(&mut f, &p.body.stmts);
            self.pop_scope(&mut f);
            if let Some(ty) = ret
                && !hir::block_returns(&body)
            {
                self.error(Kind::MissingFlag { pipe: f.name.clone(), ty }, p.name.span);
            }
            funcs.push(hir::Func { name: f.name, world: None, params: params.len(), locals: f.locals, ret, body });
        }
        for w in worlds {
            let name = format!("world_{}_{}", w.major, w.minor);
            let mut f = FnCtx { name, world: true, ret: None, locals: Vec::new(), scopes: Vec::new() };
            let body = self.block(&mut f, &w.body.stmts);
            funcs.push(hir::Func {
                name: f.name,
                world: Some((w.major, w.minor)),
                params: 0,
                locals: f.locals,
                ret: None,
                body,
            });
        }
        funcs
    }

    // ------------------------------------------------------------------ statements

    fn block(&mut self, f: &mut FnCtx, stmts: &[ast::Stmt]) -> Vec<Stmt> {
        f.scopes.push(Vec::new());
        let mut out = Vec::new();
        let (mut dead, mut warned) = (false, false);
        for s in stmts {
            let h = self.stmt(f, s);
            if dead && !warned {
                self.warning(Kind::Unreachable, s.span);
                warned = true;
            }
            // Unreachable statements are still checked and kept: removing them is the optimizer's job.
            dead = dead || hir::always_returns(&h);
            out.push(h);
        }
        self.pop_scope(f);
        out
    }

    fn pop_scope(&mut self, f: &mut FnCtx) {
        for b in f.scopes.pop().unwrap_or_default() {
            if !b.used && !b.name.starts_with('_') {
                self.warning(Kind::Unused(b.name), b.span);
            }
        }
    }

    fn stmt(&mut self, f: &mut FnCtx, s: &ast::Stmt) -> Stmt {
        let line = self.lines.line(s.span.start);
        let kind = match &s.kind {
            S::Coin { name, ty, value } => {
                let v = self.expr(f, value);
                let ty = match ty {
                    Some(t) => {
                        let declared = self.resolve_type(t);
                        if !declared.accepts(v.ty) {
                            let ctx = Ctx::Annotation(name.name.clone());
                            self.error(Kind::Mismatch { expected: declared, found: v.ty, ctx }, value.span);
                        }
                        declared
                    }
                    None => v.ty,
                };
                let scope = f.scopes.last().expect("a block scope");
                if scope.iter().any(|b| b.name == name.name) {
                    self.error(Kind::Redeclared(name.name.clone()), name.span);
                }
                let local = declare(f, name, ty);
                StmtKind::Set(local, v)
            }
            S::Assign { name, value } => {
                let v = self.expr(f, value);
                match self.lookup(f, name, false) {
                    Some(local) => {
                        let ty = f.locals[local as usize].ty;
                        if !ty.accepts(v.ty) {
                            let ctx = Ctx::Assign(name.name.clone());
                            self.error(Kind::Mismatch { expected: ty, found: v.ty, ctx }, value.span);
                        }
                        StmtKind::Set(local, v)
                    }
                    None => StmtKind::Eval(v),
                }
            }
            S::Counter { up, name, by } => {
                // `power_up x by n` is sugar for `x = x + n` (and `damage` for `x = x - n`).
                let amount = match by {
                    Some(e) => {
                        let v = self.expr(f, e);
                        if !Ty::Coins.accepts(v.ty) {
                            self.error(Kind::Mismatch { expected: Ty::Coins, found: v.ty, ctx: Ctx::Counter }, e.span);
                        }
                        v
                    }
                    None => Expr::int(1),
                };
                match self.lookup(f, name, true) {
                    Some(local) => {
                        let ty = f.locals[local as usize].ty;
                        if !Ty::Coins.accepts(ty) {
                            self.error(Kind::Mismatch { expected: Ty::Coins, found: ty, ctx: Ctx::Counter }, name.span);
                        }
                        let op = if *up { BinOp::Add } else { BinOp::Sub };
                        let current = Expr { kind: H::Local(local), ty: Ty::Coins };
                        let sum = Expr { kind: H::Binary(op, Box::new(current), Box::new(amount)), ty: Ty::Coins };
                        StmtKind::Set(local, sum)
                    }
                    None => StmtKind::Eval(amount),
                }
            }
            S::Question { cond, then, otherwise } => {
                let c = self.condition(f, cond, Ctx::Question);
                let a = self.block(f, &then.stmts);
                let b = otherwise.as_ref().map(|b| self.block(f, &b.stmts)).unwrap_or_default();
                StmtKind::If(c, a, b)
            }
            S::Bounce { cond, body } => {
                let c = self.condition(f, cond, Ctx::Bounce);
                let b = self.block(f, &body.stmts);
                StmtKind::While(c, b)
            }
            S::Run { var, from, to, body } => {
                let from_v = self.expr(f, from);
                let to_v = self.expr(f, to);
                for (v, e) in [(&from_v, from), (&to_v, to)] {
                    if !Ty::Coins.accepts(v.ty) {
                        self.error(Kind::Mismatch { expected: Ty::Coins, found: v.ty, ctx: Ctx::RunBound }, e.span);
                    }
                }
                f.scopes.push(Vec::new());
                let local = declare(f, var, Ty::Coins);
                f.scopes.last_mut().unwrap()[0].used = true; // a loop counter may go unused on purpose
                let limit = f.locals.len() as u32;
                f.locals.push(hir::Local { name: format!("{}_limit", var.name), ty: Ty::Coins });
                let b = self.block(f, &body.stmts);
                self.pop_scope(f);
                StmtKind::For { var: local, limit, from: from_v, to: to_v, body: b }
            }
            S::Flag { value } => {
                let v = value.as_ref().map(|e| (self.expr(f, e), e.span));
                match (f.world, f.ret, v) {
                    (true, _, Some((_, span))) => {
                        self.error(Kind::FlagValueInWorld, span);
                        StmtKind::Return(None)
                    }
                    (false, None, Some((_, span))) => {
                        self.error(Kind::FlagValueInPlainPipe(f.name.clone()), span);
                        StmtKind::Return(None)
                    }
                    (false, Some(ty), None) => {
                        self.error(Kind::FlagNeedsValue { pipe: f.name.clone(), ty }, s.span);
                        StmtKind::Return(None)
                    }
                    (false, Some(ty), Some((v, span))) => {
                        if !ty.accepts(v.ty) {
                            let ctx = Ctx::Flag(f.name.clone());
                            self.error(Kind::Mismatch { expected: ty, found: v.ty, ctx }, span);
                        }
                        StmtKind::Return(Some(v))
                    }
                    (_, _, None) => StmtKind::Return(None),
                }
            }
            S::Call(e) => match &e.kind {
                A::Call { callee, args } if callee.name == PRINT => {
                    StmtKind::Print(args.iter().map(|a| self.expr(f, a)).collect())
                }
                A::Call { callee, args } => StmtKind::Eval(self.call(f, callee, args, e.span, true)),
                _ => StmtKind::Eval(self.expr(f, e)), // already reported by the parser
            },
        };
        Stmt { kind, line }
    }

    fn condition(&mut self, f: &mut FnCtx, e: &ast::Expr, ctx: Ctx) -> Expr {
        let v = self.expr(f, e);
        if !Ty::Switch.accepts(v.ty) {
            self.error(Kind::Mismatch { expected: Ty::Switch, found: v.ty, ctx }, e.span);
        }
        v
    }

    fn lookup(&mut self, f: &mut FnCtx, name: &ast::Ident, mark_used: bool) -> Option<u32> {
        for scope in f.scopes.iter_mut().rev() {
            if let Some(b) = scope.iter_mut().rev().find(|b| b.name == name.name) {
                b.used |= mark_used;
                return Some(b.local);
            }
        }
        let visible: Vec<&str> = f.scopes.iter().flatten().map(|b| b.name.as_str()).collect();
        let suggestion = suggest(&name.name, visible);
        self.error(Kind::UnknownName { name: name.name.clone(), suggestion }, name.span);
        None
    }

    // ------------------------------------------------------------------ expressions

    fn expr(&mut self, f: &mut FnCtx, e: &ast::Expr) -> Expr {
        let error = Expr { kind: H::Int(0), ty: Ty::Error };
        match &e.kind {
            A::Int(n) => Expr::int(*n),
            A::Bool(b) => Expr::bool(*b),
            A::Text(s) => Expr { kind: H::Text(self.intern(s)), ty: Ty::Text },
            A::Name(n) => {
                let ident = ast::Ident { name: n.clone(), span: e.span };
                match self.lookup(f, &ident, true) {
                    Some(local) => Expr { kind: H::Local(local), ty: f.locals[local as usize].ty },
                    None => error,
                }
            }
            A::Unary(op, x) => {
                let v = self.expr(f, x);
                let want = if *op == UnOp::Neg { Ty::Coins } else { Ty::Switch };
                if !want.accepts(v.ty) {
                    self.error(Kind::UnaryTypes { op: op.symbol(), ty: v.ty }, e.span);
                }
                Expr { kind: H::Unary(*op, Box::new(v)), ty: want }
            }
            A::Binary(op, l, r) => {
                let a = self.expr(f, l);
                let b = self.expr(f, r);
                let (want, out) = match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => (Some(Ty::Coins), Ty::Coins),
                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => (Some(Ty::Coins), Ty::Switch),
                    BinOp::And | BinOp::Or => (Some(Ty::Switch), Ty::Switch),
                    BinOp::Eq | BinOp::Ne => (None, Ty::Switch),
                };
                let ok = match want {
                    Some(t) => t.accepts(a.ty) && t.accepts(b.ty),
                    None => a.ty.accepts(b.ty),
                };
                if !ok {
                    self.error(Kind::BinaryTypes { op: op.symbol(), left: a.ty, right: b.ty }, e.span);
                }
                if matches!(op, BinOp::Div | BinOp::Rem) && b.kind == H::Int(0) {
                    self.warning(Kind::DivByZero, r.span);
                }
                Expr { kind: H::Binary(*op, Box::new(a), Box::new(b)), ty: if ok { out } else { Ty::Error } }
            }
            A::Call { callee, args } => self.call(f, callee, args, e.span, false),
        }
    }

    fn call(&mut self, f: &mut FnCtx, callee: &ast::Ident, args: &[ast::Expr], span: Span, as_stmt: bool) -> Expr {
        let error = Expr { kind: H::Int(0), ty: Ty::Error };
        let values: Vec<Expr> = args.iter().map(|a| self.expr(f, a)).collect();
        if callee.name == PRINT {
            self.error(Kind::PrintIsNotValue, span);
            return error;
        }
        let Some(sig) = self.sigs.get(&callee.name) else {
            let names = self.sigs.keys().map(String::as_str).chain([PRINT]);
            let suggestion = suggest(&callee.name, names);
            self.error(Kind::UnknownPipe { name: callee.name.clone(), suggestion }, callee.span);
            return error;
        };
        let (id, params, ret) = (sig.id, sig.params.clone(), sig.ret);
        if params.len() != values.len() {
            let kind = Kind::ArgCount { pipe: callee.name.clone(), expected: params.len(), found: values.len() };
            self.error(kind, span);
        } else {
            for (i, ((want, v), a)) in params.iter().zip(&values).zip(args).enumerate() {
                if !want.accepts(v.ty) {
                    let ctx = Ctx::Arg { pipe: callee.name.clone(), index: i };
                    self.error(Kind::Mismatch { expected: *want, found: v.ty, ctx }, a.span);
                }
            }
        }
        let ty = match ret {
            Some(t) => t,
            None if as_stmt => Ty::Error, // never read
            None => {
                self.error(Kind::NoValue(callee.name.clone()), span);
                Ty::Error
            }
        };
        Expr { kind: H::Call(id, values), ty }
    }
}

fn declare(f: &mut FnCtx, name: &ast::Ident, ty: Ty) -> u32 {
    let local = f.locals.len() as u32;
    f.locals.push(hir::Local { name: name.name.clone(), ty });
    if f.scopes.is_empty() {
        f.scopes.push(Vec::new());
    }
    f.scopes.last_mut().unwrap().push(Binding { name: name.name.clone(), local, used: false, span: name.span });
    local
}

#[cfg(test)]
mod tests {
    use crate::diagnostics::Kind;

    fn kinds(src: &str) -> Vec<Kind> {
        crate::compile(src, &crate::Options::default()).diagnostics.into_iter().map(|d| d.kind).collect()
    }

    #[test]
    fn unknown_name_suggests_a_close_one() {
        let ks = kinds("world 1-1 {\n coin lives = 3\n wahoo(lifes)\n}");
        assert!(ks.contains(&Kind::UnknownName { name: "lifes".into(), suggestion: Some("lives".into()) }), "{ks:?}");
    }

    #[test]
    fn type_aliases_from_other_languages() {
        let ks = kinds("pipe f(n: int) -> bool { flag n > 0 }\nworld 1-1 { wahoo(f(1)) }");
        assert!(ks.contains(&Kind::UnknownType { name: "int".into(), suggestion: Some("coins".into()) }), "{ks:?}");
        assert!(ks.contains(&Kind::UnknownType { name: "bool".into(), suggestion: Some("switch".into()) }), "{ks:?}");
    }

    #[test]
    fn errors_do_not_cascade() {
        // One unknown name, used in arithmetic and a comparison: a single error.
        let ks = kinds("world 1-1 {\n question nope + 1 > 2 { wahoo(1) }\n}");
        assert_eq!(ks.len(), 1, "{ks:?}");
    }

    #[test]
    fn missing_flag_on_some_path() {
        let ks = kinds("pipe f(n: coins) -> coins {\n question n > 0 { flag 1 }\n}\nworld 1-1 { wahoo(f(1)) }");
        assert!(matches!(ks[0], Kind::MissingFlag { .. }), "{ks:?}");
        let ok = kinds(
            "pipe f(n: coins) -> coins {\n question n > 0 { flag 1 } else { flag 2 }\n}\nworld 1-1 { wahoo(f(1)) }",
        );
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn scopes_shadowing_and_redeclaration() {
        let ok = kinds("world 1-1 {\n coin x = 1\n question star { coin x = \"inner\"\n wahoo(x) }\n wahoo(x)\n}");
        assert!(ok.is_empty(), "{ok:?}");
        let ks = kinds("world 1-1 {\n coin x = 1\n coin x = 2\n wahoo(x)\n}");
        assert!(ks.contains(&Kind::Redeclared("x".into())), "{ks:?}");
    }

    #[test]
    fn warnings() {
        let ks = kinds("pipe f() -> coins {\n flag 1\n wahoo(2)\n}\nworld 1-1 {\n coin unused = 1\n wahoo(f() / 0)\n}");
        assert!(ks.contains(&Kind::Unreachable), "{ks:?}");
        assert!(ks.contains(&Kind::Unused("unused".into())), "{ks:?}");
        assert!(ks.contains(&Kind::DivByZero), "{ks:?}");
    }
}
