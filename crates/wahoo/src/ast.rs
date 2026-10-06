//! The abstract syntax tree: the program's structure as written, before any meaning is checked.

use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Pipe(PipeDecl),
    World(WorldDecl),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: Ident,
}

/// `pipe name(a: coins, b: coins) -> coins { ... }`
#[derive(Clone, Debug, PartialEq)]
pub struct PipeDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<Ident>,
    pub body: Block,
}

/// `world 1-2 { ... }`
#[derive(Clone, Debug, PartialEq)]
pub struct WorldDecl {
    pub major: u32,
    pub minor: u32,
    pub span: Span,
    pub body: Block,
}

impl WorldDecl {
    pub fn label(&self) -> String {
        format!("{}-{}", self.major, self.minor)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    /// `coin name: type = value` (the type is optional)
    Coin { name: Ident, ty: Option<Ident>, value: Expr },
    /// `name = value`
    Assign { name: Ident, value: Expr },
    /// `power_up name [by amount]` / `damage name [by amount]`
    Counter { up: bool, name: Ident, by: Option<Expr> },
    /// `question cond { ... } else { ... }`; `else question` is stored as an else block holding one question.
    Question { cond: Expr, then: Block, otherwise: Option<Block> },
    /// `bounce cond { ... }`
    Bounce { cond: Expr, body: Block },
    /// `run i from a to b { ... }` (both ends included)
    Run { var: Ident, from: Expr, to: Expr, body: Block },
    /// `flag [value]`
    Flag { value: Option<Expr> },
    /// A call on its own line, run for its effect.
    Call(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Int(i32),
    Text(String),
    Bool(bool),
    Name(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call { callee: Ident, args: Vec<Expr> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::Not => "not",
        }
    }
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }
}
