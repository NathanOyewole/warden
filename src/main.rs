use warden::run_source;

fn main() {
    let source = "struct Point { x, y }\n\
                   let p = Point { x: 1, y: 2 };\n\
                   let px = p.x;\n\
                   print(p.y);\n\
                   fn consume(v) {\n  print(v);\n}\n\
                   consume(px);";

    if let Err(e) = run_source(source) {
        eprintln!("{}", e);
    }
}
