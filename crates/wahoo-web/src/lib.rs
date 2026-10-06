//! The Wahoo compiler, itself compiled to WebAssembly, for the browser playground: a compiler running inside the
//! very virtual machine it targets.
//!
//! No wasm-bindgen: the interface is a handful of C-ABI functions over linear memory.
//!
//! ```text
//! p = alloc(n)                          reserve n bytes, write the UTF-8 source there
//! status = compile(p, n, task, flags)   0 = ok, 1 = the program has errors
//! out_ptr(), out_len()                  where the result is (valid until the next call)
//! dealloc(p, n)
//! ```
//!
//! Tasks: 0 tokens, 1 syntax tree, 2 check + WAT, 3 the `.wasm` binary (or diagnostics if there are errors).
//! Results are JSON except task 3 on success. Flags: bit 0 = optimize, bit 1 = messages in Spanish.

use std::cell::RefCell;

use wahoo::views::{self, json_str};
use wahoo::{Lang, Options};

pub const TASK_TOKENS: u32 = 0;
pub const TASK_AST: u32 = 1;
pub const TASK_WAT: u32 = 2;
pub const TASK_WASM: u32 = 3;

/// Runs one task. Returns the status and the result bytes.
pub fn run_task(src: &str, task: u32, flags: u32) -> (u32, Vec<u8>) {
    let lang = if flags & 2 != 0 { Lang::Es } else { Lang::En };
    let options = Options { optimize: flags & 1 != 0 };
    let status = |diags: &[wahoo::Diagnostic]| u32::from(diags.iter().any(|d| d.is_error()));
    match task {
        TASK_TOKENS => {
            let lexed = wahoo::lexer::lex(src);
            let json = format!(
                "{{\"diagnostics\":{},\"tokens\":{}}}",
                views::diagnostics_json(src, &lexed.diagnostics, lang),
                views::tokens_json(src, &lexed.tokens)
            );
            (status(&lexed.diagnostics), json.into_bytes())
        }
        TASK_AST => {
            let (program, diags) = wahoo::parse(src);
            let json = format!(
                "{{\"diagnostics\":{},\"ast\":{}}}",
                views::diagnostics_json(src, &diags, lang),
                views::tree_json(src, &views::ast_tree(&program))
            );
            (status(&diags), json.into_bytes())
        }
        TASK_WASM => {
            let c = wahoo::compile(src, &options);
            match c.wasm() {
                Some(bytes) => (0, bytes),
                None => (1, views::diagnostics_json(src, &c.diagnostics, lang).into_bytes()),
            }
        }
        _ => {
            let c = wahoo::compile(src, &options);
            let wat = c.wat().map_or("null".into(), |w| json_str(&w));
            let size = c.wasm().map_or(0, |b| b.len());
            let stats = c.stats.map_or("null".into(), |s| {
                format!("{{\"folded\":{},\"branches\":{},\"dead\":{}}}", s.folded, s.branches, s.dead)
            });
            let json = format!(
                "{{\"ok\":{},\"diagnostics\":{},\"summary\":{},\"wat\":{wat},\"size\":{size},\"stats\":{stats}}}",
                c.ok(),
                views::diagnostics_json(src, &c.diagnostics, lang),
                json_str(&wahoo::diagnostics::summary(&c.diagnostics, lang)),
            );
            (u32::from(!c.ok()), json.into_bytes())
        }
    }
}

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` and `len` must come from a previous `alloc(len)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

/// # Safety
/// `ptr..ptr+len` must be memory obtained from `alloc` and filled by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compile(ptr: *const u8, len: usize, task: u32, flags: u32) -> u32 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let src = String::from_utf8_lossy(bytes);
    let (status, out) = run_task(&src, task, flags);
    OUT.with(|o| *o.borrow_mut() = out);
    status
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const u8 {
    OUT.with(|o| o.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn out_len() -> usize {
    OUT.with(|o| o.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks() {
        let src = "world 1-1 {\n  wahoo(\"hi\")\n}\n";
        let (s, wasm) = run_task(src, TASK_WASM, 1);
        assert_eq!((s, &wasm[..4]), (0, &b"\0asm"[..]));
        let (s, json) = run_task(src, TASK_WAT, 1);
        let json = String::from_utf8(json).unwrap();
        assert_eq!(s, 0);
        assert!(json.starts_with("{\"ok\":true,") && json.contains("call $print_text"), "{json}");
        let (s, json) = run_task("world 1-1 { wahoo(x) }", TASK_WAT, 3);
        assert_eq!(s, 1);
        assert!(String::from_utf8(json).unwrap().contains("nombre desconocido"));
    }
}
