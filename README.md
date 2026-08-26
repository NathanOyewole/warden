# Warden

A minimal ownership checker written in Rust.

Warden is a small language with a full compiler pipeline (lexer → parser → ownership checker → interpreter). Values have a single owner; use after move is rejected before execution. No borrowing, no lifetimes — ownership only.

```
let a = 5;
let b = a;   // a moved
print(a);    // error: use of moved value `a`
```

## Pipeline

```
source → lexer → parser → checker → interpreter
```

| Stage | Role |
|-------|------|
| Lexer | Characters → tokens |
| Parser | Tokens → AST |
| Checker | Tracks Owned / Moved / PartiallyMoved; rejects unsafe programs |
| Interpreter | Evaluates checked programs |

## Ownership model

- **Owned** — available
- **Moved** — further use is an error
- **PartiallyMoved** — some struct fields moved; whole-value use rejected

Deliberately conservative on control flow: branch divergence and outer-loop moves are treated as moved (stricter than real Rust).

## Example

```text
struct Point { x, y }
let p = Point { x: 1, y: 2 };
let px = p.x;   // p partially moved
print(p.y);     // ok
print(p);       // error
```

## Usage

```bash
cargo test
cargo run
```

```rust
use warden::{check_source, run_source};

run_source("let a = 5; let b = a; print(b);")?;
assert!(check_source("let a = 5; let b = a; print(a);").is_err());
```

## Layout

```
src/           lexer, parser, checker, interpreter
examples/      accept/ and reject/ programs
tests/         integration tests
```

## License

MIT
