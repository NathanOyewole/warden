# Warden

**A minimal ownership checker, built from scratch in Rust, to understand why Rust's real borrow checker is hard.**

Warden is a small language with a real compiler pipeline - lexer, parser, static ownership checker, interpreter - whose entire type system is numbers, structs, and functions, plus one rule layered on top of all of it: **every binding has a single owner, and using a value after it's been moved is rejected before the program runs.**

```
let a = 5;
let b = a;        // a moved into b
print(a);         // REJECTED: use of moved value `a`
```

No `&`, no `&mut`, no lifetimes - deliberately. Warden isolates *only* the ownership half of Rust's borrow checker, proving that half in isolation is simple and learnable, and showing precisely why the other half (borrowing, lifetimes, aliasing) is where the real difficulty lives.

This is a thesis project for a self-practice, but CS major: the point is not to build a tool, but to *demonstrate understanding* of compiler architecture and static analysis by rebuilding a piece of Rust's core idea from scratch.

## The problem it addresses

Rust's borrow checker is really solving two distinct problems bundled together:

1. **Ownership** - every value has exactly one owner; when that owner gives the value up (a move), the value is gone, and any later use is a compile error.
2. **Borrowing** - temporarily letting other code look at (or modify) a value *without* taking ownership, governed by lifetimes and aliasing rules (`&`, `&mut`).

Nearly everything people find painful about early Rust is (2), not (1). Ownership alone is a genuinely simple, learnable idea. The question this project answers: **if you build ownership checking in isolation - no borrowing, no lifetimes - do you actually understand it better, and does it clarify why the other half is hard?**

## The pipeline

```
 source text ("let a = 5; let b = a; print(a);")
     │
     ▼
 ┌─────────┐   characters → tokens
 │  LEXER  │   [Let, Ident("a"), Equals, Number(5.0), Semicolon, ...]
 └─────────┘
     │  Vec<Token>
     ▼
 ┌─────────┐   tokens → tree (AST)
 │ PARSER  │   Stmt::Let { name: "a", value: Expr::Number(5.0) }
 └─────────┘
     │  AST (Program = Vec<Stmt>)
     ▼
 ┌─────────┐   walks the tree, tracks Owned / Moved / PartiallyMoved
 │ CHECKER │   per variable, rejects unsafe programs — THE THESIS
 └─────────┘
     │  only proceeds if the checker approves
     ▼
 ┌─────────────┐   walks the same tree again, computes real values,
 │ INTERPRETER │   executes print statements
 └─────────────┘
```

Four stages, each blind to everything except the stage immediately before it - the lexer never thinks about grammar, the parser never thinks about ownership, the checker never thinks about runtime values. **That separation of concerns, more than any individual algorithm, is the foundational idea of compiler architecture** and the lesson the whole project is built around.

## The core model: three ownership states

The checker tracks every binding in one of three states:

```
        let b = a;
 Owned ─────────────► Moved
   │                     ▲
   │ let x = p.field;    │  any further use of `a`
   ▼                     │  as a whole value = ERROR
 PartiallyMoved({field}) ┘
   (other fields still fine to use individually)
```

- **Owned** - fully available.
- **Moved** - used up; any further read is an error.
- **PartiallyMoved(fields)** - some struct fields taken, others still individually usable, but the struct *as a whole* is unusable until nothing's missing.

## Rules the checker enforces

| Rule | Example | Status |
|---|---|---|
| Use after move | `let b = a; print(a);` | Rejected |
| Double move | `let b = a; let c = a;` | Rejected |
| Partial move (field) | `let px = p.x; print(p.y);` | Accepted |
| Whole use after partial move | `let px = p.x; print(p);` | Rejected |
| Move then reassign (shadowing) | `let b = a; let a = 10; print(a);` | Accepted |
| Move into function call | `consume(s); print(s);` | Rejected |
| Move inside a loop (outer var) | `while (a) { let b = a; }` | Rejected |
| Move of loop-local variable | `while (a) { let x = 7; let y = x; }` | Accepted |
| Move diverges across `if` branches | `if (a) { let b = a; } print(a);` | Rejected |

Two of the checker's decisions are **deliberately conservative**, stricter than real Rust on purpose (they're part of the lesson, not bugs):

- **Branching:** the checker isn't path-sensitive. If a variable is moved on one branch of an `if`/`else` but not the other, it's treated as moved afterward - full stop. (Real Rust tracks exactly which path ran.)
- **Loops:** a variable that existed before a loop can't be moved from inside the loop body, because the checker won't attempt to prove how many times the loop runs (undecidable in general). Loop-local bindings are exempt.

## What Warden deliberately does *not* have

| Feature | Why Warden skips it | Why it's genuinely hard |
|---|---|---|
| Borrowing (`&`, `&mut`) | Everything moves instead | Requires tracking a *set* of active borrows, not a 3-state enum |
| Lifetimes | No borrows to bound | Region inference - proving no reference outlives its data |
| Non-lexical lifetimes | N/A | Real control-flow analysis, not just scope-based rules |
| Aliasing rules | No two names can ever refer to the same value | The actual rule the whole borrow checker exists to enforce |
| Mutation of existing bindings | Only fresh `let` bindings exist | Reopens aliasing questions immediately |

The honest throughline: Warden gets to skip all of the hard stuff specifically because it **never lets two names refer to the same value at the same time**. The instant real Rust allows borrowing, it inherits every row in that table at once. Move-checking alone is real ownership checking — and it's the easy 20%.

## Language syntax

```text
// numbers, print
let a = 5;
print(a);                    // 5

// structs + partial moves
struct Point { x, y }
let p = Point { x: 1, y: 2 };
let px = p.x;                // p becomes PartiallyMoved({x})
print(p.y);                  // OK — y was never touched
print(p);                    // REJECTED — p as a whole is partially moved

// functions (side effects only, no return values)
fn consume(v) {
    print(v);
}
consume(px);

// control flow (no reassignment, so loop conditions never change)
if (a) { print(a); } else { print(0); }
while (a) { let x = 1; }
```

`//` line comments are supported.

## Project structure

```
src/
  lib.rs          Public API: run_source() and check_source() wire the pipeline together
  lexer.rs        Characters → tokens (hand-rolled peek/advance scanner)
  ast.rs          The AST: Expr, Stmt, Program
  parser.rs       Recursive descent parser, tokens → tree
  checker.rs      The ownership checker — the thesis of the project
  interpreter.rs  Tree-walking interpreter for checked programs
  main.rs         A small demo program run through the full pipeline

examples/
  accept/         6 Warden programs the checker must accept (one rule each)
  reject/         6 Warden programs the checker must reject (one rule each)

tests/
  examples.rs     Integration tests: verifies every example against the real checker

docs/             Deep-dive articles explaining every stage, section by section
  ARTICLE_FINAL.md   The full written thesis, ready for publication
  ROADMAP.md      Phase-by-phase build history
```

## Getting started

Warden is a Rust project. You need a Rust toolchain (https://rustup.rs).

```bash
# run the full test suite (26 unit tests + 3 integration tests over the example suite)
cargo test

# run the demo program in main.rs (lex → parse → check → interpret)
cargo run

# check a Warden program by path (rejected programs exit with a descriptive error)
cargo run --release  # (main.rs uses a hardcoded demo; the library API is the real entry point)
```

Using the library API:

```rust
use warden::{check_source, run_source};

// full pipeline: lex → parse → check → interpret
run_source("let a = 5; let b = a; print(b);")?;

// checker only — proves a program is rejected without executing it
assert!(check_source("let a = 5; let b = a; print(a);").is_err());
```

## Tests

29 tests, all passing. They're layered deliberately:

- **3 lexer tests** - token streams, keyword recognition, unknown-char rejection.
- **8 parser tests** - AST shapes for every statement form, lookahead disambiguation, missing-semicolon errors.
- **11 checker tests** - each isolating exactly one ownership rule.
- **4 interpreter tests** - evaluation of checked programs.
- **3 integration tests** (`tests/examples.rs`) - the credibility layer: all 12 real programs in `examples/` are verified to behave the way the docs claim.

The integration tests use `check_source` rather than `run_source` because Warden has no reassignment, so some `while` loops in the accept examples are infinite by design - proving the checker accepts them doesn't require (and must not) run them.

## Documentation

The `docs/` folder is the written half of the thesis - every `src/*.rs` file explained section by section, including the Rust syntax used (not just the compiler-design ideas):

1. [`docs/00-overview.md`](docs/00-overview.md) — the project at a glance
2. [`docs/01-lexer.md`](docs/01-lexer.md) — characters → tokens
3. [`docs/02-parser.md`](docs/02-parser.md) — tokens → AST
4. [`docs/03-checker.md`](docs/03-checker.md) — the ownership analysis (the thesis)
5. [`docs/04-interpreter.md`](docs/04-interpreter.md) — AST → running program
6. [`docs/05-what-we-didnt-build.md`](docs/05-what-we-didnt-build.md) — what real Rust adds, and why each piece is hard
7. [`docs/PROJECT_VIEW.md`](docs/PROJECT_VIEW.md) — full project write-up
8. [`docs/ARTICLE_FINAL.md`](docs/ARTICLE_FINAL.md) — the complete article, ready to publish

## Roadmap

The build history lives in [`ROADMAP.md`](ROADMAP.md). Phases 1–6 (lexer, parser, structs/partial moves, the checker, interpreter, test suite + example programs) are complete; Phase 7 documents the intentionally-unbuilt features; Phase 8 covers diagrams, final edit, and packaging the thesis for publication.

## Why this exists

Not as a tool - as proof of understanding. The pitch, in one line:

> *"I built the simplest possible version of Rust's ownership model to understand why the full version is hard."*

Warden demonstrates real compiler architecture (four independent, composable stages), a genuine static-analysis problem solved from scratch (not copied from a tutorial), and Rust fluency deep enough to explain the language's own core idea by rebuilding a piece of it.
