//! A tree-walking interpreter: it evaluates the AST directly, no
//! bytecode, no compilation step. This only runs on programs that have
//! already passed the ownership checker — by the time we get here,
//! we've decided the program is safe, so the interpreter's only job is
//! correctness of evaluation, not safety.

use std::collections::HashMap;
use std::fmt;

use crate::ast::{Expr, Program, Stmt};

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Struct { name: String, fields: HashMap<String, Value> },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Struct { name, fields } => {
                write!(f, "{} {{ ", name)?;
                let mut keys: Vec<&String> = fields.keys().collect();
                keys.sort();
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, fields[*k])?;
                }
                write!(f, " }}")
            }
        }
    }
}

#[derive(Clone)]
struct FnDef {
    params: Vec<String>,
    body: Vec<Stmt>,
}

pub struct Interpreter {
    functions: HashMap<String, FnDef>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter { functions: HashMap::new() }
    }

    pub fn run(&mut self, program: &Program) -> Result<(), String> {
        // Pre-register all function definitions so call order in the
        // source doesn't matter, mirroring how the checker pre-registers
        // structs.
        for stmt in program {
            if let Stmt::FnDef { name, params, body } = stmt {
                self.functions.insert(
                    name.clone(),
                    FnDef { params: params.clone(), body: body.clone() },
                );
            }
        }

        let mut env: HashMap<String, Value> = HashMap::new();
        self.exec_block(program, &mut env)
    }

    fn exec_block(&self, stmts: &[Stmt], env: &mut HashMap<String, Value>) -> Result<(), String> {
        for stmt in stmts {
            self.exec_stmt(stmt, env)?;
        }
        Ok(())
    }

    fn exec_stmt(&self, stmt: &Stmt, env: &mut HashMap<String, Value>) -> Result<(), String> {
        match stmt {
            Stmt::StructDef { .. } | Stmt::FnDef { .. } => Ok(()), // handled at definition time

            Stmt::Let { name, value } => {
                let v = self.eval(value, env)?;
                env.insert(name.clone(), v);
                Ok(())
            }

            Stmt::Print(e) => {
                let v = self.eval(e, env)?;
                println!("{}", v);
                Ok(())
            }

            Stmt::ExprStmt(e) => {
                self.eval(e, env)?;
                Ok(())
            }

            Stmt::If { cond, then_branch, else_branch } => {
                if self.truthy(self.eval(cond, env)?) {
                    self.exec_block(then_branch, env)
                } else if let Some(else_b) = else_branch {
                    self.exec_block(else_b, env)
                } else {
                    Ok(())
                }
            }

            Stmt::While { cond, body } => {
                while self.truthy(self.eval(cond, env)?) {
                    self.exec_block(body, env)?;
                }
                Ok(())
            }
        }
    }

    fn truthy(&self, v: Value) -> bool {
        match v {
            Value::Number(n) => n != 0.0,
            Value::Struct { .. } => true,
        }
    }

    fn eval(&self, expr: &Expr, env: &HashMap<String, Value>) -> Result<Value, String> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),

            Expr::Ident(name) => env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("runtime error: undefined variable `{}`", name)),

            Expr::Field { base, field } => {
                let base_val = self.eval(base, env)?;
                match base_val {
                    Value::Struct { fields, .. } => fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| format!("runtime error: no field `{}`", field)),
                    Value::Number(_) => {
                        Err(format!("runtime error: cannot access field `{}` on a number", field))
                    }
                }
            }

            Expr::StructLit { name, fields } => {
                let mut values = HashMap::new();
                for (fname, fexpr) in fields {
                    values.insert(fname.clone(), self.eval(fexpr, env)?);
                }
                Ok(Value::Struct { name: name.clone(), fields: values })
            }

            Expr::Call { callee, args } => {
                let func = self
                    .functions
                    .get(callee)
                    .ok_or_else(|| format!("runtime error: undefined function `{}`", callee))?;

                if func.params.len() != args.len() {
                    return Err(format!(
                        "runtime error: `{}` expects {} arg(s), got {}",
                        callee,
                        func.params.len(),
                        args.len()
                    ));
                }

                let mut call_env = HashMap::new();
                for (param, arg_expr) in func.params.iter().zip(args) {
                    call_env.insert(param.clone(), self.eval(arg_expr, env)?);
                }

                self.exec_block(&func.body, &mut call_env)?;
                // Warden functions have no return statement — they're
                // used for side effects (print) only. Calling one as an
                // expression yields a nominal 0.
                Ok(Value::Number(0.0))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(src: &str) -> Result<(), String> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Interpreter::new().run(&program)
    }

    #[test]
    fn runs_simple_print() {
        assert!(run("let a = 5;\nprint(a);").is_ok());
    }

    #[test]
    fn runs_struct_field_access() {
        let src = "struct Point { x, y }\n\
                    let p = Point { x: 1, y: 2 };\n\
                    print(p.x);\n\
                    print(p.y);";
        assert!(run(src).is_ok());
    }

    #[test]
    fn runs_function_call() {
        let src = "fn consume(x) {\n  print(x);\n}\nlet s = 5;\nconsume(s);";
        assert!(run(src).is_ok());
    }

    #[test]
    fn while_loop_with_falsy_condition_never_executes_body() {
        // IMPORTANT DESIGN NOTE: Warden has no reassignment statement —
        // only `let`, which always creates a fresh binding. That means
        // a `while` condition can never change value over the loop's
        // lifetime: it's either always truthy (infinite loop) or always
        // falsy (zero iterations). There is no way in Warden today to
        // write a loop that runs a finite, nonzero number of times.
        //
        // This is a genuine gap in the toy language, not a checker
        // limitation — it belongs in the "what we didn't build" section
        // (Phase 7) as an example of a consequence you only discover by
        // actually building the thing. Fixing it would mean adding a
        // real assignment statement, which reopens aliasing questions
        // ownership checking normally has to answer.
        let src = "let n = 0;\nwhile (n) {\n  print(n);\n}";
        assert!(run(src).is_ok());
    }
}
