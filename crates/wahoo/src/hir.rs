//! The checked program: every name resolved to a numbered local or pipe, every expression typed, and the sugar
//! (`power_up`, `damage`, `else question`) rewritten into a handful of core statements. The optimizer and the
//! code generator only ever see this form.

use crate::ast::{BinOp, UnOp};
use crate::types::Ty;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    /// Pipes in source order, then worlds in play order (1-1, 1-2, ..., 2-1, ...).
    pub funcs: Vec<Func>,
    /// Text literals, each stored once.
    pub strings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Func {
    pub name: String,
    pub world: Option<(u32, u32)>,
    /// The first `params` locals are the parameters.
    pub params: usize,
    pub locals: Vec<Local>,
    pub ret: Option<Ty>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Local {
    pub name: String,
    pub ty: Ty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    /// 1-based source line, for the comments in the generated WAT.
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Set(u32, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    /// `run var from .. to ..`; `limit` is a hidden local holding the upper bound, evaluated once.
    For {
        var: u32,
        limit: u32,
        from: Expr,
        to: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    /// A pipe call run for its effect (its value, if any, is dropped).
    Eval(Expr),
    /// `wahoo(a, b, ...)`
    Print(Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Int(i32),
    Bool(bool),
    Text(u32),
    Local(u32),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(u32, Vec<Expr>),
}

impl Expr {
    pub fn int(n: i32) -> Expr {
        Expr { kind: ExprKind::Int(n), ty: Ty::Coins }
    }

    pub fn bool(b: bool) -> Expr {
        Expr { kind: ExprKind::Bool(b), ty: Ty::Switch }
    }

    /// True when evaluating the expression can't have side effects (no calls), so it may be dropped.
    pub fn is_pure(&self) -> bool {
        match &self.kind {
            ExprKind::Call(..) => false,
            ExprKind::Unary(_, e) => e.is_pure(),
            ExprKind::Binary(op, a, b) => {
                // Division can trap (by zero), which is an observable effect.
                !matches!(op, BinOp::Div | BinOp::Rem) && a.is_pure() && b.is_pure()
            }
            _ => true,
        }
    }
}

/// True when every path through the statement ends in a `flag`.
pub fn always_returns(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::If(_, a, b) => block_returns(a) && block_returns(b),
        _ => false,
    }
}

pub fn block_returns(stmts: &[Stmt]) -> bool {
    stmts.iter().any(always_returns)
}
