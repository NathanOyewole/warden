# The Interpreter - `src/interpreter.rs`

**Job:** given a program the checker has already approved, actually
compute its values and produce output. This is a *tree-walking*
interpreter - it evaluates the AST directly, with no intermediate
bytecode or compilation step.

## Diagram

```
Stmt::Let { name: "p", value: StructLit { name: "Point", fields: [("x", Number(1.0)), ("y", Number(2.0))] } }
     │
     ▼
eval(StructLit)
     │
     ├─ eval(Number(1.0)) → Value::Number(1.0)
     ├─ eval(Number(2.0)) → Value::Number(2.0)
     ▼
Value::Struct { name: "Point", fields: { "x": Number(1.0), "y": Number(2.0) } }
     │
     ▼
env["p"] = Value::Struct { ... }
```

## Rust concepts used here

- **`impl fmt::Display for Value`** - a *trait implementation*. A
  trait is Rust's version of an interface: "any type that implements
  `Display` knows how to turn itself into user-facing text." Writing
  this `impl` block is what makes `println!("{}", some_value)` work
  for our custom `Value` type - without it, the compiler wouldn't know
  how to print one.
- **`HashMap<String, Value>` as the environment** - the interpreter's
  "scope" is structurally identical to the checker's scope, just
  holding real values instead of ownership states. Same idea, later
  stage.
- **`.cloned()` on lookups** - `env.get(name).cloned()` copies the
  value out of the map rather than borrowing it. Simpler to reason
  about for a toy interpreter, at the cost of some unnecessary
  copying - a real production interpreter would be far more careful
  about this.
- **`ok_or_else(|| format!(...))`** - turns an `Option` into a
  `Result`: "if this was `None`, turn that into an `Err` with this
  message; if it was `Some(x)`, turn it into `Ok(x)`." Used everywhere
  a variable or function lookup might fail at runtime.

## Section by section

**`Value` enum**
Only two shapes of runtime value exist: `Number(f64)` and
`Struct { name: String, fields: HashMap<String, Value> }`. Notice
`fields` can itself contain `Value`s - including, in principle,
another `Struct` - which is fine here because `HashMap` (unlike a bare
recursive enum) already lives on the heap internally, so there's no
`Box` needed the way there was for `Expr::Field` in the AST.

**`run()` - pre-registering functions**
Before executing anything, the interpreter scans the whole program for
`Stmt::FnDef` and stores each one in a `functions` map. This means
call order in the source doesn't matter - you could call a function
before its definition appears textually, same as the checker's
struct-pre-registration in `check_program`.

**`eval(expr, env)` - one match arm per `Expr` variant**
This function's shape is the interpreter's version of the same pattern
you've now seen three times: `Ident` looks itself up in the
environment; `Field` evaluates its base first (which must turn out to
be a `Struct`) and then looks up the requested field; `Call` evaluates
each argument, binds them to the callee's parameter names in a
*brand-new, empty environment* (Warden has no closures - a function
body cannot see anything outside its own parameters), then runs the
function body against that environment.

**Why function calls return `Value::Number(0.0)`**
Warden has no `return` statement - functions exist purely for their
side effects (`print`). Calling one as if it were a value-producing
expression is technically allowed by the grammar (`let x = consume(a);`
would parse), so `eval` needs to return *something* for that case; `0`
is a placeholder, not a meaningful result. This is a real, named gap -
see `docs/05-what-we-didnt-build.md`.

**`exec_stmt` for `If` / `While`**
Straightforward once you've read the checker's version: evaluate the
condition, check `truthy` (any nonzero number, or any struct at all,
counts as true - Warden has no boolean type), then execute the
appropriate branch or loop while true. No surprises here — the
*interesting* control-flow logic already happened in the checker; the
interpreter's job is just to actually do it.

## A gap this file exposed, that the checker never would have

Warden has `let` (always a fresh binding) but no plain assignment
(`x = expr;` to an *existing* binding). That means a `while` loop's
condition can never change over the loop's lifetime - it's either
always truthy (infinite loop) or always falsy (zero iterations).
`tests/examples.rs` works around this by never *running* the
`while`-loop accept-examples, only checking them - running one would
hang forever. This is exactly the kind of consequence that only shows
up once you try to actually execute a language, not just parse and
check it - see `docs/05-what-we-didnt-build.md` for the full writeup.

## Why this matters for real Rust

rustc doesn't tree-walk - by the time your code runs, it's been
compiled all the way to machine code via LLVM. But every real
interpreter for a dynamic or scripting language (Python's CPython, for
long stretches of its history; Ruby's MRI) does exactly this: walk the
tree, maintain an environment, evaluate node by node. Tree-walking is
also usually the *first* working version of any language
implementation, real or toy, before anyone bothers optimizing it into
bytecode.
