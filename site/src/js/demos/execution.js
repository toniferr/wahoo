// Execution — how a dozen real languages get from source text to instructions the CPU runs.
// Every step is marked as happening before the program runs ("build"), while it runs ("run") or in hardware.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  // [name, description, when] per step; the descriptions are short on purpose.
  var LANGS = {
    en: [
      { id: "c", name: "C", kind: "aot", steps: [
        ["Source", "hello.c", "build"], ["Preprocessor", "expands #include and #define", "build"],
        ["Compiler", "parse, check, optimise (GCC, Clang)", "build"], ["Assembler", "object file .o", "build"],
        ["Linker", "joins objects and libraries into an executable", "build"], ["Loader", "the OS maps it into memory", "run"],
        ["CPU", "runs machine code directly", "hw"]],
        note: "All the translation happens before the program runs. The executable is fast to start but only runs on one operating system and processor family." },
      { id: "rust", name: "Rust", kind: "aot", steps: [
        ["Source", "main.rs", "build"], ["rustc front end", "parse, expand macros, check types and borrows", "build"],
        ["MIR → LLVM IR", "Rust's own IR, then LLVM's", "build"], ["LLVM", "optimise, select instructions, allocate registers", "build"],
        ["Linker", "executable", "build"], ["CPU", "runs machine code directly", "hw"]],
        note: "Like C, but with a much richer front end: the borrow checker proves memory safety at compile time, so no garbage collector is needed at run time." },
      { id: "go", name: "Go", kind: "aot", steps: [
        ["Source", "main.go", "build"], ["gc compiler", "parse, type-check, SSA optimisations", "build"],
        ["Linker", "one static executable, runtime included", "build"], ["Go runtime", "garbage collector and goroutine scheduler", "run"],
        ["CPU", "runs machine code", "hw"]],
        note: "Compiled ahead of time, but every binary carries a runtime: a garbage collector and a scheduler for goroutines run alongside your code." },
      { id: "haskell", name: "Haskell", kind: "aot", steps: [
        ["Source", "Main.hs", "build"], ["GHC front end", "parse, infer types (Hindley–Milner and beyond)", "build"],
        ["Core → STG → Cmm", "a chain of intermediate languages", "build"], ["Code generator", "native code (or via LLVM)", "build"],
        ["Runtime system", "lazy evaluation, garbage collection", "run"], ["CPU", "runs machine code", "hw"]],
        note: "A compiled functional language: the types are inferred, and most of the compiler's work is rewriting one small core language into another." },
      { id: "java", name: "Java", kind: "jit", steps: [
        ["Source", "Main.java", "build"], ["javac", "compiles to JVM bytecode (.class, .jar)", "build"],
        ["JVM loader", "loads and verifies the bytecode", "run"], ["Interpreter", "runs bytecode, counts hot methods", "run"],
        ["JIT (HotSpot C1, C2)", "compiles hot methods to machine code", "run"], ["CPU", "runs the compiled methods", "hw"]],
        note: "Two compilers: javac before, and a just-in-time compiler inside the JVM that only spends effort on code that turns out to be hot." },
      { id: "csharp", name: "C#", kind: "jit", steps: [
        ["Source", "Program.cs", "build"], ["Roslyn", "compiles to CIL bytecode in a .dll", "build"],
        ["CLR", "loads the assembly", "run"], ["RyuJIT", "compiles each method the first time it is called, then recompiles hot ones", "run"],
        ["CPU", "runs machine code", "hw"]],
        note: "Very close to Java's design. .NET can also compile everything ahead of time (Native AOT) when fast start-up matters more than peak speed." },
      { id: "python", name: "Python", kind: "vm", steps: [
        ["Source", "app.py", "run"], ["Compiler", "parse to an AST, compile to bytecode (imported modules are cached in __pycache__)", "run"],
        ["Bytecode VM", "CPython's evaluation loop, written in C", "run"], ["CPU", "runs the interpreter, which runs your code", "hw"]],
        note: "Python is compiled too, to bytecode, but at run time and without heavy optimisation; the CPU then runs the interpreter loop. CPython 3.13 added an experimental JIT, and PyPy is a separate implementation with a tracing JIT." },
      { id: "ruby", name: "Ruby", kind: "vm", steps: [
        ["Source", "app.rb", "run"], ["Parser", "builds the syntax tree", "run"],
        ["YARV compiler", "turns it into bytecode", "run"], ["YARV VM", "runs the bytecode", "run"],
        ["YJIT (optional)", "compiles hot code to machine code", "run"], ["CPU", "runs it", "hw"]],
        note: "Until Ruby 1.8 the interpreter walked the syntax tree directly; since 1.9 it compiles to bytecode first. It has had a JIT since 2.6; the current one, YJIT, arrived in 3.1." },
      { id: "js", name: "JavaScript", kind: "jit", steps: [
        ["Source", "the browser downloads app.js", "run"], ["Parser", "AST, lazily: functions are parsed when first called", "run"],
        ["Ignition", "bytecode interpreter, collects type feedback", "run"], ["Sparkplug / Maglev", "quick compilers for warm code", "run"],
        ["TurboFan", "optimising compiler for hot code; deoptimises if a guess fails", "run"], ["CPU", "runs machine code", "hw"]],
        note: "Everything happens on the user's machine, under time pressure: V8 starts interpreting at once and only optimises what the page actually uses, guessing types from what it has seen." },
      { id: "ts", name: "TypeScript", kind: "trans", steps: [
        ["Source", "app.ts", "build"], ["tsc", "type-checks, then erases the types", "build"],
        ["JavaScript", "plain .js", "build"], ["JS engine", "then as JavaScript: interpreter + JIT", "run"],
        ["CPU", "runs machine code", "hw"]],
        note: "A transpiler (source-to-source compiler): the type checker catches errors before running, then the types vanish and the engine never sees them." },
      { id: "bash", name: "Bash", kind: "interp", steps: [
        ["Source", "script.sh", "run"], ["Read a command", "one line or compound command at a time", "run"],
        ["Parse and expand", "variables, globs, quotes", "run"], ["Execute", "built-in, or start another program", "run"],
        ["CPU", "runs bash itself (a compiled C program)", "hw"]],
        note: "A classic interpreter: it reads, parses and executes each command in turn, so a syntax error on line 50 only shows up when execution reaches line 50." },
      { id: "wahoo", name: "Wahoo", kind: "aot", steps: [
        ["Source", "fib.wahoo", "build"], ["wahoo compiler", "lexer, parser, checker, optimiser, code generator (Rust)", "build"],
        ["WebAssembly", "a .wasm module", "build"], ["Wasm engine", "validates, then compiles it (V8: Liftoff, then TurboFan)", "run"],
        ["CPU", "runs machine code", "hw"]],
        note: "This site's language. In the playground the compiler itself is a WebAssembly module: Rust compiled to Wasm, compiling Wahoo to Wasm." },
    ],
    es: [
      { id: "c", name: "C", kind: "aot", steps: [
        ["Fuente", "hola.c", "build"], ["Preprocesador", "expande #include y #define", "build"],
        ["Compilador", "analiza, comprueba, optimiza (GCC, Clang)", "build"], ["Ensamblador", "fichero objeto .o", "build"],
        ["Enlazador", "une objetos y bibliotecas en un ejecutable", "build"], ["Cargador", "el sistema operativo lo lleva a memoria", "run"],
        ["CPU", "ejecuta código máquina directamente", "hw"]],
        note: "Toda la traducción ocurre antes de ejecutar. El ejecutable arranca rápido, pero solo funciona en un sistema operativo y una familia de procesadores." },
      { id: "rust", name: "Rust", kind: "aot", steps: [
        ["Fuente", "main.rs", "build"], ["Front end de rustc", "analiza, expande macros, comprueba tipos y préstamos", "build"],
        ["MIR → LLVM IR", "la IR propia de Rust y luego la de LLVM", "build"], ["LLVM", "optimiza, elige instrucciones, asigna registros", "build"],
        ["Enlazador", "ejecutable", "build"], ["CPU", "ejecuta código máquina directamente", "hw"]],
        note: "Como C, pero con un front end mucho más rico: el borrow checker demuestra la seguridad de memoria al compilar, así que no hace falta recolector de basura." },
      { id: "go", name: "Go", kind: "aot", steps: [
        ["Fuente", "main.go", "build"], ["Compilador gc", "analiza, comprueba tipos, optimiza en SSA", "build"],
        ["Enlazador", "un único ejecutable estático, con su runtime", "build"], ["Runtime de Go", "recolector de basura y planificador de goroutines", "run"],
        ["CPU", "ejecuta código máquina", "hw"]],
        note: "Se compila de antemano, pero cada binario lleva un runtime: un recolector de basura y un planificador de goroutines trabajan junto a tu código." },
      { id: "haskell", name: "Haskell", kind: "aot", steps: [
        ["Fuente", "Main.hs", "build"], ["Front end de GHC", "analiza, infiere tipos (Hindley–Milner y más)", "build"],
        ["Core → STG → Cmm", "una cadena de lenguajes intermedios", "build"], ["Generador de código", "código nativo (o vía LLVM)", "build"],
        ["Sistema de ejecución", "evaluación perezosa, recolección de basura", "run"], ["CPU", "ejecuta código máquina", "hw"]],
        note: "Un lenguaje funcional compilado: los tipos se infieren y casi todo el trabajo del compilador es reescribir un pequeño lenguaje núcleo en otro." },
      { id: "java", name: "Java", kind: "jit", steps: [
        ["Fuente", "Main.java", "build"], ["javac", "compila a bytecode de la JVM (.class, .jar)", "build"],
        ["Cargador de la JVM", "carga y verifica el bytecode", "run"], ["Intérprete", "ejecuta bytecode y cuenta los métodos calientes", "run"],
        ["JIT (HotSpot C1, C2)", "compila los métodos calientes a código máquina", "run"], ["CPU", "ejecuta los métodos compilados", "hw"]],
        note: "Dos compiladores: javac antes, y uno «justo a tiempo» dentro de la JVM que solo se esfuerza con el código que resulta ser caliente." },
      { id: "csharp", name: "C#", kind: "jit", steps: [
        ["Fuente", "Program.cs", "build"], ["Roslyn", "compila a bytecode CIL en una .dll", "build"],
        ["CLR", "carga el ensamblado", "run"], ["RyuJIT", "compila cada método la primera vez que se llama, y recompila los calientes", "run"],
        ["CPU", "ejecuta código máquina", "hw"]],
        note: "Un diseño muy parecido al de Java. .NET también puede compilarlo todo de antemano (Native AOT) cuando importa más arrancar rápido que la velocidad punta." },
      { id: "python", name: "Python", kind: "vm", steps: [
        ["Fuente", "app.py", "run"], ["Compilador", "analiza a un AST y compila a bytecode (los módulos importados se guardan en __pycache__)", "run"],
        ["Máquina virtual", "el bucle de evaluación de CPython, escrito en C", "run"], ["CPU", "ejecuta el intérprete, que ejecuta tu código", "hw"]],
        note: "Python también se compila, a bytecode, pero al ejecutar y sin optimizar apenas; la CPU ejecuta el bucle del intérprete. CPython 3.13 añadió un JIT experimental, y PyPy es otra implementación con un JIT de trazas." },
      { id: "ruby", name: "Ruby", kind: "vm", steps: [
        ["Fuente", "app.rb", "run"], ["Parser", "construye el árbol sintáctico", "run"],
        ["Compilador YARV", "lo convierte en bytecode", "run"], ["Máquina virtual YARV", "ejecuta el bytecode", "run"],
        ["YJIT (opcional)", "compila el código caliente a código máquina", "run"], ["CPU", "lo ejecuta", "hw"]],
        note: "Hasta Ruby 1.8 el intérprete recorría el árbol sintáctico directamente; desde la 1.9 compila antes a bytecode. Tiene JIT desde la 2.6; el actual, YJIT, llegó con la 3.1." },
      { id: "js", name: "JavaScript", kind: "jit", steps: [
        ["Fuente", "el navegador descarga app.js", "run"], ["Parser", "AST, de forma perezosa: cada función se analiza al llamarla", "run"],
        ["Ignition", "intérprete de bytecode que recoge información de tipos", "run"], ["Sparkplug / Maglev", "compiladores rápidos para código templado", "run"],
        ["TurboFan", "compilador optimizador para el código caliente; desoptimiza si falla una suposición", "run"], ["CPU", "ejecuta código máquina", "hw"]],
        note: "Todo ocurre en la máquina del usuario y con prisa: V8 empieza a interpretar de inmediato y solo optimiza lo que la página usa de verdad, adivinando los tipos por lo que ha visto." },
      { id: "ts", name: "TypeScript", kind: "trans", steps: [
        ["Fuente", "app.ts", "build"], ["tsc", "comprueba los tipos y luego los borra", "build"],
        ["JavaScript", ".js normal", "build"], ["Motor de JS", "a partir de aquí, como JavaScript: intérprete + JIT", "run"],
        ["CPU", "ejecuta código máquina", "hw"]],
        note: "Un transpilador (compilador de fuente a fuente): el comprobador de tipos caza errores antes de ejecutar; luego los tipos desaparecen y el motor nunca los ve." },
      { id: "bash", name: "Bash", kind: "interp", steps: [
        ["Fuente", "script.sh", "run"], ["Leer una orden", "una línea u orden compuesta cada vez", "run"],
        ["Analizar y expandir", "variables, comodines, comillas", "run"], ["Ejecutar", "orden interna, o lanzar otro programa", "run"],
        ["CPU", "ejecuta el propio bash (un programa en C compilado)", "hw"]],
        note: "Un intérprete clásico: lee, analiza y ejecuta cada orden por turnos, así que un error de sintaxis en la línea 50 solo aparece cuando la ejecución llega a la línea 50." },
      { id: "wahoo", name: "Wahoo", kind: "aot", steps: [
        ["Fuente", "fib.wahoo", "build"], ["Compilador wahoo", "lexer, parser, comprobador, optimizador, generador (Rust)", "build"],
        ["WebAssembly", "un módulo .wasm", "build"], ["Motor Wasm", "lo valida y lo compila (V8: Liftoff y luego TurboFan)", "run"],
        ["CPU", "ejecuta código máquina", "hw"]],
        note: "El lenguaje de esta web. En el playground el propio compilador es un módulo WebAssembly: Rust compilado a Wasm, compilando Wahoo a Wasm." },
    ],
  };

  WH.register("execution/pipeline", function (stage) {
    var langs = LANGS[WH.lang()] || LANGS.en;
    var picker = h("div", { class: "lang-picker", role: "group", "aria-label": t("exec.pick") });
    var flow = h("ol", { class: "pipe-flow", "aria-live": "polite" });
    var note = h("p", { class: "pipe-note" });
    var buttons = langs.map(function (l) {
      var b = h("button", { type: "button", class: "chip", "aria-pressed": "false", onclick: function () { show(l); } }, l.name);
      picker.appendChild(b);
      return b;
    });

    function show(l) {
      buttons.forEach(function (b, i) { b.setAttribute("aria-pressed", langs[i] === l ? "true" : "false"); });
      WH.clear(flow);
      l.steps.forEach(function (st) {
        var cls = st[2] === "hw" ? "hw" : st[2] === "build" ? "at-build" : "at-run";
        flow.appendChild(h("li", { class: "pipe-step " + cls },
          h("strong", {}, st[0]), st[1], h("span", { class: "when" }, t("exec.when." + st[2]))));
      });
      note.textContent = t("exec.kind." + l.kind) + " " + l.note;
    }

    stage.appendChild(picker);
    stage.appendChild(flow);
    stage.appendChild(note);
    show(langs[0]);
  });
})();
