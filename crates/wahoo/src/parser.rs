//! Syntax analysis: tokens → abstract syntax tree.
//!
//! Statements are parsed by recursive descent: one function per grammar rule, choosing the rule from the next
//! token (Wahoo's grammar is LL(1) at statement level: every statement starts with its own keyword or a name).
//! Expressions use Pratt parsing ("top-down operator precedence"): each infix operator has a left and a right
//! binding power, and a single loop builds the tree with the right precedence and associativity.
//!
//! ```text
//! program  := item*
//! item     := 'pipe' NAME '(' [param {',' param}] ')' ['->' NAME] block
//!           | 'world' INT '-' INT block
//! param    := NAME ':' NAME
//! block    := '{' stmt* '}'
//! stmt     := 'coin' NAME [':' NAME] '=' expr
//!           | ('power_up' | 'damage') NAME ['by' expr]
//!           | 'question' expr block ['else' (question | block)]
//!           | 'bounce' expr block
//!           | 'run' NAME 'from' expr 'to' expr block
//!           | 'flag' [expr]
//!           | NAME '=' expr
//!           | call
//! expr     := or            (lowest precedence first)
//! or       := and {'or' and}
//! and      := not {'and' not}
//! not      := 'not' not | cmp
//! cmp      := sum [('=='|'!='|'<'|'<='|'>'|'>=') sum]     (not chainable)
//! sum      := term {('+'|'-') term}
//! term     := unary {('*'|'/'|'%') unary}
//! unary    := '-' unary | primary
//! primary  := INT | TEXT | 'star' | 'goomba' | call | NAME | '(' expr ')'
//! call     := NAME '(' [expr {',' expr}] ')'
//! ```
//!
//! On a syntax error the parser reports it and then skips ahead to the next line that starts a statement
//! ("panic-mode recovery"), so a single run can report several independent mistakes.

use crate::ast::*;
use crate::diagnostics::{Diagnostic, Expected, Kind};
use crate::lexer::{Token, TokenKind};
use crate::span::Span;

const MAX_ERRORS: usize = 20;

pub fn parse(tokens: &[Token]) -> (Program, Vec<Diagnostic>) {
    let mut p = Parser { toks: tokens, pos: 0, diags: Vec::new() };
    let program = p.program();
    (program, p.diags)
}

/// Marker for "an error was reported; unwind to a recovery point".
struct Bail;
type PResult<T> = Result<T, Bail>;

struct Parser<'t> {
    toks: &'t [Token],
    pos: usize,
    diags: Vec<Diagnostic>,
}

/// Binding powers of the infix operators: (operator, left power, right power).
/// Left-associative operators have right power = left power + 1.
fn infix(kind: &TokenKind) -> Option<(BinOp, u8, u8)> {
    Some(match kind {
        TokenKind::Or => (BinOp::Or, 1, 2),
        TokenKind::And => (BinOp::And, 3, 4),
        TokenKind::EqEq => (BinOp::Eq, 7, 8),
        TokenKind::NotEq => (BinOp::Ne, 7, 8),
        TokenKind::Lt => (BinOp::Lt, 7, 8),
        TokenKind::Le => (BinOp::Le, 7, 8),
        TokenKind::Gt => (BinOp::Gt, 7, 8),
        TokenKind::Ge => (BinOp::Ge, 7, 8),
        TokenKind::Plus => (BinOp::Add, 9, 10),
        TokenKind::Minus => (BinOp::Sub, 9, 10),
        TokenKind::Times => (BinOp::Mul, 11, 12),
        TokenKind::Slash => (BinOp::Div, 11, 12),
        TokenKind::Percent => (BinOp::Rem, 11, 12),
        _ => return None,
    })
}

const NOT_POWER: u8 = 5;
const NEG_POWER: u8 = 13;

fn starts_stmt(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Coin
            | TokenKind::PowerUp
            | TokenKind::Damage
            | TokenKind::Question
            | TokenKind::Bounce
            | TokenKind::Run
            | TokenKind::Flag
            | TokenKind::Ident(_)
    )
}

fn describe(tok: &Token) -> String {
    match &tok.kind {
        TokenKind::Int(n) => format!("`{n}`"),
        TokenKind::Text(s) => format!("\"{}\"", s.replace('\n', "\\n")),
        TokenKind::Ident(n) => format!("`{n}`"),
        TokenKind::Eof => String::new(),
        k => format!("`{}`", k.spelling().unwrap_or("?")),
    }
}

impl Parser<'_> {
    // ------------------------------------------------------------------ token helpers

    fn peek(&self) -> &Token {
        &self.toks[self.pos]
    }

    fn peek_at(&self, n: usize) -> &Token {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)]
    }

    fn at(&self, kind: &TokenKind) -> bool {
        &self.peek().kind == kind
    }

    fn advance(&mut self) -> Token {
        let tok = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn report(&mut self, kind: Kind, span: Span) {
        if self.diags.len() >= MAX_ERRORS {
            return;
        }
        self.diags.push(Diagnostic::error(kind, span));
        if self.diags.len() == MAX_ERRORS {
            self.diags.push(Diagnostic::error(Kind::TooManyErrors, span));
            self.pos = self.toks.len() - 1; // jump to Eof: every loop ends
        }
    }

    fn expected(&mut self, expected: Expected) -> Bail {
        let tok = self.peek().clone();
        self.report(Kind::Expected { expected, found: describe(&tok) }, tok.span);
        Bail
    }

    fn expect(&mut self, kind: TokenKind) -> PResult<Span> {
        if self.at(&kind) {
            Ok(self.advance().span)
        } else {
            Err(self.expected(Expected::Token(kind.spelling().unwrap_or("?"))))
        }
    }

    fn ident(&mut self, what: Expected) -> PResult<Ident> {
        match &self.peek().kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = self.advance().span;
                Ok(Ident { name, span })
            }
            _ => Err(self.expected(what)),
        }
    }

    fn prev_span(&self) -> Span {
        self.toks[self.pos.saturating_sub(1)].span
    }

    // ------------------------------------------------------------------ items

    fn program(&mut self) -> Program {
        let mut items = Vec::new();
        while !self.at(&TokenKind::Eof) {
            let item = match self.peek().kind {
                TokenKind::Pipe => self.pipe().map(Item::Pipe),
                TokenKind::World => self.world().map(Item::World),
                _ => Err(self.expected(Expected::Item)),
            };
            match item {
                Ok(item) => items.push(item),
                Err(Bail) => {
                    // Skip to the next item.
                    self.advance();
                    while !matches!(self.peek().kind, TokenKind::Pipe | TokenKind::World | TokenKind::Eof) {
                        self.advance();
                    }
                }
            }
        }
        Program { items }
    }

    fn pipe(&mut self) -> PResult<PipeDecl> {
        self.expect(TokenKind::Pipe)?;
        let name = self.ident(Expected::Name)?;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                let pname = self.ident(Expected::Name)?;
                self.expect(TokenKind::Colon)?;
                let ty = self.ident(Expected::Type)?;
                params.push(Param { name: pname, ty });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;
        let ret = if self.eat(&TokenKind::Arrow) { Some(self.ident(Expected::Type)?) } else { None };
        let body = self.block()?;
        Ok(PipeDecl { name, params, ret, body })
    }

    fn world(&mut self) -> PResult<WorldDecl> {
        let start = self.expect(TokenKind::World)?;
        let major = self.world_part()?;
        if !self.eat(&TokenKind::Minus) {
            return Err(self.expected(Expected::WorldNumber));
        }
        let minor = self.world_part()?;
        let span = start.to(self.prev_span());
        let body = self.block()?;
        Ok(WorldDecl { major, minor, span, body })
    }

    fn world_part(&mut self) -> PResult<u32> {
        match self.peek().kind {
            TokenKind::Int(n) => {
                self.advance();
                Ok(n as u32)
            }
            _ => Err(self.expected(Expected::WorldNumber)),
        }
    }

    // ------------------------------------------------------------------ statements

    fn block(&mut self) -> PResult<Block> {
        let open = self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        loop {
            let tok = self.peek();
            match tok.kind {
                TokenKind::RBrace => break,
                TokenKind::Eof => {
                    self.expected(Expected::Token("}"));
                    return Ok(Block { stmts, span: open.to(self.prev_span()) });
                }
                // A new item at the start of a line: the `}` of this block is missing.
                TokenKind::Pipe | TokenKind::World if tok.line_start => {
                    self.expected(Expected::Token("}"));
                    return Ok(Block { stmts, span: open.to(self.prev_span()) });
                }
                _ => {}
            }
            let start = self.pos;
            match self.stmt() {
                Ok(s) => stmts.push(s),
                Err(Bail) => self.sync(start),
            }
        }
        let close = self.advance().span;
        Ok(Block { stmts, span: open.to(close) })
    }

    /// Panic-mode recovery: skip to the next line that starts a statement, or to the `}` closing this block.
    fn sync(&mut self, start: usize) {
        let mut depth = 0usize;
        loop {
            let tok = self.peek();
            match tok.kind {
                TokenKind::Eof => return,
                TokenKind::RBrace if depth == 0 => return,
                TokenKind::RBrace => depth -= 1,
                TokenKind::LBrace => depth += 1,
                TokenKind::Pipe | TokenKind::World if depth == 0 && tok.line_start => return,
                ref k if depth == 0 && tok.line_start && self.pos > start && starts_stmt(k) => return,
                _ => {}
            }
            self.advance();
        }
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.peek().span;
        let kind = match self.peek().kind.clone() {
            TokenKind::Coin => {
                self.advance();
                let name = self.ident(Expected::Name)?;
                let ty = if self.eat(&TokenKind::Colon) { Some(self.ident(Expected::Type)?) } else { None };
                self.expect(TokenKind::Assign)?;
                let value = self.expr()?;
                StmtKind::Coin { name, ty, value }
            }
            TokenKind::PowerUp | TokenKind::Damage => {
                let up = self.advance().kind == TokenKind::PowerUp;
                let name = self.ident(Expected::Name)?;
                let by = if self.eat(&TokenKind::By) { Some(self.expr()?) } else { None };
                StmtKind::Counter { up, name, by }
            }
            TokenKind::Question => return self.question(),
            TokenKind::Bounce => {
                self.advance();
                let cond = self.expr()?;
                let body = self.block()?;
                StmtKind::Bounce { cond, body }
            }
            TokenKind::Run => {
                self.advance();
                let var = self.ident(Expected::Name)?;
                self.expect(TokenKind::From)?;
                let from = self.expr()?;
                self.expect(TokenKind::To)?;
                let to = self.expr()?;
                let body = self.block()?;
                StmtKind::Run { var, from, to, body }
            }
            TokenKind::Flag => {
                self.advance();
                let next = self.peek();
                let value = if next.line_start || next.kind == TokenKind::RBrace { None } else { Some(self.expr()?) };
                StmtKind::Flag { value }
            }
            TokenKind::Else => {
                let span = self.advance().span;
                self.report(Kind::ElseWithoutQuestion, span);
                return Err(Bail);
            }
            TokenKind::Ident(_) if self.peek_at(1).kind == TokenKind::Assign => {
                let name = self.ident(Expected::Name)?;
                self.advance();
                let value = self.expr()?;
                StmtKind::Assign { name, value }
            }
            _ => {
                let e = self.expr()?;
                if !matches!(e.kind, ExprKind::Call { .. }) {
                    self.report(Kind::StandaloneExpr, e.span);
                }
                StmtKind::Call(e)
            }
        };
        Ok(Stmt { kind, span: start.to(self.prev_span()) })
    }

    fn question(&mut self) -> PResult<Stmt> {
        let start = self.expect(TokenKind::Question)?;
        let cond = self.expr()?;
        let then = self.block()?;
        let otherwise = if self.eat(&TokenKind::Else) {
            if self.at(&TokenKind::Question) {
                let nested = self.question()?;
                let span = nested.span;
                Some(Block { stmts: vec![nested], span })
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        Ok(Stmt { kind: StmtKind::Question { cond, then, otherwise }, span: start.to(self.prev_span()) })
    }

    // ------------------------------------------------------------------ expressions (Pratt)

    fn expr(&mut self) -> PResult<Expr> {
        self.expr_bp(0)
    }

    fn expr_bp(&mut self, min_power: u8) -> PResult<Expr> {
        let mut lhs = self.prefix()?;
        while let Some((op, left, right)) = infix(&self.peek().kind) {
            if left < min_power {
                break;
            }
            self.advance();
            let rhs = self.expr_bp(right)?;
            if op.is_comparison()
                && let Some((next, _, _)) = infix(&self.peek().kind)
                && next.is_comparison()
            {
                let span = lhs.span.to(self.peek().span);
                self.report(Kind::ChainedComparison, span);
                return Err(Bail);
            }
            let span = lhs.span.to(rhs.span);
            lhs = Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), span };
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> PResult<Expr> {
        let tok = self.peek().clone();
        let (op, power) = match tok.kind {
            TokenKind::Not => (UnOp::Not, NOT_POWER),
            TokenKind::Minus => (UnOp::Neg, NEG_POWER),
            _ => return self.primary(),
        };
        self.advance();
        let operand = self.expr_bp(power)?;
        let span = tok.span.to(operand.span);
        Ok(Expr { kind: ExprKind::Unary(op, Box::new(operand)), span })
    }

    fn primary(&mut self) -> PResult<Expr> {
        let tok = self.peek().clone();
        let kind = match tok.kind {
            TokenKind::Int(n) => ExprKind::Int(n),
            TokenKind::Text(s) => ExprKind::Text(s),
            TokenKind::Star => ExprKind::Bool(true),
            TokenKind::Goomba => ExprKind::Bool(false),
            TokenKind::Ident(name) => {
                let next = self.peek_at(1);
                if next.kind == TokenKind::LParen && !next.line_start {
                    return self.call();
                }
                ExprKind::Name(name)
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.expr()?;
                let close = self.expect(TokenKind::RParen)?;
                return Ok(Expr { kind: inner.kind, span: tok.span.to(close) });
            }
            _ => return Err(self.expected(Expected::Expr)),
        };
        self.advance();
        Ok(Expr { kind, span: tok.span })
    }

    fn call(&mut self) -> PResult<Expr> {
        let callee = self.ident(Expected::Name)?;
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                args.push(self.expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let close = self.expect(TokenKind::RParen)?;
        let span = callee.span.to(close);
        Ok(Expr { kind: ExprKind::Call { callee, args }, span })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_src(src: &str) -> (Program, Vec<Diagnostic>) {
        parse(&lex(src).tokens)
    }

    /// Fully parenthesised rendering of the first expression statement of world 1-1.
    fn shape(src: &str) -> String {
        fn show(e: &Expr) -> String {
            match &e.kind {
                ExprKind::Int(n) => n.to_string(),
                ExprKind::Name(n) => n.clone(),
                ExprKind::Bool(b) => b.to_string(),
                ExprKind::Text(t) => format!("{t:?}"),
                ExprKind::Unary(op, x) => format!("({} {})", op.symbol(), show(x)),
                ExprKind::Binary(op, a, b) => format!("({} {} {})", show(a), op.symbol(), show(b)),
                ExprKind::Call { callee, args } => {
                    format!("{}({})", callee.name, args.iter().map(show).collect::<Vec<_>>().join(", "))
                }
            }
        }
        let (prog, diags) = parse_src(&format!("world 1-1 {{ coin x = {src} }}"));
        assert!(diags.is_empty(), "{diags:?}");
        let Item::World(w) = &prog.items[0] else { panic!() };
        let StmtKind::Coin { value, .. } = &w.body.stmts[0].kind else { panic!() };
        show(value)
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(shape("1 + 2 * 3"), "(1 + (2 * 3))");
        assert_eq!(shape("10 - 4 - 3"), "((10 - 4) - 3)");
        assert_eq!(shape("-a * b"), "((- a) * b)");
        assert_eq!(shape("a < b + 1 and not c or d"), "(((a < (b + 1)) and (not c)) or d)");
        assert_eq!(shape("not a == b"), "(not (a == b))");
        assert_eq!(shape("(1 + 2) * f(3, 4)"), "((1 + 2) * f(3, 4))");
    }

    #[test]
    fn chained_comparison_is_an_error() {
        let (_, diags) = parse_src("world 1-1 { coin x = 1 < 2 < 3 }");
        assert!(matches!(diags[0].kind, Kind::ChainedComparison));
    }

    #[test]
    fn recovers_and_reports_several_errors() {
        let src = "world 1-1 {\n  coin = 3\n  coin y = 4\n  wahoo(y +)\n  wahoo(y)\n}\n";
        let (prog, diags) = parse_src(src);
        assert_eq!(diags.len(), 2, "{diags:?}");
        let Item::World(w) = &prog.items[0] else { panic!() };
        assert_eq!(w.body.stmts.len(), 2); // `coin y = 4` and the last wahoo
    }

    #[test]
    fn missing_closing_brace_before_next_item() {
        let (prog, diags) = parse_src("pipe f() {\n  wahoo(1)\nworld 1-1 {\n  f()\n}\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(prog.items.len(), 2);
    }

    #[test]
    fn flag_ends_at_line_break() {
        let (prog, diags) = parse_src("world 1-1 {\n  flag\n  wahoo(1)\n}");
        assert!(diags.is_empty());
        let Item::World(w) = &prog.items[0] else { panic!() };
        assert_eq!(w.body.stmts.len(), 2);
    }
}
