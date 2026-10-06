//! Lexical analysis: source text → tokens.
//!
//! A hand-written scanner. It walks the text once, character by character, always taking the longest token that
//! matches at the current position ("maximal munch": `<=` is one token, not `<` then `=`). Words are scanned as
//! identifiers and then looked up in the keyword table. Errors don't stop the scan: the bad character is reported
//! and skipped, so one typo doesn't hide the rest of the program.

use crate::diagnostics::{Diagnostic, Kind};
use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Int(i32),
    Text(String),
    Ident(String),

    // Keywords
    World,
    Pipe,
    Coin,
    PowerUp,
    Damage,
    By,
    Question,
    Else,
    Bounce,
    Run,
    From,
    To,
    Flag,
    Star,
    Goomba,
    And,
    Or,
    Not,

    // Symbols
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Arrow,
    Assign,
    Plus,
    Minus,
    Times,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,

    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// True when this is the first token on its line. The parser uses it so that a line break ends a `flag` and
    /// never turns `name` + newline + `(…)` into a call.
    pub line_start: bool,
}

pub const KEYWORDS: &[(&str, TokenKind)] = &[
    ("world", TokenKind::World),
    ("pipe", TokenKind::Pipe),
    ("coin", TokenKind::Coin),
    ("power_up", TokenKind::PowerUp),
    ("damage", TokenKind::Damage),
    ("by", TokenKind::By),
    ("question", TokenKind::Question),
    ("else", TokenKind::Else),
    ("bounce", TokenKind::Bounce),
    ("run", TokenKind::Run),
    ("from", TokenKind::From),
    ("to", TokenKind::To),
    ("flag", TokenKind::Flag),
    ("star", TokenKind::Star),
    ("goomba", TokenKind::Goomba),
    ("and", TokenKind::And),
    ("or", TokenKind::Or),
    ("not", TokenKind::Not),
];

impl TokenKind {
    /// The token class shown in token listings.
    pub fn class(&self) -> &'static str {
        match self {
            TokenKind::Int(_) => "number",
            TokenKind::Text(_) => "text",
            TokenKind::Ident(_) => "name",
            TokenKind::Eof => "eof",
            k if k.keyword().is_some() => "keyword",
            _ => "symbol",
        }
    }

    pub fn keyword(&self) -> Option<&'static str> {
        KEYWORDS.iter().find(|(_, k)| k == self).map(|(w, _)| *w)
    }

    /// Source spelling of keywords and symbols.
    pub fn spelling(&self) -> Option<&'static str> {
        if let Some(k) = self.keyword() {
            return Some(k);
        }
        Some(match self {
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::Arrow => "->",
            TokenKind::Assign => "=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Times => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::EqEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::Le => "<=",
            TokenKind::Gt => ">",
            TokenKind::Ge => ">=",
            _ => return None,
        })
    }
}

pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(src: &str) -> Lexed {
    let mut lx = Lexer { src, pos: 0, tokens: Vec::new(), diagnostics: Vec::new(), line_start: true };
    lx.run();
    Lexed { tokens: lx.tokens, diagnostics: lx.diagnostics }
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
    line_start: bool,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.src[self.pos..].chars();
        it.next();
        it.next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token { kind, span: Span::new(start, self.pos), line_start: self.line_start });
        self.line_start = false;
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            let start = self.pos;
            match c {
                '\n' => {
                    self.bump();
                    self.line_start = true;
                }
                c if c.is_whitespace() => {
                    self.bump();
                }
                '/' if self.peek2() == Some('/') => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                }
                '0'..='9' => self.number(start),
                c if c.is_ascii_alphabetic() || c == '_' => self.word(start),
                '"' => self.text(start),
                _ => self.symbol(start, c),
            }
        }
        let end = self.src.len();
        self.tokens.push(Token { kind: TokenKind::Eof, span: Span::new(end, end), line_start: true });
    }

    fn number(&mut self, start: usize) {
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            self.bump();
        }
        // A letter glued to a number (`3lives`) is almost certainly a typo, not two tokens.
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
            self.bump();
        }
        let raw = &self.src[start..self.pos];
        let digits: String = raw.chars().filter(|c| *c != '_').collect();
        let value = match digits.parse::<i64>() {
            Ok(v) if v <= i32::MAX as i64 => v as i32,
            Ok(_) => {
                self.error(Kind::NumberTooBig(raw.to_string()), start);
                0
            }
            Err(_) if digits.chars().all(|c| c.is_ascii_digit()) => {
                self.error(Kind::NumberTooBig(raw.to_string()), start);
                0
            }
            Err(_) => {
                self.error(Kind::BadNumber(raw.to_string()), start);
                0
            }
        };
        self.push(TokenKind::Int(value), start);
    }

    fn word(&mut self, start: usize) {
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            self.bump();
        }
        let word = &self.src[start..self.pos];
        let kind = KEYWORDS
            .iter()
            .find(|(w, _)| *w == word)
            .map(|(_, k)| k.clone())
            .unwrap_or_else(|| TokenKind::Ident(word.to_string()));
        self.push(kind, start);
    }

    fn text(&mut self, start: usize) {
        self.bump(); // opening quote
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    self.error(Kind::UnterminatedText, start);
                    break;
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    let esc_start = self.pos;
                    self.bump();
                    match self.bump() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some(c) => {
                            self.error(Kind::BadEscape(c), esc_start);
                        }
                        None => {
                            self.error(Kind::UnterminatedText, start);
                            break;
                        }
                    }
                }
                Some(c) => {
                    self.bump();
                    value.push(c);
                }
            }
        }
        self.push(TokenKind::Text(value), start);
    }

    fn symbol(&mut self, start: usize, c: char) {
        self.bump();
        let next = self.peek();
        let two = |lx: &mut Self, kind| {
            lx.bump();
            kind
        };
        let kind = match (c, next) {
            ('-', Some('>')) => two(self, TokenKind::Arrow),
            ('=', Some('=')) => two(self, TokenKind::EqEq),
            ('!', Some('=')) => two(self, TokenKind::NotEq),
            ('<', Some('=')) => two(self, TokenKind::Le),
            ('>', Some('=')) => two(self, TokenKind::Ge),
            ('(', _) => TokenKind::LParen,
            (')', _) => TokenKind::RParen,
            ('{', _) => TokenKind::LBrace,
            ('}', _) => TokenKind::RBrace,
            (',', _) => TokenKind::Comma,
            (':', _) => TokenKind::Colon,
            ('=', _) => TokenKind::Assign,
            ('+', _) => TokenKind::Plus,
            ('-', _) => TokenKind::Minus,
            ('*', _) => TokenKind::Times,
            ('/', _) => TokenKind::Slash,
            ('%', _) => TokenKind::Percent,
            ('<', _) => TokenKind::Lt,
            ('>', _) => TokenKind::Gt,
            _ => {
                self.error(Kind::UnexpectedChar(c), start);
                return;
            }
        };
        self.push(kind, start);
    }

    fn error(&mut self, kind: Kind, start: usize) {
        self.diagnostics.push(Diagnostic::error(kind, Span::new(start, self.pos.max(start + 1))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn maximal_munch() {
        use TokenKind::*;
        assert_eq!(kinds("a<=b->c"), vec![Ident("a".into()), Le, Ident("b".into()), Arrow, Ident("c".into()), Eof]);
    }

    #[test]
    fn keywords_and_names() {
        use TokenKind::*;
        assert_eq!(
            kinds("coin coins power_up powerup"),
            vec![Coin, Ident("coins".into()), PowerUp, Ident("powerup".into()), Eof]
        );
    }

    #[test]
    fn text_escapes_and_comments() {
        use TokenKind::*;
        assert_eq!(kinds("\"a\\n\\\"b\" // hi\n1_000"), vec![Text("a\n\"b".into()), Int(1000), Eof]);
    }

    #[test]
    fn line_start_flags() {
        let toks = lex("a b\n c").tokens;
        let flags: Vec<bool> = toks.iter().map(|t| t.line_start).collect();
        assert_eq!(flags, vec![true, false, true, true]);
    }

    #[test]
    fn errors_do_not_stop_the_scan() {
        let out = lex("coin x = 3 @ 99999999999 \"open");
        assert_eq!(out.diagnostics.len(), 3);
        assert!(out.tokens.iter().any(|t| t.kind == TokenKind::Text("open".into())));
    }
}
