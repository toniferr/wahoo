//! A small optimizer working on the typed HIR, before code generation.
//!
//! - **Constant folding**: `2 * 60 + 5` becomes `125` at compile time, with the same wrap-around arithmetic the
//!   machine would use. Divisions that would trap (by zero, or `-2147483648 / -1`) are left alone so the program
//!   still fails at run time, where the language says it fails.
//! - **Algebraic simplification**: `x + 0`, `x * 1`, `not not s`, `star and s`, ...
//! - **Branch elimination**: `question star { A } else { B }` becomes `A`; `bounce goomba { ... }` disappears.
//! - **Dead code elimination**: statements after a `flag` are dropped.
//!
//! Every rewrite preserves behaviour: an expression is only thrown away when it is pure (no calls, no traps).

use crate::ast::{BinOp, UnOp};
use crate::hir::{self, Expr, ExprKind as H, Stmt, StmtKind};
use crate::types::Ty;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub folded: usize,
    pub branches: usize,
    pub dead: usize,
}

pub fn optimize(program: &mut hir::Program) -> Stats {
    let mut stats = Stats::default();
    for f in &mut program.funcs {
        let body = std::mem::take(&mut f.body);
        f.body = block(body, &mut stats);
    }
    stats
}

fn block(stmts: Vec<Stmt>, st: &mut Stats) -> Vec<Stmt> {
    let mut out: Vec<Stmt> = Vec::new();
    let mut dead = false;
    for s in stmts {
        if dead {
            st.dead += 1;
            continue;
        }
        let line = s.line;
        match s.kind {
            StmtKind::If(c, a, b) => {
                let c = expr(c, st);
                let a = block(a, st);
                let b = block(b, st);
                match c.kind {
                    H::Bool(keep_then) => {
                        st.branches += 1;
                        out.extend(if keep_then { a } else { b });
                    }
                    _ => out.push(Stmt { kind: StmtKind::If(c, a, b), line }),
                }
            }
            StmtKind::While(c, body) => {
                let c = expr(c, st);
                if c.kind == H::Bool(false) {
                    st.branches += 1;
                } else {
                    out.push(Stmt { kind: StmtKind::While(c, block(body, st)), line });
                }
            }
            StmtKind::For { var, limit, from, to, body } => {
                let (from, to) = (expr(from, st), expr(to, st));
                match (&from.kind, &to.kind) {
                    (H::Int(a), H::Int(b)) if a > b => st.branches += 1,
                    _ => out.push(Stmt { kind: StmtKind::For { var, limit, from, to, body: block(body, st) }, line }),
                }
            }
            StmtKind::Set(l, e) => out.push(Stmt { kind: StmtKind::Set(l, expr(e, st)), line }),
            StmtKind::Return(e) => out.push(Stmt { kind: StmtKind::Return(e.map(|e| expr(e, st))), line }),
            StmtKind::Eval(e) => out.push(Stmt { kind: StmtKind::Eval(expr(e, st)), line }),
            StmtKind::Print(args) => {
                out.push(Stmt { kind: StmtKind::Print(args.into_iter().map(|e| expr(e, st)).collect()), line })
            }
        }
        dead = out.iter().any(hir::always_returns);
    }
    out
}

fn expr(e: Expr, st: &mut Stats) -> Expr {
    let ty = e.ty;
    match e.kind {
        H::Unary(op, x) => {
            let x = expr(*x, st);
            let folded = match (op, &x.kind) {
                (UnOp::Neg, H::Int(n)) => Some(Expr::int(n.wrapping_neg())),
                (UnOp::Not, H::Bool(b)) => Some(Expr::bool(!b)),
                (UnOp::Not, H::Unary(UnOp::Not, inner)) => Some((**inner).clone()),
                _ => None,
            };
            match folded {
                Some(f) => {
                    st.folded += 1;
                    f
                }
                None => Expr { kind: H::Unary(op, Box::new(x)), ty },
            }
        }
        H::Binary(op, a, b) => {
            let (a, b) = (expr(*a, st), expr(*b, st));
            match binary(op, &a, &b) {
                Some(f) => {
                    st.folded += 1;
                    f
                }
                None => Expr { kind: H::Binary(op, Box::new(a), Box::new(b)), ty },
            }
        }
        H::Call(id, args) => Expr { kind: H::Call(id, args.into_iter().map(|a| expr(a, st)).collect()), ty },
        kind => Expr { kind, ty },
    }
}

fn binary(op: BinOp, a: &Expr, b: &Expr) -> Option<Expr> {
    use BinOp::*;
    // Both sides known: compute now.
    match (&a.kind, &b.kind) {
        (H::Int(x), H::Int(y)) => {
            let (x, y) = (*x, *y);
            return match op {
                Add => Some(Expr::int(x.wrapping_add(y))),
                Sub => Some(Expr::int(x.wrapping_sub(y))),
                Mul => Some(Expr::int(x.wrapping_mul(y))),
                Div | Rem if y == 0 || (x == i32::MIN && y == -1) => None,
                Div => Some(Expr::int(x / y)),
                Rem => Some(Expr::int(x % y)),
                Eq => Some(Expr::bool(x == y)),
                Ne => Some(Expr::bool(x != y)),
                Lt => Some(Expr::bool(x < y)),
                Le => Some(Expr::bool(x <= y)),
                Gt => Some(Expr::bool(x > y)),
                Ge => Some(Expr::bool(x >= y)),
                And | Or => None,
            };
        }
        (H::Bool(x), H::Bool(y)) if matches!(op, Eq | Ne) => return Some(Expr::bool((x == y) == (op == Eq))),
        // Each text literal is stored once, so equal texts are the same literal.
        (H::Text(x), H::Text(y)) if matches!(op, Eq | Ne) => return Some(Expr::bool((x == y) == (op == Eq))),
        _ => {}
    }
    // One side known: identities. Short-circuit rules may skip the right side because it would not have run.
    let keep = |e: &Expr| Some(e.clone());
    match (op, &a.kind, &b.kind) {
        (And, H::Bool(true), _) => keep(b),
        (And, H::Bool(false), _) => Some(Expr::bool(false)),
        (And, _, H::Bool(true)) => keep(a),
        (And, _, H::Bool(false)) if a.is_pure() => Some(Expr::bool(false)),
        (Or, H::Bool(true), _) => Some(Expr::bool(true)),
        (Or, H::Bool(false), _) => keep(b),
        (Or, _, H::Bool(false)) => keep(a),
        (Or, _, H::Bool(true)) if a.is_pure() => Some(Expr::bool(true)),
        (Add, H::Int(0), _) => keep(b),
        (Add | Sub, _, H::Int(0)) => keep(a),
        (Mul, H::Int(1), _) => keep(b),
        (Mul | Div, _, H::Int(1)) => keep(a),
        (Mul, H::Int(0), _) if b.is_pure() => Some(Expr::int(0)),
        (Mul, _, H::Int(0)) if a.is_pure() => Some(Expr::int(0)),
        (Sub, H::Local(x), H::Local(y)) if x == y && a.ty == Ty::Coins => Some(Expr::int(0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{Options, compile};

    fn wat(src: &str, optimize: bool) -> String {
        let c = compile(src, &Options { optimize });
        assert!(c.ok(), "{:?}", c.diagnostics);
        c.wat().unwrap()
    }

    #[test]
    fn folds_constants() {
        let src = "world 1-1 { wahoo(2 * 60 + 5) }";
        assert!(wat(src, true).contains("i32.const 125"));
        assert!(wat(src, false).contains("i32.mul"));
    }

    #[test]
    fn keeps_trapping_division() {
        assert!(wat("world 1-1 { wahoo(1 / 0) }", true).contains("i32.div_s"));
    }

    #[test]
    fn removes_dead_branches_and_code() {
        let src =
            "pipe f() -> coins {\n question 1 < 2 { flag 1 } else { flag 2 }\n wahoo(3)\n}\nworld 1-1 { wahoo(f()) }";
        let out = wat(src, true);
        assert!(!out.contains("i32.const 2\n"), "{out}");
        assert!(!out.contains("i32.const 3"), "{out}");
    }
}
