//! # Wahoo
//!
//! A compiler for Wahoo, a tiny language from the Mushroom Kingdom, to WebAssembly. Every phase of a classic
//! compiler is here, small enough to read in an evening:
//!
//! ```text
//! source ─▶ lexer ─▶ tokens ─▶ parser ─▶ AST ─▶ checker ─▶ HIR ─▶ optimizer ─▶ codegen ─▶ Wasm module
//!          lexer.rs          parser.rs        checker.rs       optimizer.rs  codegen.rs   wasm.rs
//! ```
//!
//! ```
//! let src = "world 1-1 {\n  wahoo(\"Let's-a go!\")\n}\n";
//! let out = wahoo::compile(src, &wahoo::Options::default());
//! assert!(out.ok());
//! let bytes = out.wasm().unwrap();
//! assert_eq!(&bytes[..4], b"\0asm");
//! ```

pub mod ast;
pub mod checker;
pub mod codegen;
pub mod diagnostics;
pub mod hir;
pub mod lexer;
pub mod optimizer;
pub mod parser;
pub mod span;
pub mod types;
pub mod views;
pub mod wasm;

pub use diagnostics::{Diagnostic, Lang, Severity};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Run the optimizer (constant folding, branch and dead code elimination).
    pub optimize: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { optimize: true }
    }
}

/// The result of a compiler run: diagnostics, and the module when there were no errors.
pub struct Compilation {
    pub diagnostics: Vec<Diagnostic>,
    pub module: Option<wasm::Module>,
    pub stats: Option<optimizer::Stats>,
}

impl Compilation {
    pub fn ok(&self) -> bool {
        self.module.is_some()
    }

    pub fn wasm(&self) -> Option<Vec<u8>> {
        self.module.as_ref().map(wasm::Module::encode)
    }

    pub fn wat(&self) -> Option<String> {
        self.module.as_ref().map(wasm::Module::to_wat)
    }
}

fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(Diagnostic::is_error)
}

fn sorted(mut diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diags.sort_by_key(|d| d.span.start);
    diags
}

/// Lex and parse only (the front end's syntax half). The program is returned even with errors, as far as it
/// could be parsed.
pub fn parse(src: &str) -> (ast::Program, Vec<Diagnostic>) {
    let lexed = lexer::lex(src);
    let (program, mut diags) = parser::parse(&lexed.tokens);
    diags.extend(lexed.diagnostics);
    (program, sorted(diags))
}

pub fn compile(src: &str, options: &Options) -> Compilation {
    let fail = |diagnostics| Compilation { diagnostics, module: None, stats: None };
    let (program, diags) = parse(src);
    if has_errors(&diags) {
        // Checking a half-parsed program mostly produces noise: stop at syntax errors.
        return fail(diags);
    }
    let (mut hir, more) = checker::check(&program, src);
    let mut diags = diags;
    diags.extend(more);
    let diags = sorted(diags);
    if has_errors(&diags) {
        return fail(diags);
    }
    let stats = options.optimize.then(|| optimizer::optimize(&mut hir));
    let module = codegen::generate(&hir, src);
    Compilation { diagnostics: diags, module: Some(module), stats }
}
