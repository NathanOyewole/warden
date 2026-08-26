# Warden

**A minimal ownership checker, built from scratch in Rust, to understand why Rust's real borrow checker is hard.**

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-Shipped-brightgreen)](https://github.com/NathanOyewole/warden)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Warden is a small language with a real compiler pipeline — lexer, parser, static ownership checker, interpreter — whose type system is numbers, structs, and functions, plus one rule: **every binding has a single owner, and using a value after it's been moved is rejected before the program runs.**

```
let a = 5;
let b = a;        // a moved into b
print(a);         // REJECTED: use of moved value `a`
```

No `&`, no `&mut`, no lifetimes — deliberately. Warden isolates *only* the ownership half of Rust's borrow checker, proving that half in isolation is simple and learnable, and showing precisely why the other half (borrowing, lifetimes, aliasing) is where the real difficulty lives.

This is a CS-major thesis project, built as self-practice: the point is not to build a tool, but to *demonstrate understanding* of compiler architecture and static analysis by rebuilding a piece of Rust's core idea from scratch.

## The problem it addresses

Rust's borrow checker is really solving two distinct problems bundled together:

1. **Ownership** — every value has exactly one owner; when that owner gives the value up (a move), the value is gone, and any later use is a compile error.
2. **Borrowing** — temporarily letting other code look at (or modify) a value *without* taking ownership, governed by lifetimes and aliasing rules (`&`, `&mut`).

Nearly everything people find painful about early Rust is (2), not (1). Ownership alone is a genuinely simple, learnable idea. The question this project answers: **if you build ownership checking in isolation — no borrowing, no lifetimes — do you actually understand it better, and does it clarify why the other half is hard?**

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
 ┌────────────┐   walks the same tree again, computes real values,
 │ INTERPRETER │   executes print statements
 └────────────┘
```

Four stages, each blind to everything except the stage immediately before it. **That separation of concerns, more than any individual algorithm, is the foundational idea of compiler architecture** and the lesson the whole project is built around.

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

- **Owned** — fully available.
- **Moved** — used up; any further read is an error.
- **PartiallyMoved(fields)** — some struct fields taken, others still individually usable, but the struct *as a whole* is unusable until nothing's missing.

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

Two decisions are **deliberately conservative**, stricter than real Rust on purpose (part of the lesson):

- **Branching:** not path-sensitive. If a variable is moved on one branch of an `if`/`else` but not the other, it's treated as moved afterward.
- **Loops:** a variable that existed before a loop can't be moved from inside the loop body. Loop-local bindings are exempt.

## What Warden deliberately does *not* have

| Feature | Why Warden skips it | Why it's genuinely hard |
|---|---|---|
| Borrowing (`&`, `&mut`) | Everything moves instead | Tracking a *set* of active borrows |
| Lifetimes | No borrows to bound | Region inference |
| Non-lexical lifetimes | N/A | Real control-flow analysis |
| Aliasing rules | No two names can refer to the same value | The rule the borrow checker exists to enforce |
| Mutation of existing bindings | Only fresh `let` bindings | Reopens aliasing questions |

The throughline: Warden skips the hard stuff because it **never lets two names refer to the same value at the same time**. The instant real Rust allows borrowing, it inherits every row in that table. Move-checking alone is real ownership checking — and it's the easy 20%.

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

// control flow
if (a) { print(a); } else { print(0); }
while (a) { let x = 1; }
```

`//` line comments are supported.

## Project structure

```
src/
  lib.rs          Public API: run_source() and check_source()
  lexer.rs        Characters → tokens
  ast.rs          AST: Expr, Stmt, Program
  parser.rs       Recursive descent parser
  checker.rs      The ownership checker — the thesis
  interpreter.rs  Tree-walking interpreter
  main.rs         Demo program through the full pipeline

examples/
  accept/         Programs the checker must accept
  reject/         Programs the checker must reject

tests/
  examples.rs     Integration tests over the example suite

docs/             Deep-dive articles + full thesis write-up
```

## Getting started

You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```bash
cargo test          # full suite
cargo run           # demo through lex → parse → check → interpret
```

Library API:

```rust
use warden::{check_source, run_source};

run_source("let a = 5; let b = a; print(b);")?;
assert!(check_source("let a = 5; let b = a; print(a);").is_err());
```

## Tests

29 tests, all passing — lexer, parser, checker (one rule each), interpreter, and integration tests over `examples/`.

## Documentation

The `docs/` folder is the written half of the thesis:

1. [`docs/00-overview.md`](docs/00-overview.md)
2. [`docs/01-lexer.md`](docs/01-lexer.md)
3. [`docs/02-parser.md`](docs/02-parser.md)
4. [`docs/03-checker.md`](docs/03-checker.md) — the ownership analysis
5. [`docs/04-interpreter.md`](docs/04-interpreter.md)
6. [`docs/05-what-we-didnt-build.md`](docs/05-what-we-didnt-build.md)
7. [`docs/ARTICLE_FINAL.md`](docs/ARTICLE_FINAL.md) — complete article

## Why this exists

Not as a tool — as proof of understanding:

> *"I built the simplest possible version of Rust's ownership model to understand why the full version is hard."*

## Status

**Shipped** — pipeline, checker, tests, and docs complete.

## License

MIT
