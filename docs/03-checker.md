# The Ownership Checker - `src/checker.rs`

**This is the actual thesis of the project.** Everything before this
(lexer, parser) exists to get the program into a shape this file can
analyze. Everything after (the interpreter) only runs if this file
says the program is safe.

## The core model

Every variable is in exactly one of three states:

```
   Owned ──────let b = a;─────► Moved
     │                             ▲
     │  let p.x = ...              │  using `a` again
     ▼                             │  after this = ERROR
PartiallyMoved({x}) ───let _ = p.y;┘
     (p.y still fine to use)
```

- **Owned** - fully available.
- **Moved** - used up. Any further read is an error.
- **PartiallyMoved(set of field names)** - some struct fields moved
  out, others still usable individually, but the struct *as a whole*
  can't be used until nothing's missing.

## Diagram: checking `let a = 5; let b = a; print(a);`

```
scope = {}

stmt 1: let a = 5;
  check_use(5)         → OK, it's a literal
  apply_move(5)        → no-op, literals aren't bindings
  scope["a"] = Owned

stmt 2: let b = a;
  check_use(a)         → scope["a"] is Owned → OK
  apply_move(a)        → scope["a"] = Moved
  scope["b"] = Owned

stmt 3: print(a);
  check_use(a)         → scope["a"] is Moved → ERROR
                          "use of moved value `a`"
```

## Rust concepts used here

- **`HashMap<String, VarState>`** - the "scope": a lookup table from
  variable name to its current ownership state. `scope.clone()` shows
  up wherever we need to explore a branch (an `if`'s two arms)
  without letting one branch's changes leak into the other.
- **`match entry { VarState::Owned => ..., VarState::Moved => ... }`**
  - the same exhaustive-match pattern from the parser, now deciding
  what's *legal*, not just what to build.
- **`.entry(name).or_insert(...)`** - a `HashMap` idiom: "give me the
  entry for this key, and if it doesn't exist yet, insert a default
  first." Used when partially moving a field for the first time.
- **Recursion mirroring the AST shape** - `check_use` and `apply_move`
  each have one match arm per `Expr` variant, and the `Field` and
  `Call` arms call themselves recursively on their inner expressions.
  This "the function's shape mirrors the data's shape" pattern is why
  functional-style tree-walking is so common in compilers.

## Section by section

**`check_use(expr, scope)`**
Answers: *"can I legally read this right now?"* Never changes
anything - it's a pure check. For an `Ident`, look up its state: fine
if `Owned`, error if `Moved` or `PartiallyMoved`. For `p.x`, look at
`p`'s state specifically for field `x` - if `x` is in the moved-fields
set, error; otherwise fine, *even if other fields of `p` are moved*.

**`apply_move(expr, scope, loop_depth, loop_locals)`**
Runs right after a successful `check_use`, and is where a value
actually gets marked used-up. For an `Ident`, flip its state to
`Moved`. For `p.x`, add `"x"` to `p`'s partial-move set (creating it
if this is the first field moved from `p`). This function is also
where the loop-move rule lives — see below.

**`Stmt::Let` - why shadowing "resets" a variable**
```rust
scope.insert(name.clone(), VarState::Owned);
```
A `let` *always* inserts a fresh `Owned` entry, even if that name
already existed and was `Moved`. That's the entire mechanism behind
"move, then reassign is fine": `let a = 5; let b = a; let a = 10;` —
the second `let a` doesn't care what happened to the first `a`, it's a
completely new binding that happens to share a name.

**`Stmt::If` - branch merging, and a deliberate simplification**
The checker clones the scope, checks the `then` branch against one
copy and the `else` branch against another, then merges the results.
The merge rule: a variable is `Owned` after the `if` only if it was
`Owned` on *both* paths. Any disagreement between branches - including
"moved on one path, untouched on the other" - is conservatively
treated as `Moved`.

This is **stricter than real Rust**, on purpose. Real Rust does
path-sensitive analysis: it can prove "if you took the branch that
moved `a`, using `a` afterward is only an error on that path, and it's
fine on the other." That's a genuinely harder piece of dataflow
analysis. Warden's simpler, more conservative merge is exactly the
kind of shortcut you're tempted to take when you first build this -
and seeing where it forces a real limitation (`accept
/05_if_both_branches_then_reassign.wd` needs a shadow-reassign to
un-stick a variable that real Rust wouldn't need) *is* the lesson.

**`Stmt::While` - the loop-move rule**
```rust
if loop_depth > 0 && !loop_locals.contains(name) {
    return Err(...)
}
```
Any variable that existed *before* the loop started is forbidden from
being moved *inside* the loop body - full stop, regardless of whether
the loop would actually run more than once. The checker doesn't try to
prove how many times a loop executes (that's undecidable in general -
see the Halting Problem); it just conservatively assumes "could be more
than once" and blocks the move. Variables freshly `let`-bound *inside*
the loop body are exempt (`loop_locals`), since a fresh binding exists
independently on every iteration.

## The four other rules, and where they fall out of the model above

- **Use-after-move** - directly `check_use` seeing `Moved`.
- **Double-move** - a second move attempt is just another
  `check_use`/`apply_move` pair on an already-`Moved` binding; it's
  the same code path as use-after-move.
- **Partial-move tracking** - the `Field` arms of `check_use` /
  `apply_move`, operating on the field-name set inside
  `PartiallyMoved`.
- **Move-then-reassign** - falls out for free from `let` always
  inserting a fresh `Owned` entry (see above); no special-case code
  needed.

## Why this matters for real Rust

Real Rust's borrow checker solves a strictly harder version of every
problem here: it's path-sensitive (branches), it understands *partial
initialization* the same way Warden does but generalizes it to nested
paths of arbitrary depth, and on top of all of that it also tracks
*borrows* (`&`/`&mut`) with lifetimes, which is the whole other axis
Warden skips entirely (see `docs/05-what-we-didnt-build.md`). Every
simplification called out above is a specific, nameable piece of that
larger problem.
