//! End to end: compile every example, let wasmi validate and run the module, and compare what it prints with
//! the expected `.out` file, with and without the optimizer (both must behave the same). The Spanish examples in
//! `examples/es/` go through the same checks.

#![cfg(feature = "run")]

use std::path::PathBuf;

use wahoo_cli::run_source;

const FUEL: Option<u64> = Some(50_000_000);

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

#[test]
fn examples_print_what_they_should() {
    let mut checked = 0;
    let dirs = [examples_dir(), examples_dir().join("es")];
    for entry in dirs.iter().flat_map(|d| std::fs::read_dir(d).unwrap()) {
        let path = entry.unwrap().path();
        let expected = path.with_extension("out");
        if path.extension().is_none_or(|e| e != "wahoo") || !expected.exists() {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let want = std::fs::read_to_string(&expected).unwrap().replace("\r\n", "\n");
        for optimize in [true, false] {
            let got = run_source(&src, optimize, FUEL).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(got, want, "{} (optimize: {optimize})", path.display());
        }
        checked += 1;
    }
    assert!(checked >= 12, "only {checked} examples found");
}

#[test]
fn the_error_example_reports_each_trap() {
    for path in [examples_dir().join("errors.wahoo"), examples_dir().join("es/errors.wahoo")] {
        let src = std::fs::read_to_string(&path).unwrap();
        let c = wahoo::compile(&src, &wahoo::Options::default());
        let codes: Vec<&str> = c.diagnostics.iter().map(|d| d.code()).collect();
        assert_eq!(codes, ["E207", "E205", "E209", "E210"], "{}: {:?}", path.display(), c.diagnostics);
    }
}

fn run(src: &str) -> Result<String, String> {
    run_source(src, true, FUEL)
}

#[test]
fn runtime_traps_are_reported_in_character() {
    let div =
        run("pipe zero() -> coins { flag 0 }\nworld 1-1 {\n wahoo(\"before\")\n wahoo(10 / zero())\n}").unwrap_err();
    assert!(div.starts_with("before\n") && div.contains("fell into a pit"), "{div}");

    let forever = run_source("world 1-1 {\n bounce star {\n }\n}", true, Some(100_000)).unwrap_err();
    assert!(forever.contains("Time's up"), "{forever}");

    let deep = run("pipe down(n: coins) -> coins { flag down(n + 1) }\nworld 1-1 { wahoo(down(0)) }").unwrap_err();
    assert!(deep.contains("stack overflow"), "{deep}");
}

#[test]
fn short_circuit_skips_the_right_side() {
    let src = "pipe loud() -> switch {\n wahoo(\"called\")\n flag star\n}\n\
               world 1-1 {\n wahoo(goomba and loud())\n wahoo(star or loud())\n wahoo(star and loud())\n}";
    for optimize in [true, false] {
        assert_eq!(run_source(src, optimize, FUEL).unwrap(), "goomba\nstar\ncalled\nstar\n");
    }
}

#[test]
fn arithmetic_matches_32_bit_machines() {
    let src = "world 1-1 {\n coin big = 2147483647\n power_up big\n wahoo(big)\n wahoo(-7 / 2, \" \", -7 % 2)\n}";
    for optimize in [true, false] {
        assert_eq!(run_source(src, optimize, FUEL).unwrap(), "-2147483648\n-3 -1\n");
    }
}

#[test]
fn texts_are_utf8_and_shared() {
    let src =
        "world 1-1 {\n coin a = \"¡Wahoo! ñ\"\n coin b = \"¡Wahoo! ñ\"\n wahoo(a, \" \", a == b, \" \", a != \"x\")\n}";
    assert_eq!(run(src).unwrap(), "¡Wahoo! ñ star star\n");
}

#[test]
fn scopes_shadow_and_loops_count_inclusively() {
    let src = "world 1-1 {\n coin x = 1\n run i from 3 to 5 {\n  coin x = i * 10\n  wahoo(x)\n }\n wahoo(x)\n run j from 2 to 1 { wahoo(\"never\") }\n}";
    for optimize in [true, false] {
        assert_eq!(run_source(src, optimize, FUEL).unwrap(), "30\n40\n50\n1\n");
    }
}

#[test]
fn early_flag_leaves_a_world() {
    let src = "world 1-1 {\n run i from 1 to 10 {\n  question i == 3 { flag }\n  wahoo(i)\n }\n}\nworld 1-2 { wahoo(\"next\") }";
    assert_eq!(run(src).unwrap(), "1\n2\nnext\n");
}
