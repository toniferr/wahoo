//! A tiny WebAssembly toolkit: an in-memory module, its binary encoding and its text form (WAT).
//!
//! Only what Wahoo needs: `i32` values, functions, imports, one linear memory and data segments. The binary
//! format is a magic number and a version followed by numbered sections (types, imports, functions, memory,
//! exports, code, data), every integer written in LEB128, a variable-length encoding where small numbers take
//! one byte. A custom `name` section carries function and local names, so tools and stack traces can show them.

use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValType {
    I32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
    Empty,
    I32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    Unreachable,
    Block,
    Loop,
    If(BlockType),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    Return,
    Call(u32),
    Drop,
    LocalGet(u32),
    LocalSet(u32),
    I32Const(i32),
    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32GtS,
    I32LeS,
    I32GeS,
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32RemS,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    pub module: String,
    pub field: String,
    pub name: String,
    pub ty: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub ty: u32,
    pub params: Vec<String>,
    /// Extra locals (all `i32`), after the parameters.
    pub locals: Vec<String>,
    pub body: Vec<Instr>,
    /// Source comments shown in the WAT before the instruction at the given index.
    pub notes: Vec<(usize, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Func,
    Memory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    pub offset: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub types: Vec<FuncType>,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
    pub memory_pages: Option<u32>,
    pub exports: Vec<Export>,
    pub data: Vec<Data>,
}

impl Module {
    /// Index of a function type, adding it if it is new (identical signatures share one entry).
    pub fn type_index(&mut self, params: usize, results: usize) -> u32 {
        let ft = FuncType { params: vec![ValType::I32; params], results: vec![ValType::I32; results] };
        match self.types.iter().position(|t| *t == ft) {
            Some(i) => i as u32,
            None => {
                self.types.push(ft);
                (self.types.len() - 1) as u32
            }
        }
    }

    /// Function names by index: imports first, then defined functions (that is how Wasm numbers them).
    fn func_names(&self) -> Vec<&str> {
        self.imports.iter().map(|i| i.name.as_str()).chain(self.functions.iter().map(|f| f.name.as_str())).collect()
    }

    // ------------------------------------------------------------------ binary

    pub fn encode(&self) -> Vec<u8> {
        let mut out = b"\0asm".to_vec();
        out.extend(1u32.to_le_bytes());

        section(
            &mut out,
            1,
            &vec_of(&self.types, |b, t| {
                b.push(0x60);
                vec_bytes(b, &t.params.iter().map(|v| valtype(*v)).collect::<Vec<_>>());
                vec_bytes(b, &t.results.iter().map(|v| valtype(*v)).collect::<Vec<_>>());
            }),
        );
        section(
            &mut out,
            2,
            &vec_of(&self.imports, |b, i| {
                name(b, &i.module);
                name(b, &i.field);
                b.push(0x00); // function import
                uleb(b, i.ty as u64);
            }),
        );
        section(&mut out, 3, &vec_of(&self.functions, |b, f| uleb(b, f.ty as u64)));
        if let Some(pages) = self.memory_pages {
            let mut mem = vec![0x01, 0x00]; // one memory, limits with a minimum only
            uleb(&mut mem, pages as u64);
            section(&mut out, 5, &mem);
        }
        section(
            &mut out,
            7,
            &vec_of(&self.exports, |b, e| {
                name(b, &e.name);
                b.push(match e.kind {
                    ExportKind::Func => 0x00,
                    ExportKind::Memory => 0x02,
                });
                uleb(b, e.index as u64);
            }),
        );
        section(
            &mut out,
            10,
            &vec_of(&self.functions, |b, f| {
                let mut body = Vec::new();
                if f.locals.is_empty() {
                    uleb(&mut body, 0);
                } else {
                    uleb(&mut body, 1); // one run of locals...
                    uleb(&mut body, f.locals.len() as u64); // ...this many...
                    body.push(valtype(ValType::I32)); // ...of type i32
                }
                for ins in &f.body {
                    encode_instr(&mut body, *ins);
                }
                body.push(0x0B); // end of the function body
                uleb(b, body.len() as u64);
                b.extend(body);
            }),
        );
        if !self.data.is_empty() {
            section(
                &mut out,
                11,
                &vec_of(&self.data, |b, d| {
                    b.push(0x00); // active segment in memory 0
                    b.push(0x41); // i32.const offset
                    sleb(b, d.offset as i64);
                    b.push(0x0B);
                    vec_bytes(b, &d.bytes);
                }),
            );
        }

        // Custom "name" section: 1 = function names, 2 = local names.
        let names = self.func_names();
        let mut custom = Vec::new();
        name(&mut custom, "name");
        let mut funcs = Vec::new();
        uleb(&mut funcs, names.len() as u64);
        for (i, n) in names.iter().enumerate() {
            uleb(&mut funcs, i as u64);
            name(&mut funcs, n);
        }
        custom.push(1);
        uleb(&mut custom, funcs.len() as u64);
        custom.extend(funcs);
        let mut locals = Vec::new();
        uleb(&mut locals, self.functions.len() as u64);
        for (i, f) in self.functions.iter().enumerate() {
            uleb(&mut locals, (self.imports.len() + i) as u64);
            let all: Vec<&String> = f.params.iter().chain(&f.locals).collect();
            uleb(&mut locals, all.len() as u64);
            for (j, n) in all.iter().enumerate() {
                uleb(&mut locals, j as u64);
                name(&mut locals, n);
            }
        }
        custom.push(2);
        uleb(&mut custom, locals.len() as u64);
        custom.extend(locals);
        section(&mut out, 0, &custom);
        out
    }

    // ------------------------------------------------------------------ text

    pub fn to_wat(&self) -> String {
        let names = self.func_names();
        let mut w = String::from("(module\n");
        for (i, t) in self.types.iter().enumerate() {
            let _ = writeln!(w, "  (type (;{i};) (func{}))", signature(&[], t));
        }
        for imp in &self.imports {
            let _ =
                writeln!(w, "  (import \"{}\" \"{}\" (func ${} (type {})))", imp.module, imp.field, imp.name, imp.ty);
        }
        if let Some(pages) = self.memory_pages {
            let _ = writeln!(w, "  (memory (;0;) {pages})");
        }
        for e in &self.exports {
            let target = match e.kind {
                ExportKind::Func => format!("func ${}", names[e.index as usize]),
                ExportKind::Memory => format!("memory {}", e.index),
            };
            let _ = writeln!(w, "  (export \"{}\" ({target}))", e.name);
        }
        for f in &self.functions {
            let t = &self.types[f.ty as usize];
            let _ = writeln!(w, "  (func ${}{}", f.name, signature(&f.params, t));
            for l in &f.locals {
                let _ = writeln!(w, "    (local ${l} i32)");
            }
            let all: Vec<&String> = f.params.iter().chain(&f.locals).collect();
            let mut depth = 2;
            let mut notes = f.notes.iter().peekable();
            for (i, ins) in f.body.iter().enumerate() {
                while let Some((_, note)) = notes.next_if(|(at, _)| *at == i) {
                    let _ = writeln!(w, "{};; {note}", "  ".repeat(depth));
                }
                if matches!(ins, Instr::End | Instr::Else) {
                    depth -= 1;
                }
                let _ = writeln!(w, "{}{}", "  ".repeat(depth), instr_text(*ins, &names, &all));
                if matches!(ins, Instr::Block | Instr::Loop | Instr::If(_) | Instr::Else) {
                    depth += 1;
                }
            }
            w.push_str("  )\n");
        }
        for d in &self.data {
            let _ = writeln!(w, "  (data (i32.const {}) \"{}\")", d.offset, wat_string(&d.bytes));
        }
        w.push_str(")\n");
        w
    }
}

fn signature(params: &[String], t: &FuncType) -> String {
    let mut s = String::new();
    for (i, _) in t.params.iter().enumerate() {
        match params.get(i) {
            Some(n) => {
                let _ = write!(s, " (param ${n} i32)");
            }
            None => s.push_str(" (param i32)"),
        }
    }
    if !t.results.is_empty() {
        s.push_str(" (result i32)");
    }
    s
}

fn wat_string(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        if (0x20..0x7F).contains(&b) && b != b'"' && b != b'\\' {
            s.push(b as char);
        } else {
            let _ = write!(s, "\\{b:02x}");
        }
    }
    s
}

pub fn instr_text(ins: Instr, funcs: &[&str], locals: &[&String]) -> String {
    let local = |i: u32| locals.get(i as usize).map_or(i.to_string(), |n| format!("${n}"));
    match ins {
        Instr::Unreachable => "unreachable".into(),
        Instr::Block => "block".into(),
        Instr::Loop => "loop".into(),
        Instr::If(BlockType::Empty) => "if".into(),
        Instr::If(BlockType::I32) => "if (result i32)".into(),
        Instr::Else => "else".into(),
        Instr::End => "end".into(),
        Instr::Br(l) => format!("br {l}"),
        Instr::BrIf(l) => format!("br_if {l}"),
        Instr::Return => "return".into(),
        Instr::Call(f) => format!("call ${}", funcs.get(f as usize).copied().unwrap_or("?")),
        Instr::Drop => "drop".into(),
        Instr::LocalGet(i) => format!("local.get {}", local(i)),
        Instr::LocalSet(i) => format!("local.set {}", local(i)),
        Instr::I32Const(n) => format!("i32.const {n}"),
        Instr::I32Eqz => "i32.eqz".into(),
        Instr::I32Eq => "i32.eq".into(),
        Instr::I32Ne => "i32.ne".into(),
        Instr::I32LtS => "i32.lt_s".into(),
        Instr::I32GtS => "i32.gt_s".into(),
        Instr::I32LeS => "i32.le_s".into(),
        Instr::I32GeS => "i32.ge_s".into(),
        Instr::I32Add => "i32.add".into(),
        Instr::I32Sub => "i32.sub".into(),
        Instr::I32Mul => "i32.mul".into(),
        Instr::I32DivS => "i32.div_s".into(),
        Instr::I32RemS => "i32.rem_s".into(),
    }
}

fn encode_instr(b: &mut Vec<u8>, ins: Instr) {
    let block_type = |bt: BlockType| match bt {
        BlockType::Empty => 0x40,
        BlockType::I32 => valtype(ValType::I32),
    };
    match ins {
        Instr::Unreachable => b.push(0x00),
        Instr::Block => b.extend([0x02, 0x40]),
        Instr::Loop => b.extend([0x03, 0x40]),
        Instr::If(bt) => b.extend([0x04, block_type(bt)]),
        Instr::Else => b.push(0x05),
        Instr::End => b.push(0x0B),
        Instr::Br(l) => {
            b.push(0x0C);
            uleb(b, l as u64);
        }
        Instr::BrIf(l) => {
            b.push(0x0D);
            uleb(b, l as u64);
        }
        Instr::Return => b.push(0x0F),
        Instr::Call(f) => {
            b.push(0x10);
            uleb(b, f as u64);
        }
        Instr::Drop => b.push(0x1A),
        Instr::LocalGet(i) => {
            b.push(0x20);
            uleb(b, i as u64);
        }
        Instr::LocalSet(i) => {
            b.push(0x21);
            uleb(b, i as u64);
        }
        Instr::I32Const(n) => {
            b.push(0x41);
            sleb(b, n as i64);
        }
        Instr::I32Eqz => b.push(0x45),
        Instr::I32Eq => b.push(0x46),
        Instr::I32Ne => b.push(0x47),
        Instr::I32LtS => b.push(0x48),
        Instr::I32GtS => b.push(0x4A),
        Instr::I32LeS => b.push(0x4C),
        Instr::I32GeS => b.push(0x4E),
        Instr::I32Add => b.push(0x6A),
        Instr::I32Sub => b.push(0x6B),
        Instr::I32Mul => b.push(0x6C),
        Instr::I32DivS => b.push(0x6D),
        Instr::I32RemS => b.push(0x6F),
    }
}

fn valtype(v: ValType) -> u8 {
    match v {
        ValType::I32 => 0x7F,
    }
}

/// Unsigned LEB128: 7 bits per byte, low bits first, high bit set on every byte but the last.
pub fn uleb(b: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            b.push(byte);
            return;
        }
        b.push(byte | 0x80);
    }
}

/// Signed LEB128: like `uleb`, stopping when the remaining bits are all copies of the sign bit.
pub fn sleb(b: &mut Vec<u8>, mut v: i64) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7; // arithmetic shift keeps the sign
        let done = (v == 0 && byte & 0x40 == 0) || (v == -1 && byte & 0x40 != 0);
        b.push(if done { byte } else { byte | 0x80 });
        if done {
            return;
        }
    }
}

fn name(b: &mut Vec<u8>, s: &str) {
    vec_bytes(b, s.as_bytes());
}

fn vec_bytes(b: &mut Vec<u8>, bytes: &[u8]) {
    uleb(b, bytes.len() as u64);
    b.extend_from_slice(bytes);
}

fn vec_of<T>(items: &[T], mut each: impl FnMut(&mut Vec<u8>, &T)) -> Vec<u8> {
    let mut b = Vec::new();
    uleb(&mut b, items.len() as u64);
    for it in items {
        each(&mut b, it);
    }
    b
}

fn section(out: &mut Vec<u8>, id: u8, content: &[u8]) {
    out.push(id);
    uleb(out, content.len() as u64);
    out.extend_from_slice(content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leb128() {
        let enc = |f: fn(&mut Vec<u8>, i64), v| {
            let mut b = Vec::new();
            f(&mut b, v);
            b
        };
        let u = |b: &mut Vec<u8>, v: i64| uleb(b, v as u64);
        assert_eq!(enc(u, 0), [0x00]);
        assert_eq!(enc(u, 127), [0x7F]);
        assert_eq!(enc(u, 128), [0x80, 0x01]);
        assert_eq!(enc(u, 624485), [0xE5, 0x8E, 0x26]);
        assert_eq!(enc(sleb, -1), [0x7F]);
        assert_eq!(enc(sleb, 63), [0x3F]);
        assert_eq!(enc(sleb, 64), [0xC0, 0x00]);
        assert_eq!(enc(sleb, -123456), [0xC0, 0xBB, 0x78]);
    }

    #[test]
    fn empty_module_is_just_the_header_and_names() {
        let bytes = Module::default().encode();
        assert_eq!(&bytes[..8], b"\0asm\x01\0\0\0");
    }
}
