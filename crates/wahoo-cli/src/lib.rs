//! Running compiled Wahoo modules: the host side of the four `wahoo.print_*` imports, on the wasmi interpreter.
#![cfg(feature = "run")]

use std::io::Write;

use wahoo::Lang;
use wasmi::{Caller, Config, Engine, Extern, Linker, Module, Store, TrapCode};

/// Why a program stopped early.
#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    /// The module failed validation or instantiation: a compiler bug.
    Invalid(String),
    /// The program trapped.
    Trap(Trap),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Trap {
    DivisionByZero,
    Overflow,
    StackOverflow,
    OutOfFuel(u64),
    Other(String),
}

impl RunError {
    pub fn message(&self, lang: Lang) -> String {
        let es = lang == Lang::Es;
        match self {
            RunError::Invalid(e) if es => format!("el módulo generado no es válido (fallo del compilador): {e}"),
            RunError::Invalid(e) => format!("the generated module is invalid (a compiler bug): {e}"),
            RunError::Trap(t) => match (t, es) {
                (Trap::DivisionByZero, false) => "You fell into a pit: division by zero".into(),
                (Trap::DivisionByZero, true) => "Has caído a un foso: división entre cero".into(),
                (Trap::Overflow, false) => "Too many coins: -2147483648 / -1 doesn't fit in 32 bits".into(),
                (Trap::Overflow, true) => "Demasiadas monedas: -2147483648 / -1 no cabe en 32 bits".into(),
                (Trap::StackOverflow, false) => "Too many pipes deep: stack overflow (endless recursion?)".into(),
                (Trap::StackOverflow, true) => {
                    "Demasiadas tuberías anidadas: desbordamiento de pila (¿recursión infinita?)".into()
                }
                (Trap::OutOfFuel(n), false) => {
                    format!("Time's up! The level ran out of time after {n} steps (endless loop?)")
                }
                (Trap::OutOfFuel(n), true) => {
                    format!("¡Se acabó el tiempo! El nivel superó los {n} pasos (¿bucle infinito?)")
                }
                (Trap::Other(e), false) => format!("the program stopped: {e}"),
                (Trap::Other(e), true) => format!("el programa se ha detenido: {e}"),
            },
        }
    }
}

/// Where a running program's output goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Straight to standard output, line by line.
    Stdout,
    /// Into a buffer, returned when the program stops.
    Capture,
}

/// The store's host state. It owns everything it touches, so it is `'static` as wasmi requires.
struct Host {
    output: Output,
    captured: Vec<u8>,
}

impl Host {
    fn emit(&mut self, s: &str) {
        match self.output {
            Output::Stdout => {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(s.as_bytes());
                if s.ends_with('\n') {
                    let _ = out.flush();
                }
            }
            Output::Capture => self.captured.extend_from_slice(s.as_bytes()),
        }
    }
}

/// Reads a `text` value: a little-endian u32 length at `ptr`, then that many UTF-8 bytes.
fn read_text(caller: &Caller<'_, Host>, ptr: i32) -> String {
    let Some(Extern::Memory(mem)) = caller.get_export("memory") else { return String::new() };
    let data = mem.data(caller);
    let p = ptr as u32 as usize;
    let Some(len) = data.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) else {
        return String::new();
    };
    data.get(p + 4..p + 4 + len).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default()
}

/// Validates, instantiates and runs a compiled module. Returns what it printed (empty unless the output is
/// captured) and how it stopped. `fuel` limits the number of executed instructions (roughly), to stop endless
/// loops.
pub fn run(wasm: &[u8], fuel: Option<u64>, output: Output) -> (String, Result<(), RunError>) {
    let mut store = None;
    let result = execute(wasm, fuel, output, &mut store);
    let captured = store.map(|s| String::from_utf8_lossy(&s.into_data().captured).into_owned()).unwrap_or_default();
    (captured, result)
}

/// Errors from validating, instantiating or linking: the module is malformed, which is a compiler bug.
fn invalid(e: impl std::fmt::Display) -> RunError {
    RunError::Invalid(e.to_string())
}

fn execute(wasm: &[u8], fuel: Option<u64>, output: Output, slot: &mut Option<Store<Host>>) -> Result<(), RunError> {
    let mut config = Config::default();
    config.consume_fuel(fuel.is_some());
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm).map_err(invalid)?;
    let store = slot.insert(Store::new(&engine, Host { output, captured: Vec::new() }));
    if let Some(f) = fuel {
        store.set_fuel(f).map_err(invalid)?;
    }

    let mut linker = <Linker<Host>>::new(&engine);
    linker
        .func_wrap("wahoo", "print_coins", |mut c: Caller<'_, Host>, n: i32| c.data_mut().emit(&n.to_string()))
        .map_err(invalid)?;
    linker
        .func_wrap("wahoo", "print_switch", |mut c: Caller<'_, Host>, b: i32| {
            c.data_mut().emit(if b != 0 { "star" } else { "goomba" })
        })
        .map_err(invalid)?;
    linker
        .func_wrap("wahoo", "print_text", |mut c: Caller<'_, Host>, ptr: i32| {
            let s = read_text(&c, ptr);
            c.data_mut().emit(&s);
        })
        .map_err(invalid)?;
    linker.func_wrap("wahoo", "print_newline", |mut c: Caller<'_, Host>| c.data_mut().emit("\n")).map_err(invalid)?;

    let instance = linker.instantiate_and_start(&mut *store, &module).map_err(invalid)?;
    let main = instance.get_typed_func::<(), ()>(&*store, "main").map_err(invalid)?;
    main.call(&mut *store, ()).map_err(|e| {
        RunError::Trap(match e.as_trap_code() {
            Some(TrapCode::IntegerDivisionByZero) => Trap::DivisionByZero,
            Some(TrapCode::IntegerOverflow) => Trap::Overflow,
            Some(TrapCode::StackOverflow) => Trap::StackOverflow,
            Some(TrapCode::OutOfFuel) => Trap::OutOfFuel(fuel.unwrap_or(0)),
            _ => Trap::Other(e.to_string()),
        })
    })
}

/// Compile and run in one go, returning everything printed (handy for tests).
pub fn run_source(src: &str, optimize: bool, fuel: Option<u64>) -> Result<String, String> {
    let c = wahoo::compile(src, &wahoo::Options { optimize });
    let Some(bytes) = c.wasm() else {
        return Err(c.diagnostics.iter().map(|d| d.render(src, "test.wahoo", Lang::En)).collect());
    };
    match run(&bytes, fuel, Output::Capture) {
        (out, Ok(())) => Ok(out),
        (out, Err(e)) => Err(format!("{out}{}", e.message(Lang::En))),
    }
}
