//! The ownership checker: a static analysis pass over the AST that
//! decides whether a program is allowed to run, before we ever
//! interpret it. This file is deliberately the most heavily commented
//! in the project — it's the actual thesis of the whole piece.
//!
//! Core model: every binding is in one of three states.
//!   Owned              — fully available
//!   Moved               — used up; any further use is an error
//!   PartiallyMoved(set) — some struct fields moved, others still usable
//!
//! Known, deliberate simplifications vs. real Rust (documented so the
//! article can point at them directly):
//!   1. Everything moves — there's no Copy-type exception. `let a = 5;
//!      let b = a;` really does invalidate `a`, even though real Rust
//!      would let you do this for an i32. This keeps the core lesson
//!      undiluted by a second concept.
//!   2. If/else merging is NOT path-sensitive. If a variable is moved
//!      in only one branch, we conservatively treat it as moved after
//!      the `if` entirely, rather than tracking "moved on this path,
//!      fine on that path." Real Rust does the harder, precise version.
//!   3. Loop-move checking is a single static pass, not a fixed-point
//!      iteration. We forbid moving any variable that existed *before*
//!      the loop from inside the loop body, full stop — we don't
//!      attempt to prove some loops only run once.

use std::collections::{HashMap, HashSet};

use crate::ast::{Expr, Program, Stmt};

#[derive(Debug, Clone, PartialEq)]
enum VarState {
    Owned,
    Moved,
    PartiallyMoved(HashSet<String>),
}

type Scope = HashMap<String, VarState>;

pub struct Checker {
    structs: HashMap<String, Vec<String>>,
}

impl Checker {
    pub fn new() -> Self {
        Checker { structs: HashMap::new() }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        // Pre-register struct definitions so field lookups work regardless
        // of where in the file the struct is defined relative to its use.
        for stmt in program {
            if let Stmt::StructDef { name, fields } = stmt {
                self.structs.insert(name.clone(), fields.clone());
            }
        }

        let mut scope = Scope::new();
        let empty: HashSet<String> = HashSet::new();
        self.check_block(program, &mut scope, 0, &empty)
    }

    fn check_block(
        &self,
        stmts: &[Stmt],
        scope: &mut Scope,
        loop_depth: usize,
        loop_locals: &HashSet<String>,
    ) -> Result<(), String> {
        for stmt in stmts {
            self.check_stmt(stmt, scope, loop_depth, loop_locals)?;
        }
        Ok(())
    }

    fn check_stmt(
        &self,
        stmt: &Stmt,
        scope: &mut Scope,
        loop_depth: usize,
        loop_locals: &HashSet<String>,
    ) -> Result<(), String> {
        match stmt {
            Stmt::StructDef { .. } => Ok(()), // already registered in check_program

            Stmt::FnDef { params, body, .. } => {
                // Function bodies are checked in total isolation: a fresh
                // scope containing only the parameters. Warden has no
                // closures, so a function can never see outer bindings —
                // which sidesteps an entire category of capture-move
                // rules real Rust has to handle.
                let mut fn_scope = Scope::new();
                for p in params {
                    fn_scope.insert(p.clone(), VarState::Owned);
                }
                let empty: HashSet<String> = HashSet::new();
                self.check_block(body, &mut fn_scope, 0, &empty)
            }

            Stmt::Let { name, value } => {
                self.check_use(value, scope)?;
                self.apply_move(value, scope, loop_depth, loop_locals)?;
                // A `let` always creates a fresh binding, even if `name`
                // already existed. This is what makes "move, then
                // reassign" legal: shadowing resets the state to Owned
                // regardless of what happened to the old binding.
                scope.insert(name.clone(), VarState::Owned);
                Ok(())
            }

            Stmt::Print(e) | Stmt::ExprStmt(e) => {
                self.check_use(e, scope)?;
                self.apply_move(e, scope, loop_depth, loop_locals)?;
                Ok(())
            }

            Stmt::If { cond, then_branch, else_branch } => {
                // The condition is read, not moved — Warden has no
                // boolean bindings to move, only a truthy numeric check.
                self.check_use(cond, scope)?;

                let mut then_scope = scope.clone();
                self.check_block(then_branch, &mut then_scope, loop_depth, loop_locals)?;

                let mut else_scope = scope.clone();
                if let Some(else_b) = else_branch {
                    self.check_block(else_b, &mut else_scope, loop_depth, loop_locals)?;
                }

                self.merge_branches(scope, &then_scope, &else_scope);
                Ok(())
            }

            Stmt::While { cond, body } => {
                self.check_use(cond, scope)?;

                // Anything `let`-bound directly in the loop body is fresh
                // each iteration, so moving it is safe — only variables
                // that existed *before* the loop are dangerous to move.
                let mut new_loop_locals: HashSet<String> = HashSet::new();
                for s in body {
                    if let Stmt::Let { name, .. } = s {
                        new_loop_locals.insert(name.clone());
                    }
                }

                let mut body_scope = scope.clone();
                self.check_block(body, &mut body_scope, loop_depth + 1, &new_loop_locals)?;

                // Propagate any legitimate state changes (e.g. shadowing)
                // back out for variables that existed before the loop.
                for (k, v) in body_scope.iter() {
                    if scope.contains_key(k) {
                        scope.insert(k.clone(), v.clone());
                    }
                }
                Ok(())
            }
        }
    }

    /// Conservative merge: a variable is only Owned after the if/else if
    /// it was Owned on *both* paths. Any divergence — including "moved
    /// on one path, fine on the other" — is treated as Moved. This is
    /// the simplification called out in the module doc comment; it's
    /// strictly more restrictive than real Rust, not less, so it never
    /// silently accepts something unsound.
    fn merge_branches(&self, scope: &mut Scope, then_scope: &Scope, else_scope: &Scope) {
        let keys: HashSet<&String> = then_scope.keys().chain(else_scope.keys()).collect();
        for k in keys {
            let t = then_scope.get(k);
            let e = else_scope.get(k);
            let merged = match (t, e) {
                (Some(a), Some(b)) if a == b => a.clone(),
                (Some(_), Some(_)) => VarState::Moved,
                (Some(a), None) | (None, Some(a)) => a.clone(),
                (None, None) => continue,
            };
            scope.insert(k.clone(), merged);
        }
    }

    /// Check that `expr` can legally be *read* right now — the value
    /// it refers to hasn't already been moved away. This never mutates
    /// state; `apply_move` does that afterward.
    fn check_use(&self, expr: &Expr, scope: &Scope) -> Result<(), String> {
        match expr {
            Expr::Number(_) => Ok(()),

            Expr::Ident(name) => match scope.get(name) {
                None => Err(format!("use of undeclared variable `{}`", name)),
                Some(VarState::Owned) => Ok(()),
                Some(VarState::Moved) => {
                    Err(format!("use of moved value `{}`", name))
                }
                Some(VarState::PartiallyMoved(fields)) => {
                    let mut moved: Vec<&String> = fields.iter().collect();
                    moved.sort();
                    let list = moved
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    Err(format!(
                        "use of partially moved value `{}` (field(s) already moved: {})",
                        name, list
                    ))
                }
            },

            Expr::Field { base, field } => {
                if let Expr::Ident(name) = base.as_ref() {
                    match scope.get(name) {
                        None => Err(format!("use of undeclared variable `{}`", name)),
                        Some(VarState::Owned) => Ok(()),
                        Some(VarState::Moved) => {
                            Err(format!("use of moved value `{}`", name))
                        }
                        Some(VarState::PartiallyMoved(fields)) => {
                            if fields.contains(field) {
                                Err(format!(
                                    "field `{}` of `{}` was already moved",
                                    field, name
                                ))
                            } else {
                                Ok(())
                            }
                        }
                    }
                } else {
                    // Nested field chains (a.b.c) fall back to checking
                    // the inner base only — one level of partial-move
                    // tracking is enough to make the point without a
                    // full recursive field-path lattice.
                    self.check_use(base, scope)
                }
            }

            Expr::Call { args, .. } => {
                for a in args {
                    self.check_use(a, scope)?;
                }
                Ok(())
            }

            Expr::StructLit { fields, .. } => {
                for (_, v) in fields {
                    self.check_use(v, scope)?;
                }
                Ok(())
            }
        }
    }

    /// After a value has been read successfully, mark whatever it
    /// referenced as moved. This is where loop-move and partial-move
    /// rules actually take effect.
    fn apply_move(
        &self,
        expr: &Expr,
        scope: &mut Scope,
        loop_depth: usize,
        loop_locals: &HashSet<String>,
    ) -> Result<(), String> {
        match expr {
            Expr::Number(_) => Ok(()),

            Expr::Ident(name) => {
                if loop_depth > 0 && !loop_locals.contains(name) {
                    return Err(format!(
                        "cannot move `{}` inside a loop; it was declared outside the loop \
                         and the loop may execute more than once",
                        name
                    ));
                }
                scope.insert(name.clone(), VarState::Moved);
                Ok(())
            }

            Expr::Field { base, field } => {
                if let Expr::Ident(name) = base.as_ref() {
                    if loop_depth > 0 && !loop_locals.contains(name) {
                        return Err(format!(
                            "cannot move field `{}` of `{}` inside a loop; `{}` was declared \
                             outside the loop",
                            field, name, name
                        ));
                    }
                    let entry = scope.entry(name.clone()).or_insert(VarState::Owned);
                    match entry {
                        VarState::Owned => {
                            let mut set = HashSet::new();
                            set.insert(field.clone());
                            *entry = VarState::PartiallyMoved(set);
                        }
                        VarState::PartiallyMoved(set) => {
                            set.insert(field.clone());
                        }
                        VarState::Moved => {
                            // check_use would already have rejected this;
                            // reaching here means base was fully moved.
                            return Err(format!("use of moved value `{}`", name));
                        }
                    }
                    Ok(())
                } else {
                    self.apply_move(base, scope, loop_depth, loop_locals)
                }
            }

            Expr::Call { args, .. } => {
                for a in args {
                    self.apply_move(a, scope, loop_depth, loop_locals)?;
                }
                Ok(())
            }

            Expr::StructLit { fields, .. } => {
                for (_, v) in fields {
                    self.apply_move(v, scope, loop_depth, loop_locals)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> Result<(), String> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Checker::new().check_program(&program)
    }

    #[test]
    fn allows_simple_owned_use() {
        assert!(check("let a = 5;\nprint(a);").is_ok());
    }

    #[test]
    fn rejects_use_after_move() {
        let err = check("let a = 5;\nlet b = a;\nprint(a);").unwrap_err();
        assert!(err.contains("moved value `a`"), "{}", err);
    }

    #[test]
    fn rejects_double_move() {
        let err = check("let a = 5;\nlet b = a;\nlet c = a;").unwrap_err();
        assert!(err.contains("moved value `a`"), "{}", err);
    }

    #[test]
    fn allows_move_then_reassign_via_shadowing() {
        let src = "let a = 5;\nlet b = a;\nlet a = 10;\nprint(a);";
        assert!(check(src).is_ok());
    }

    #[test]
    fn allows_partial_move_of_struct_field() {
        let src = "struct Point { x, y }\n\
                    let p = Point { x: 1, y: 2 };\n\
                    let px = p.x;\n\
                    print(p.y);";
        assert!(check(src).is_ok());
    }

    #[test]
    fn rejects_whole_use_after_partial_move() {
        let src = "struct Point { x, y }\n\
                    let p = Point { x: 1, y: 2 };\n\
                    let px = p.x;\n\
                    print(p);";
        let err = check(src).unwrap_err();
        assert!(err.contains("partially moved"), "{}", err);
    }

    #[test]
    fn rejects_move_of_outer_variable_inside_loop() {
        let src = "let a = 5;\nwhile (a) {\n  let b = a;\n}";
        let err = check(src).unwrap_err();
        assert!(err.contains("inside a loop"), "{}", err);
    }

    #[test]
    fn allows_moving_loop_local_variable_inside_loop() {
        let src = "let a = 5;\nwhile (a) {\n  let x = 1;\n  let y = x;\n}";
        assert!(check(src).is_ok());
    }

    #[test]
    fn rejects_use_after_move_in_one_if_branch() {
        let src = "let a = 5;\nif (a) {\n  let b = a;\n} else {\n  print(0);\n}\nprint(a);";
        let err = check(src).unwrap_err();
        assert!(err.contains("moved value `a`"), "{}", err);
    }

    #[test]
    fn allows_use_after_move_in_both_if_branches_then_reassign() {
        let src = "let a = 5;\n\
                    if (a) {\n  let b = a;\n} else {\n  let c = a;\n}\n\
                    let a = 10;\n\
                    print(a);";
        assert!(check(src).is_ok());
    }

    #[test]
    fn rejects_move_into_function_then_reuse() {
        let src = "fn consume(x) {\n  print(x);\n}\n\
                    let s = 5;\n\
                    consume(s);\n\
                    print(s);";
        let err = check(src).unwrap_err();
        assert!(err.contains("moved value `s`"), "{}", err);
    }
}
