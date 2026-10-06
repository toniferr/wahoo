# Wahoo! How a compiler works

A small but complete compiler, written in Rust, for **Wahoo**, a statically typed language from the Mushroom
Kingdom, targeting **WebAssembly**; and an interactive site about how programming languages are processed, published
at **https://toniferr.github.io/wahoo/** in English and Spanish ([/es/](https://toniferr.github.io/wahoo/es/)). The
site's figures and playground run the real compiler in the browser, itself compiled to WebAssembly.

```wahoo
pipe fib(n: coins) -> coins {
  question n < 2 {
    flag n
  }
  flag fib(n - 1) + fib(n - 2)
}

world 1-1 {
  run i from 0 to 10 {
    wahoo("fib(", i, ") = ", fib(i))
  }
}
```

Worlds are where programs start (played in order: 1-1, 1-2, …), `coin` declares a variable, `pipe` is a function and
`flag` returns from it, `question`/`else` is an if, `bounce` a while loop, `run … from … to` a counting loop,
`power_up`/`damage` add to and subtract from a counter, and `wahoo(…)` prints. The types are `coins` (32-bit
integers), `switch` (`star`/`goomba`) and `text`. The whole language fits on
[one page](https://toniferr.github.io/wahoo/language/).

## The compiler

Every classic phase, small enough to read in an evening (`crates/wahoo`, no dependencies):

| Phase | File | What it does |
| --- | --- | --- |
| Lexer | `lexer.rs` | characters → tokens, maximal munch, keeps going after errors |
| Parser | `parser.rs` | recursive descent for statements, Pratt parsing for expressions, panic-mode recovery |
| Checker | `checker.rs` | scopes, types, inference of local types, definite-return analysis, desugaring into the HIR (`hir.rs`) |
| Optimizer | `optimizer.rs` | constant folding with wrap-around arithmetic, algebraic identities, branch and dead-code elimination |
| Code generator | `codegen.rs` | HIR → stack-machine code, structured control flow, text literals in linear memory |
| Wasm toolkit | `wasm.rs` | hand-written binary encoder (LEB128, sections, `name` section) and WAT printer |
| Diagnostics | `diagnostics.rs` | errors and warnings in English and Spanish, with carets, hints and "did you mean" suggestions |

Errors come with precise titles and a touch of character:

```text
error[E205]: unknown name `lifes`
 --> level.wahoo:3:9
  |
3 |   wahoo(lifes)
  |         ^^^^^ Bowser has never heard of it
  = hint: did you mean `lives`?
```

`crates/wahoo-cli` is the `wahoo` command (`run`, `build`, `wat`, `tokens`, `ast`, `check`); it runs modules with
the [wasmi](https://github.com/wasmi-labs/wasmi) interpreter behind the default `run` feature. `crates/wahoo-web`
exposes the compiler to the browser through a tiny C ABI (no wasm-bindgen).

```sh
cargo test --workspace                       # unit tests + every example end to end, with and without the optimizer
cargo run -p wahoo-cli -- run examples/fib.wahoo
cargo run -p wahoo-cli -- wat examples/fib.wahoo
cargo run -p wahoo-cli -- check examples/es/errors.wahoo --lang es
node scripts/run.mjs program.wasm             # run a module with Node instead (V8)
```

`examples/` holds the sample programs with their expected output (`.out`), and `examples/es/` their Spanish versions.

## The site

Same principles as its sister sites ([Math of AI](https://toniferr.github.io/math-of-ai/),
[Math of Quantum](https://toniferr.github.io/math-of-quantum/)): hand-written HTML, CSS and JavaScript, a generator
that uses only the Python standard library, a strict Content Security Policy (own resources only; `'wasm-unsafe-eval'`
so the page may compile WebAssembly, and `worker-src 'self'` for the worker that runs programs), no CDN, no trackers.

```sh
cargo build -p wahoo-web --release --target wasm32-unknown-unknown   # the compiler, for the browser
python site/build.py --serve                                          # builds dist/ and serves it on :8000
python site/build.py --release                                        # strict: any warning fails the build
```

```text
site/
├── build.py                   generator → dist/ (English) and dist/es/ (Spanish)
├── content/
│   ├── site.json              base URL, languages, chapter order, playground examples
│   ├── i18n/{en,es}.json      interface and figure strings (same keys in both)
│   └── {en,es}/               home, chapters/<id>.html, playground, reference, timeline.json
└── src/
    ├── template.html, favicon.svg, css/main.css
    └── js/
        ├── core.js            demo registry and DOM helpers
        ├── wahoo.js           loads the compiler and provides the shared widgets (editor, tokens, tree, bytes…)
        ├── run-worker.js      runs a compiled program off the main thread, stopped after 3 s
        └── demos/<group>.js   interactive figures for each chapter and the playground
```

Seven chapters: execution models (compiled, interpreted, bytecode, JIT, transpiled, with a dozen real languages
compared), lexing, parsing, semantic analysis, code generation, optimisation and the runtime (WebAssembly), plus the
playground, the language reference and a history of compilers from 1843 to today.

In chapters, `<pre class="wahoo">` and `<pre class="wat">` blocks are highlighted at build time, and links are written
as `href="@ch:<id>#anchor"`, `@playground`, `@reference`, `@timeline` or `@home`; the build checks them all.

The `.github/workflows/deploy.yml` workflow checks formatting, runs clippy and the tests, builds the compiler for
wasm32 and the site, and deploys to GitHub Pages from `main`.

Wahoo is a fan-made teaching language; Mario and the Mushroom Kingdom belong to Nintendo, and no game assets are used.
