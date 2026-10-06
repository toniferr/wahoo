//! Code generation: typed HIR → WebAssembly module.
//!
//! WebAssembly is a stack machine: `a + b * 2` becomes "push a, push b, push 2, multiply, add". Expressions are
//! emitted in post-order (operands first, then the operator), which is exactly what a stack machine wants.
//! Control flow is structured: an `if`/`else`/`end` for `question`, and a `block` wrapping a `loop` for `bounce`
//! and `run`, where `br_if 1` jumps out of the block and `br 0` back to the top of the loop.
//!
//! Memory layout: text literals live in linear memory from address 16, each stored as a 4-byte little-endian
//! length followed by its UTF-8 bytes; a `text` value is the address of its length.
//!
//! The host provides four imports (module `wahoo`): `print_coins`, `print_switch`, `print_text` and
//! `print_newline`. The module exports `memory` and `main`, which plays every world in order.

use std::collections::HashSet;

use crate::ast::{BinOp, UnOp};
use crate::hir::{self, Expr, ExprKind as H, Stmt, StmtKind};
use crate::span::line_text;
use crate::types::Ty;
use crate::wasm::{BlockType, Data, Export, ExportKind, Function, Import, Instr, Module};

pub const IMPORT_MODULE: &str = "wahoo";
pub const IMPORTS: [(&str, usize); 4] =
    [("print_coins", 1), ("print_switch", 1), ("print_text", 1), ("print_newline", 0)];
const PRINT_COINS: u32 = 0;
const PRINT_SWITCH: u32 = 1;
const PRINT_TEXT: u32 = 2;
const PRINT_NEWLINE: u32 = 3;
pub const DATA_START: u32 = 16;

pub fn generate(program: &hir::Program, src: &str) -> Module {
    let mut m = Module { memory_pages: Some(1), ..Module::default() };
    let mut names = NameSet::default();
    for (field, params) in IMPORTS {
        let ty = m.type_index(params, 0);
        m.imports.push(Import { module: IMPORT_MODULE.into(), field: field.into(), name: names.unique(field), ty });
    }

    // Text literals → data segment.
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    for s in &program.strings {
        offsets.push(DATA_START + bytes.len() as u32);
        bytes.extend((s.len() as u32).to_le_bytes());
        bytes.extend(s.as_bytes());
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
    }
    if !bytes.is_empty() {
        m.data.push(Data { offset: DATA_START, bytes });
    }

    let first = IMPORTS.len() as u32;
    for f in &program.funcs {
        let ty = m.type_index(f.params, usize::from(f.ret.is_some()));
        let mut locals = NameSet::default();
        let all: Vec<String> = f.locals.iter().map(|l| locals.unique(&l.name)).collect();
        let mut g = FnGen { code: Vec::new(), notes: Vec::new(), last_line: 0, src, offsets: &offsets, first };
        g.block(&f.body);
        if f.ret.is_some() && g.code.last() != Some(&Instr::Return) {
            // The checker proved every path returns; tell the validator nothing falls off the end.
            g.code.push(Instr::Unreachable);
        }
        m.functions.push(Function {
            name: names.unique(&f.name),
            ty,
            params: all[..f.params].to_vec(),
            locals: all[f.params..].to_vec(),
            body: g.code,
            notes: g.notes,
        });
    }

    // main: play the worlds in order.
    let mut body = Vec::new();
    let mut notes = Vec::new();
    for (i, f) in program.funcs.iter().enumerate() {
        if let Some((a, b)) = f.world {
            notes.push((body.len(), format!("world {a}-{b}")));
            body.push(Instr::Call(first + i as u32));
        }
    }
    let ty = m.type_index(0, 0);
    let main_index = first + m.functions.len() as u32;
    m.functions.push(Function { name: names.unique("main"), ty, params: vec![], locals: vec![], body, notes });
    m.exports.push(Export { name: "memory".into(), kind: ExportKind::Memory, index: 0 });
    m.exports.push(Export { name: "main".into(), kind: ExportKind::Func, index: main_index });
    m
}

/// Hands out unique WAT identifiers (`x`, `x_2`, ...) for shadowed names.
#[derive(Default)]
struct NameSet(HashSet<String>);

impl NameSet {
    fn unique(&mut self, base: &str) -> String {
        let mut name = base.to_string();
        let mut n = 2;
        while self.0.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        self.0.insert(name.clone());
        name
    }
}

struct FnGen<'a> {
    code: Vec<Instr>,
    notes: Vec<(usize, String)>,
    last_line: usize,
    src: &'a str,
    offsets: &'a [u32],
    first: u32,
}

impl FnGen<'_> {
    fn emit(&mut self, ins: Instr) {
        self.code.push(ins);
    }

    fn note(&mut self, line: usize) {
        if line == self.last_line {
            return; // several statements on one line: one comment
        }
        self.last_line = line;
        let text = line_text(self.src, line).trim();
        let text: String =
            if text.chars().count() > 60 { text.chars().take(57).chain("...".chars()).collect() } else { text.into() };
        self.notes.push((self.code.len(), format!("{line}: {text}")));
    }

    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        self.note(s.line);
        match &s.kind {
            StmtKind::Set(local, e) => {
                self.expr(e);
                self.emit(Instr::LocalSet(*local));
            }
            StmtKind::If(c, a, b) => {
                self.expr(c);
                self.emit(Instr::If(BlockType::Empty));
                self.block(a);
                if !b.is_empty() {
                    self.emit(Instr::Else);
                    self.block(b);
                }
                self.emit(Instr::End);
            }
            StmtKind::While(c, body) => {
                self.emit(Instr::Block);
                self.emit(Instr::Loop);
                self.expr(c);
                self.emit(Instr::I32Eqz);
                self.emit(Instr::BrIf(1)); // condition false: leave the block
                self.block(body);
                self.emit(Instr::Br(0)); // back to the top of the loop
                self.emit(Instr::End);
                self.emit(Instr::End);
            }
            StmtKind::For { var, limit, from, to, body } => {
                self.expr(from);
                self.emit(Instr::LocalSet(*var));
                self.expr(to);
                self.emit(Instr::LocalSet(*limit));
                self.emit(Instr::Block);
                self.emit(Instr::Loop);
                self.emit(Instr::LocalGet(*var));
                self.emit(Instr::LocalGet(*limit));
                self.emit(Instr::I32GtS);
                self.emit(Instr::BrIf(1)); // past the limit: leave
                self.block(body);
                self.emit(Instr::LocalGet(*var));
                self.emit(Instr::I32Const(1));
                self.emit(Instr::I32Add);
                self.emit(Instr::LocalSet(*var));
                self.emit(Instr::Br(0));
                self.emit(Instr::End);
                self.emit(Instr::End);
            }
            StmtKind::Return(e) => {
                if let Some(e) = e {
                    self.expr(e);
                }
                self.emit(Instr::Return);
            }
            StmtKind::Eval(e) => {
                self.expr(e);
                if e.ty != Ty::Error {
                    self.emit(Instr::Drop); // a value nobody uses
                }
            }
            StmtKind::Print(args) => {
                for a in args {
                    self.expr(a);
                    self.emit(Instr::Call(match a.ty {
                        Ty::Switch => PRINT_SWITCH,
                        Ty::Text => PRINT_TEXT,
                        _ => PRINT_COINS,
                    }));
                }
                self.emit(Instr::Call(PRINT_NEWLINE));
            }
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            H::Int(n) => self.emit(Instr::I32Const(*n)),
            H::Bool(b) => self.emit(Instr::I32Const(i32::from(*b))),
            H::Text(id) => self.emit(Instr::I32Const(self.offsets[*id as usize] as i32)),
            H::Local(l) => self.emit(Instr::LocalGet(*l)),
            H::Unary(UnOp::Neg, x) => {
                self.emit(Instr::I32Const(0));
                self.expr(x);
                self.emit(Instr::I32Sub);
            }
            H::Unary(UnOp::Not, x) => {
                self.expr(x);
                self.emit(Instr::I32Eqz);
            }
            // Short-circuit: the right side only runs when it can change the result.
            H::Binary(BinOp::And, a, b) => {
                self.expr(a);
                self.emit(Instr::If(BlockType::I32));
                self.expr(b);
                self.emit(Instr::Else);
                self.emit(Instr::I32Const(0));
                self.emit(Instr::End);
            }
            H::Binary(BinOp::Or, a, b) => {
                self.expr(a);
                self.emit(Instr::If(BlockType::I32));
                self.emit(Instr::I32Const(1));
                self.emit(Instr::Else);
                self.expr(b);
                self.emit(Instr::End);
            }
            H::Binary(op, a, b) => {
                self.expr(a);
                self.expr(b);
                self.emit(match op {
                    BinOp::Add => Instr::I32Add,
                    BinOp::Sub => Instr::I32Sub,
                    BinOp::Mul => Instr::I32Mul,
                    BinOp::Div => Instr::I32DivS,
                    BinOp::Rem => Instr::I32RemS,
                    BinOp::Eq => Instr::I32Eq,
                    BinOp::Ne => Instr::I32Ne,
                    BinOp::Lt => Instr::I32LtS,
                    BinOp::Le => Instr::I32LeS,
                    BinOp::Gt => Instr::I32GtS,
                    BinOp::Ge => Instr::I32GeS,
                    BinOp::And | BinOp::Or => unreachable!(),
                });
            }
            H::Call(id, args) => {
                for a in args {
                    self.expr(a);
                }
                self.emit(Instr::Call(self.first + id));
            }
        }
    }
}
