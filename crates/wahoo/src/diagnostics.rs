//! Errors and warnings, and how they are shown.
//!
//! Every phase reports problems as a [`Kind`] plus a [`Span`]; the words are only chosen when a diagnostic is
//! rendered, in English or Spanish. Titles are plain and precise; the short label under the code is where the
//! Mushroom Kingdom gets a say (Toad reads the characters, Lakitu films the structure, Bowser guards the types).
//!
//! Codes: E0xx lexer, E1xx parser, E2xx checker, W3xx warnings.

use crate::span::{Span, line_col, line_text};
use crate::types::Ty;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: Kind,
    pub span: Span,
}

/// What the parser was looking for.
#[derive(Clone, Debug, PartialEq)]
pub enum Expected {
    Token(&'static str),
    Expr,
    Type,
    Name,
    Item,
    WorldNumber,
}

/// Where a type mismatch happened, to explain it in context.
#[derive(Clone, Debug, PartialEq)]
pub enum Ctx {
    Annotation(String),
    Assign(String),
    Arg { pipe: String, index: usize },
    Flag(String),
    Question,
    Bounce,
    Counter,
    RunBound,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    // Lexer
    UnexpectedChar(char),
    UnterminatedText,
    BadEscape(char),
    NumberTooBig(String),
    BadNumber(String),
    // Parser
    Expected { expected: Expected, found: String },
    ChainedComparison,
    StandaloneExpr,
    ElseWithoutQuestion,
    TooManyErrors,
    // Checker
    NoWorld,
    DuplicateWorld(String),
    DuplicatePipe(String),
    ReservedName(String),
    UnknownName { name: String, suggestion: Option<String> },
    UnknownPipe { name: String, suggestion: Option<String> },
    UnknownType { name: String, suggestion: Option<String> },
    Redeclared(String),
    Mismatch { expected: Ty, found: Ty, ctx: Ctx },
    BinaryTypes { op: &'static str, left: Ty, right: Ty },
    UnaryTypes { op: &'static str, ty: Ty },
    ArgCount { pipe: String, expected: usize, found: usize },
    NoValue(String),
    PrintIsNotValue,
    MissingFlag { pipe: String, ty: Ty },
    FlagValueInWorld,
    FlagValueInPlainPipe(String),
    FlagNeedsValue { pipe: String, ty: Ty },
    // Warnings
    Unused(String),
    Unreachable,
    DivByZero,
}

impl Diagnostic {
    pub fn error(kind: Kind, span: Span) -> Self {
        Diagnostic { severity: Severity::Error, kind, span }
    }

    pub fn warning(kind: Kind, span: Span) -> Self {
        Diagnostic { severity: Severity::Warning, kind, span }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn code(&self) -> &'static str {
        use Kind::*;
        match self.kind {
            UnexpectedChar(_) => "E001",
            UnterminatedText => "E002",
            BadEscape(_) => "E003",
            NumberTooBig(_) => "E004",
            BadNumber(_) => "E005",
            Expected { .. } => "E101",
            ChainedComparison => "E102",
            StandaloneExpr => "E103",
            ElseWithoutQuestion => "E104",
            TooManyErrors => "E105",
            NoWorld => "E201",
            DuplicateWorld(_) => "E202",
            DuplicatePipe(_) => "E203",
            ReservedName(_) => "E204",
            UnknownName { .. } => "E205",
            UnknownPipe { .. } => "E206",
            UnknownType { .. } => "E207",
            Redeclared(_) => "E208",
            Mismatch { .. } => "E209",
            BinaryTypes { .. } => "E210",
            UnaryTypes { .. } => "E211",
            ArgCount { .. } => "E212",
            NoValue(_) => "E213",
            PrintIsNotValue => "E214",
            MissingFlag { .. } => "E215",
            FlagValueInWorld => "E216",
            FlagValueInPlainPipe(_) => "E217",
            FlagNeedsValue { .. } => "E218",
            Unused(_) => "W301",
            Unreachable => "W302",
            DivByZero => "W303",
        }
    }

    /// (title, label under the code, optional hint)
    pub fn texts(&self, lang: Lang) -> (String, String, Option<String>) {
        let es = lang == Lang::Es;
        let m = |en: &str, es_: &str| if es { es_.to_string() } else { en.to_string() };
        use Kind::*;
        match &self.kind {
            UnexpectedChar(c) => (
                if es { format!("carácter inesperado `{c}`") } else { format!("unexpected character `{c}`") },
                m("Toad can't read this", "Toad no sabe leer esto"),
                match c {
                    '!' => Some(m("negation is written `not`", "la negación se escribe `not`")),
                    '&' | '|' => Some(m("use the words `and` / `or`", "usa las palabras `and` / `or`")),
                    ';' => Some(m(
                        "statements end at the line break: no `;` needed",
                        "las sentencias acaban en el salto de línea: no hace falta `;`",
                    )),
                    '\'' => Some(m(
                        "text goes between double quotes: \"like this\"",
                        "el texto va entre comillas dobles: \"así\"",
                    )),
                    _ => None,
                },
            ),
            UnterminatedText => (
                m("text without its closing quote", "texto sin comilla de cierre"),
                m("this text never ends", "este texto no termina nunca"),
                Some(m("close it with `\"` on the same line", "ciérralo con `\"` en la misma línea")),
            ),
            BadEscape(c) => (
                if es {
                    format!("secuencia de escape desconocida `\\{c}`")
                } else {
                    format!("unknown escape sequence `\\{c}`")
                },
                m("Toad only knows \\n \\t \\\" \\\\", "Toad solo conoce \\n \\t \\\" \\\\"),
                None,
            ),
            NumberTooBig(n) => (
                if es {
                    format!("`{n}` monedas no caben en la cartera")
                } else {
                    format!("`{n}` coins don't fit in the wallet")
                },
                m("too big for 32 bits", "demasiado grande para 32 bits"),
                Some(m("the largest number is 2147483647", "el número más grande es 2147483647")),
            ),
            BadNumber(n) => (
                if es { format!("número mal formado `{n}`") } else { format!("malformed number `{n}`") },
                m("digits and letters got glued together", "se han pegado cifras y letras"),
                Some(m("names can't start with a digit", "los nombres no pueden empezar por una cifra")),
            ),
            Expected { expected, found } => {
                let what = match expected {
                    self::Expected::Token(t) => format!("`{t}`"),
                    self::Expected::Expr => m("an expression", "una expresión"),
                    self::Expected::Type => m("a type (coins, switch or text)", "un tipo (coins, switch o text)"),
                    self::Expected::Name => m("a name", "un nombre"),
                    self::Expected::Item => m("`pipe` or `world`", "`pipe` o `world`"),
                    self::Expected::WorldNumber => m("a world number like `1-1`", "un número de mundo como `1-1`"),
                };
                let found =
                    if found.is_empty() { m("the end of the file", "el final del fichero") } else { found.clone() };
                (
                    if es {
                        format!("se esperaba {what}, pero hay {found}")
                    } else {
                        format!("expected {what}, found {found}")
                    },
                    m("Lakitu lost the thread here", "Lakitu ha perdido el hilo aquí"),
                    match expected {
                        self::Expected::Item => Some(m(
                            "every statement lives inside a `world` or a `pipe`",
                            "toda sentencia vive dentro de un `world` o de un `pipe`",
                        )),
                        _ => None,
                    },
                )
            }
            ChainedComparison => (
                m("comparisons can't be chained", "las comparaciones no se pueden encadenar"),
                m("is this (a < b) < c ?", "¿esto es (a < b) < c ?"),
                Some(m("write `a < b and b < c`", "escribe `a < b and b < c`")),
            ),
            StandaloneExpr => (
                m("this value is computed and then thrown away", "este valor se calcula y se tira"),
                m("a lonely expression", "una expresión solitaria"),
                Some(m(
                    "only pipe calls can stand alone; to keep it use `coin name = ...`, to show it use `wahoo(...)`",
                    "solo las llamadas a pipes pueden ir solas; para guardarlo usa `coin nombre = ...`, para mostrarlo `wahoo(...)`",
                )),
            ),
            ElseWithoutQuestion => (
                m("`else` without a `question` before it", "`else` sin un `question` delante"),
                m("which question block is this for?", "¿de qué bloque question es?"),
                Some(m(
                    "`else` must follow the closing `}` of a `question`",
                    "`else` debe ir tras la `}` de un `question`",
                )),
            ),
            TooManyErrors => (
                m("too many errors, stopping here", "demasiados errores, paro aquí"),
                m("Game Over", "Game Over"),
                Some(m(
                    "fix the first ones: later errors are often a consequence",
                    "arregla los primeros: los siguientes suelen ser consecuencia",
                )),
            ),
            NoWorld => (
                m("there is no world to play", "no hay ningún mundo que jugar"),
                m("the game needs a level", "el juego necesita un nivel"),
                Some(m(
                    "add `world 1-1 { ... }`: worlds are where the program starts",
                    "añade `world 1-1 { ... }`: los mundos son donde empieza el programa",
                )),
            ),
            DuplicateWorld(w) => (
                if es { format!("el mundo {w} está repetido") } else { format!("world {w} is defined twice") },
                m("there's already a level with this number", "ya hay un nivel con este número"),
                Some(m(
                    "worlds run in order: 1-1, 1-2, ... 2-1, ...",
                    "los mundos se juegan en orden: 1-1, 1-2, ... 2-1, ...",
                )),
            ),
            DuplicatePipe(p) => (
                if es {
                    format!("la pipe `{p}` está definida dos veces")
                } else {
                    format!("pipe `{p}` is defined twice")
                },
                m("two warp pipes with the same name", "dos tuberías con el mismo nombre"),
                None,
            ),
            ReservedName(p) => (
                if es { format!("`{p}` es un nombre reservado") } else { format!("`{p}` is a reserved name") },
                m("this name belongs to the game", "este nombre es del juego"),
                Some(m(
                    "`wahoo` is the built-in that prints values",
                    "`wahoo` es la función interna que imprime valores",
                )),
            ),
            UnknownName { name, suggestion } => (
                if es { format!("nombre desconocido `{name}`") } else { format!("unknown name `{name}`") },
                m("Bowser has never heard of it", "Bowser no lo ha oído nunca"),
                suggestion
                    .as_ref()
                    .map(|s| if es { format!("¿querías decir `{s}`?") } else { format!("did you mean `{s}`?") })
                    .or_else(|| {
                        Some(m(
                            "declare it first with `coin name = value`",
                            "declárala antes con `coin nombre = valor`",
                        ))
                    }),
            ),
            UnknownPipe { name, suggestion } => (
                if es {
                    format!("no existe ninguna pipe `{name}`")
                } else {
                    format!("there is no pipe called `{name}`")
                },
                m("this pipe leads nowhere", "esta tubería no lleva a ninguna parte"),
                suggestion
                    .as_ref()
                    .map(|s| if es { format!("¿querías decir `{s}`?") } else { format!("did you mean `{s}`?") }),
            ),
            UnknownType { name, suggestion } => (
                if es { format!("tipo desconocido `{name}`") } else { format!("unknown type `{name}`") },
                m("not a type in the Mushroom Kingdom", "no es un tipo del Reino Champiñón"),
                Some(match suggestion {
                    Some(s) => {
                        if es {
                            format!("¿querías decir `{s}`?")
                        } else {
                            format!("did you mean `{s}`?")
                        }
                    }
                    None => m(
                        "the types are `coins` (whole numbers), `switch` (star/goomba) and `text`",
                        "los tipos son `coins` (enteros), `switch` (star/goomba) y `text`",
                    ),
                }),
            ),
            Redeclared(n) => (
                if es {
                    format!("`{n}` ya está declarada en este bloque")
                } else {
                    format!("`{n}` is already declared in this block")
                },
                m("a second coin with the same name", "una segunda moneda con el mismo nombre"),
                Some(if es {
                    format!("para cambiar su valor escribe `{n} = ...` sin `coin`")
                } else {
                    format!("to change its value write `{n} = ...` without `coin`")
                }),
            ),
            Mismatch { expected, found, ctx } => {
                let (e, f) = (expected.name(lang), found.name(lang));
                let title = match ctx {
                    Ctx::Annotation(n) => {
                        if es {
                            format!("`{n}` se declara como {e} pero recibe {f}")
                        } else {
                            format!("`{n}` is declared as {e} but gets {f}")
                        }
                    }
                    Ctx::Assign(n) => {
                        if es {
                            format!("`{n}` es {e} y no puede guardar {f}")
                        } else {
                            format!("`{n}` holds {e} and can't take {f}")
                        }
                    }
                    Ctx::Arg { pipe, index } => {
                        if es {
                            format!("el argumento {} de `{pipe}` debe ser {e}, no {f}", index + 1)
                        } else {
                            format!("argument {} of `{pipe}` must be {e}, not {f}", index + 1)
                        }
                    }
                    Ctx::Flag(p) => {
                        if es {
                            format!("`{p}` devuelve {e} pero este flag da {f}")
                        } else {
                            format!("`{p}` returns {e} but this flag gives {f}")
                        }
                    }
                    Ctx::Question => {
                        if es {
                            format!("la condición de un question debe ser switch, no {f}")
                        } else {
                            format!("a question needs a switch condition, not {f}")
                        }
                    }
                    Ctx::Bounce => {
                        if es {
                            format!("la condición de un bounce debe ser switch, no {f}")
                        } else {
                            format!("a bounce needs a switch condition, not {f}")
                        }
                    }
                    Ctx::Counter => {
                        if es {
                            format!("power_up y damage cuentan monedas (coins), no {f}")
                        } else {
                            format!("power_up and damage count coins, not {f}")
                        }
                    }
                    Ctx::RunBound => {
                        if es {
                            format!("los límites de un run deben ser coins, no {f}")
                        } else {
                            format!("the bounds of a run must be coins, not {f}")
                        }
                    }
                };
                let hint = match (ctx, found) {
                    (Ctx::Question | Ctx::Bounce, Ty::Coins) => {
                        Some(m("compare it: `x != 0`, `x > 0`...", "compáralo: `x != 0`, `x > 0`..."))
                    }
                    _ => None,
                };
                (title, m("Bowser blocks the way: wrong type", "Bowser corta el paso: tipo equivocado"), hint)
            }
            BinaryTypes { op, left, right } => (
                if es {
                    format!("`{op}` no funciona con {} y {}", left.name(lang), right.name(lang))
                } else {
                    format!("`{op}` doesn't work on {} and {}", left.name(lang), right.name(lang))
                },
                m("these don't mix", "esto no se mezcla"),
                if *op == "+" && (*left == Ty::Text || *right == Ty::Text) {
                    Some(m(
                        "text can't be added; print pieces together with `wahoo(a, b)`",
                        "el texto no se suma; imprime los trozos juntos con `wahoo(a, b)`",
                    ))
                } else if matches!(*op, "and" | "or") {
                    Some(m("`and` / `or` join switches (star/goomba)", "`and` / `or` unen switches (star/goomba)"))
                } else {
                    None
                },
            ),
            UnaryTypes { op, ty } => (
                if es {
                    format!("`{op}` no funciona con {}", ty.name(lang))
                } else {
                    format!("`{op}` doesn't work on {}", ty.name(lang))
                },
                m("wrong kind of value", "tipo de valor equivocado"),
                None,
            ),
            ArgCount { pipe, expected, found } => (
                if es {
                    format!("`{pipe}` recibe {expected} argumento(s), pero le pasas {found}")
                } else {
                    format!("`{pipe}` takes {expected} argument(s) but gets {found}")
                },
                m("the pipe doesn't fit", "la tubería no encaja"),
                None,
            ),
            NoValue(p) => (
                if es {
                    format!("`{p}` no devuelve ningún valor")
                } else {
                    format!("`{p}` doesn't give back a value")
                },
                m("nothing comes out of this pipe", "de esta tubería no sale nada"),
                Some(if es {
                    format!("añade `-> coins` (u otro tipo) a `pipe {p}` y termina con `flag valor`")
                } else {
                    format!("add `-> coins` (or another type) to `pipe {p}` and end it with `flag value`")
                }),
            ),
            PrintIsNotValue => (
                m("`wahoo(...)` prints, it has no value", "`wahoo(...)` imprime, no tiene valor"),
                m("Mario shouts, and that's all", "Mario grita, y nada más"),
                None,
            ),
            MissingFlag { pipe, ty } => (
                if es {
                    format!("`{pipe}` puede terminar sin llegar a un flag")
                } else {
                    format!("`{pipe}` can end without reaching a flag")
                },
                if es {
                    format!("debe devolver {}", ty.name(lang))
                } else {
                    format!("it must give back {}", ty.name(lang))
                },
                Some(m(
                    "make sure every path ends in `flag value` (an `else` often helps)",
                    "asegúrate de que todos los caminos acaban en `flag valor` (un `else` suele ayudar)",
                )),
            ),
            FlagValueInWorld => (
                m("a world's flag carries no value", "el flag de un mundo no lleva valor"),
                m("worlds don't give anything back", "los mundos no devuelven nada"),
                Some(m(
                    "write just `flag` to finish the world early",
                    "escribe solo `flag` para acabar el mundo antes",
                )),
            ),
            FlagValueInPlainPipe(p) => (
                if es {
                    format!("`{p}` no declara qué devuelve")
                } else {
                    format!("`{p}` doesn't declare what it gives back")
                },
                m("a value with nowhere to go", "un valor sin destino"),
                Some(if es {
                    format!("añade el tipo: `pipe {p}(...) -> coins`")
                } else {
                    format!("add the type: `pipe {p}(...) -> coins`")
                }),
            ),
            FlagNeedsValue { pipe, ty } => (
                if es {
                    format!("`{pipe}` debe devolver {}", ty.name(lang))
                } else {
                    format!("`{pipe}` must give back {}", ty.name(lang))
                },
                m("an empty flag", "un flag vacío"),
                Some(m("write `flag value`", "escribe `flag valor`")),
            ),
            Unused(n) => (
                if es { format!("la moneda `{n}` nunca se usa") } else { format!("coin `{n}` is never used") },
                m("Luigi noticed this one", "Luigi se ha fijado en esta"),
                Some(if es {
                    format!("bórrala, o llámala `_{n}` si es a propósito")
                } else {
                    format!("remove it, or name it `_{n}` if it's on purpose")
                }),
            ),
            Unreachable => (
                m("this code can never run", "este código no se ejecuta nunca"),
                m("after the flag the level is over", "tras el flag el nivel ha terminado"),
                None,
            ),
            DivByZero => (
                m("division by zero", "división entre cero"),
                m("this falls into a pit when it runs", "esto cae a un foso al ejecutarse"),
                None,
            ),
        }
    }

    /// A compiler-style report with the offending line and a caret underline.
    pub fn render(&self, src: &str, file: &str, lang: Lang) -> String {
        let (title, label, hint) = self.texts(lang);
        let start = line_col(src, self.span.start);
        let end = line_col(src, self.span.end.max(self.span.start));
        let sev = match (self.severity, lang) {
            (Severity::Error, _) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Warning, Lang::Es) => "aviso",
        };
        let text = line_text(src, start.line);
        let gutter = start.line.to_string().len();
        let pad = " ".repeat(gutter);
        let width = if end.line == start.line { end.col.saturating_sub(start.col).max(1) } else { 1 };
        let caret_pad: String = text.chars().take(start.col - 1).map(|c| if c == '\t' { '\t' } else { ' ' }).collect();
        let mut out = format!(
            "{sev}[{}]: {title}\n{pad}--> {file}:{}:{}\n{pad} |\n{} | {text}\n{pad} | {caret_pad}{} {label}\n",
            self.code(),
            start.line,
            start.col,
            start.line,
            "^".repeat(width),
        );
        if let Some(h) = hint {
            let word = if lang == Lang::Es { "pista" } else { "hint" };
            out.push_str(&format!("{pad} = {word}: {h}\n"));
        }
        out
    }
}

/// "Course clear!" or "Game Over" line that closes a compiler run.
pub fn summary(diags: &[Diagnostic], lang: Lang) -> String {
    let errors = diags.iter().filter(|d| d.is_error()).count();
    let warnings = diags.len() - errors;
    let es = lang == Lang::Es;
    let w = match (warnings, es) {
        (0, _) => String::new(),
        (1, false) => ", 1 warning".into(),
        (n, false) => format!(", {n} warnings"),
        (1, true) => ", 1 aviso".into(),
        (n, true) => format!(", {n} avisos"),
    };
    match (errors, es) {
        (0, false) => format!("Course clear!{}", w),
        (0, true) => format!("¡Nivel superado!{}", w),
        (1, false) => format!("Game Over: 1 error{w}"),
        (n, false) => format!("Game Over: {n} errors{w}"),
        (1, true) => format!("Game Over: 1 error{w}"),
        (n, true) => format!("Game Over: {n} errores{w}"),
    }
}

/// Levenshtein distance, for "did you mean ...?" suggestions.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The closest candidate within a small edit distance, if any.
pub fn suggest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let limit = (name.chars().count() / 3).clamp(1, 3);
    candidates
        .into_iter()
        .filter(|c| *c != name)
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions() {
        assert_eq!(suggest("lifes", ["lives", "score"]), Some("lives".into()));
        assert_eq!(suggest("x", ["score"]), None);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn render_points_at_the_column() {
        let src = "world 1-1 {\n  wahoo(lifes)\n}";
        let d = Diagnostic::error(
            Kind::UnknownName { name: "lifes".into(), suggestion: Some("lives".into()) },
            Span::new(20, 25),
        );
        let out = d.render(src, "t.wahoo", Lang::En);
        assert!(out.contains("t.wahoo:2:9"), "{out}");
        assert!(out.contains("2 |   wahoo(lifes)\n  |         ^^^^^"), "{out}");
        assert!(out.contains("did you mean `lives`?"), "{out}");
    }
}
