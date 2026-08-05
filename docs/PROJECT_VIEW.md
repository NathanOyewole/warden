# Warden — Project View

*A minimal ownership checker, built to understand why Rust's real one is hard.*

## 1. The Problem

Rust has a reputation for being brutally hard to learn, and almost all
of that reputation traces back to one subsystem: the borrow checker.
But "the borrow checker" is actually solving *two* distinct problems
bundled together:

1. **Ownership** — every value has exactly one owner; when that owner
   goes out of scope (or the value is moved elsewhere), the value is
   gone. Using it after that is a compile error.
2. **Borrowing** — temporarily letting other code look at (or modify)
   a value *without* taking ownership of it, governed by lifetimes and
   aliasing rules (`&`, `&mut`).

Nearly everything people find painful about early Rust is actually
(2), not (1). Ownership alone is a genuinely simple, learnable idea.
The question this project set out to answer: **if you build ownership
checking in isolation — no borrowing, no lifetimes — do you actually
understand it better, and does it clarify why the *other* half is
hard?**

## 2. The Solution: Warden

Warden is a small language with a real compiler pipeline (lexer →
parser → static checker → interpreter), written in Rust, whose entire
type system is: numbers, structs, functions — and one rule layered on
top of all of it: **every binding has a single owner, and using a
value after it's been moved is rejected before the program runs.**

```
let a = 5;
let b = a;        // a moved into b
print(a);         // REJECTED: use of moved value `a`
```

No `&`, no `&mut`, no lifetimes — deliberately. Warden proves out
*only* the ownership half, in isolation, so that half becomes fully
legible.

## 3. Architecture

```
 source text
     │
     ▼
 ┌─────────┐  characters → tokens
 │  LEXER  │  "let a = 5;" → [Let, Ident("a"), Equals, Number(5.0), Semicolon]
 └─────────┘
     │
     ▼
 ┌─────────┐  tokens → tree (AST)
 │ PARSER  │  → Stmt::Let { name: "a", value: Expr::Number(5.0) }
 └─────────┘
     │
     ▼
 ┌─────────┐  walks the tree, tracks ownership state per variable,
 │ CHECKER │  rejects unsafe programs — THE THESIS OF THE PROJECT
 └─────────┘
     │  only proceeds if Ok(())
     ▼
 ┌─────────────┐  walks the same tree again, computes real values,
 │ INTERPRETER │  executes print statements
 └─────────────┘
```

Four stages, each blind to everything except the stage immediately
before it. That separation — not any single algorithm — is the
foundational idea of compiler architecture.

## 4. The Core Model: Three Ownership States

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
- **PartiallyMoved(fields)** — some struct fields taken, others
  still individually usable, but the struct *as a whole* is unusable
  until nothing's missing.

## 5. Worked Example: Partial Moves

```
struct Point { x, y }

let p = Point { x: 1, y: 2 };
let px = p.x;     // p becomes PartiallyMoved({x})
print(p.y);        // OK — y was never touched
print(p);          // REJECTED — p as a whole is partially moved
```

```
        Point { x: 1, y: 2 }
              p (Owned)
                 │
        let px = p.x;
                 │
                 ▼
        Point { x: ✗moved, y: 2 }
       p (PartiallyMoved({x}))
        │                    │
   print(p.y) → OK      print(p) → ERROR
   (y untouched)         (whole value, x missing)
```

## 6. Worked Example: The Loop Rule

```
let a = 1;
while (a) {
    let b = a;    // REJECTED — a existed before the loop;
}                 //            loop could run more than once
```

The checker doesn't try to prove how many times a loop actually runs
(undecidable in general). It conservatively blocks moving anything
that existed *before* the loop, from inside the loop body — full
stop. Variables `let`-bound fresh inside the loop are exempt, since
they're conceptually new on every iteration.

## 7. Worked Example: Branching (and a deliberate simplification)

```
let a = 5;
if (a) {
    let b = a;     // a moved on THIS path only
} else {
    print(0);
}
print(a);           // REJECTED — even though only one path moved `a`
```

```
             a: Owned
                │
        ┌───────┴───────┐
     if-branch        else-branch
     a → Moved         a stays Owned
        └───────┬───────┘
                 ▼
          merge: DISAGREE
                 │
                 ▼
         a → Moved (conservative)
```

Real Rust is *path-sensitive* here — it can prove the disagreement
away by tracking exactly which path was taken. Warden's checker merges
conservatively instead: any disagreement between branches becomes
`Moved`, full stop. This is **stricter than necessary**, on purpose —
it's the exact shortcut you're tempted to take when you first build
this, and hitting its limitation directly is part of the lesson.

## 8. What Got Left Out, and Why It's Hard

| Feature | Why Warden skips it | Why it's genuinely hard |
|---|---|---|
| Borrowing (`&`, `&mut`) | Everything moves instead | Requires tracking *active borrows* as a set, not a 3-state enum |
| Lifetimes | No borrows to bound | Region inference: proving no reference outlives its data |
| Non-lexical lifetimes | N/A | Real control-flow analysis, not just scope-based rules |
| Aliasing rules | No two names can ever refer to the same value | The actual rule the whole borrow checker exists to enforce |
| Mutation of existing bindings | Only fresh `let` bindings exist | Reopens aliasing questions immediately |

The honest throughline: Warden gets to skip *all* of the hard stuff
specifically because it never lets two names refer to the same value
at the same time. The instant real Rust allows borrowing, it inherits
every row in that table at once.

## 9. Validation: The Example Suite

12 real Warden programs, each isolating exactly one rule, run through
an automated test that verifies the checker's actual behavior matches
the claim:

- **`examples/accept/`** (6 programs) — simple move, partial move,
  shadow-reassign, function-call move, both-branches-moved-then-shadow,
  loop-local move.
- **`examples/reject/`** (6 programs) — use-after-move, double-move,
  whole-use-after-partial-move, move-in-loop, branch-divergence,
  move-into-function-then-reuse.

29 total tests (11 checker unit tests + 4 interpreter tests + 8 parser
tests + 3 lexer tests + 3 integration tests over the example suite),
all passing.

## 10. Why This Exists

Not as a tool — as proof of understanding. The pitch, in one line:
*"I built the simplest possible version of Rust's ownership model to
understand why the full version is hard."* It demonstrates real
compiler architecture (four independent, composable stages), a
genuine static-analysis problem solved from scratch (not copied from a
tutorial), and Rust fluency deep enough to explain the language's own
core idea by rebuilding a piece of it.
