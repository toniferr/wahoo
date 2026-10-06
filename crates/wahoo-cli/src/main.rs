//! `wahoo`: the command-line driver. Every phase of the compiler can be looked at on its own.

use std::io::{self, Read};
use std::process::ExitCode;

use wahoo::diagnostics::summary;
use wahoo::{Lang, Options, views};

const USAGE_EN: &str = "\
wahoo — a compiler from the Mushroom Kingdom to WebAssembly

usage: wahoo <command> <file.wahoo> [options]

commands:
  run      compile and run the program
  build    compile to a .wasm file (-o <path>, default: next to the source)
  wat      print the generated WebAssembly in text form
  tokens   print the tokens the lexer sees
  ast      print the syntax tree the parser builds
  check    only look for errors and warnings

options:
  --no-opt      skip the optimizer
  --fuel <n>    stop `run` after about n instructions (endless loops)
  --lang <l>    messages in `en` or `es` (default: from $LANG)

The file can be `-` to read from standard input.";

const USAGE_ES: &str = "\
wahoo — un compilador del Reino Champiñón a WebAssembly

uso: wahoo <orden> <fichero.wahoo> [opciones]

órdenes:
  run      compila y ejecuta el programa
  build    compila a un fichero .wasm (-o <ruta>; por defecto, junto al fuente)
  wat      muestra el WebAssembly generado en formato texto
  tokens   muestra los tokens que ve el lexer
  ast      muestra el árbol sintáctico que construye el parser
  check    solo busca errores y avisos

opciones:
  --no-opt      sin optimizador
  --fuel <n>    detiene `run` tras unas n instrucciones (bucles infinitos)
  --lang <l>    mensajes en `en` o `es` (por defecto, según $LANG)

El fichero puede ser `-` para leer de la entrada estándar.";

struct Args {
    command: String,
    file: String,
    output: Option<String>,
    optimize: bool,
    fuel: Option<u64>,
    lang: Lang,
}

fn default_lang() -> Lang {
    let env = std::env::var("WAHOO_LANG").or_else(|_| std::env::var("LANG")).unwrap_or_default();
    if env.starts_with("es") { Lang::Es } else { Lang::En }
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut positional = Vec::new();
    let mut args = Args {
        command: String::new(),
        file: String::new(),
        output: None,
        optimize: true,
        fuel: None,
        lang: default_lang(),
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--no-opt" => args.optimize = false,
            "-o" | "--output" => args.output = Some(it.next().ok_or("-o needs a path")?),
            "--fuel" => {
                let n = it.next().ok_or("--fuel needs a number")?;
                args.fuel = Some(n.parse().map_err(|_| format!("--fuel: `{n}` is not a number"))?);
            }
            "--lang" => {
                args.lang = match it.next().as_deref() {
                    Some("es") => Lang::Es,
                    Some("en") => Lang::En,
                    _ => return Err("--lang must be `en` or `es`".into()),
                }
            }
            "-h" | "--help" | "help" => return Err(String::new()),
            "-V" | "--version" => {
                println!("wahoo {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            s if s.starts_with("--") => return Err(format!("unknown option `{s}`")),
            _ => positional.push(a),
        }
    }
    match positional.as_slice() {
        [cmd, file] => {
            args.command = cmd.clone();
            args.file = file.clone();
            Ok(args)
        }
        _ => Err(String::new()),
    }
}

fn read_source(path: &str) -> io::Result<String> {
    if path == "-" {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        Ok(s)
    } else {
        std::fs::read_to_string(path)
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}\n");
            }
            eprintln!("{}", if default_lang() == Lang::Es { USAGE_ES } else { USAGE_EN });
            return ExitCode::from(64);
        }
    };
    let src = match read_source(&args.file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}: {e}", args.file);
            return ExitCode::from(66);
        }
    };
    let name = if args.file == "-" { "<stdin>" } else { args.file.as_str() };

    match args.command.as_str() {
        "tokens" => {
            let lexed = wahoo::lexer::lex(&src);
            print!("{}", views::tokens_text(&src, &lexed.tokens));
            report(&src, name, &lexed.diagnostics, args.lang, false)
        }
        "ast" => {
            let (program, diags) = wahoo::parse(&src);
            print!("{}", views::tree_text(&views::ast_tree(&program)));
            report(&src, name, &diags, args.lang, false)
        }
        "check" | "wat" | "build" | "run" => {
            let c = wahoo::compile(&src, &Options { optimize: args.optimize });
            let quiet = args.command == "run" || args.command == "wat";
            let code = report(&src, name, &c.diagnostics, args.lang, !quiet);
            let Some(module) = c.module else { return code };
            match args.command.as_str() {
                "wat" => print!("{}", module.to_wat()),
                "build" => {
                    let out = args.output.clone().unwrap_or_else(|| {
                        let stem = name.strip_suffix(".wahoo").unwrap_or(name);
                        format!("{stem}.wasm")
                    });
                    let bytes = module.encode();
                    if let Err(e) = std::fs::write(&out, &bytes) {
                        eprintln!("error: {out}: {e}");
                        return ExitCode::from(73);
                    }
                    eprintln!("{out} ({} bytes)", bytes.len());
                }
                #[cfg(feature = "run")]
                "run" => {
                    if let (_, Err(e)) = wahoo_cli::run(&module.encode(), args.fuel, wahoo_cli::Output::Stdout) {
                        eprintln!("\n{}", e.message(args.lang));
                        return ExitCode::from(2);
                    }
                }
                #[cfg(not(feature = "run"))]
                "run" => {
                    eprintln!(
                        "this wahoo was built without the `run` feature: use `wahoo build` and `node scripts/run.mjs`"
                    );
                    return ExitCode::from(69);
                }
                _ => {}
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!(
                "error: unknown command `{other}`\n\n{}",
                if args.lang == Lang::Es { USAGE_ES } else { USAGE_EN }
            );
            ExitCode::from(64)
        }
    }
}

/// Prints the diagnostics to stderr. Returns failure when there were errors.
fn report(src: &str, name: &str, diags: &[wahoo::Diagnostic], lang: Lang, always_summary: bool) -> ExitCode {
    for d in diags {
        eprintln!("{}", d.render(src, name, lang));
    }
    let failed = diags.iter().any(|d| d.is_error());
    if failed || always_summary || !diags.is_empty() {
        eprintln!("{}", summary(diags, lang));
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
