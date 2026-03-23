use rush::lexer::{Lexer, Token};
use rush::parser::Parser;

fn main() {
    let input = "cat <<EOF
hello world
EOF";
    let tokens = Lexer::tokenize(input).unwrap();
    println!("=== Token Stream ===");
    for (i, token) in tokens.iter().enumerate() {
        println!("  [{}] {:?}", i, token);
    }

    let tokens2 = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens2);
    let stmts = parser.parse().unwrap();
    println!("\n=== Statements ({}) ===", stmts.len());
    for (i, stmt) in stmts.iter().enumerate() {
        println!("  [{}] {:?}", i, stmt);
    }
}
