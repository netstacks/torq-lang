use std::process;

use clap::{Parser as ClapParser, Subcommand};
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;

#[derive(ClapParser)]
#[command(name = "torqc", about = "TORQ language compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a .torq file and print the AST as JSON
    Parse {
        /// Path to the .torq source file
        file: String,
    },
    /// Check a .torq file for semantic errors
    Check {
        /// Path to the .torq source file
        file: String,
    },
    /// Compile a .torq file into a native binary
    Build {
        /// Path to the .torq source file
        file: String,
        /// Output binary path (defaults to filename without extension)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file } => {
            if let Err(e) = run_parse(&file) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Commands::Check { file } => {
            if let Err(e) = run_check(&file) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Commands::Build { file, output } => {
            if let Err(e) = run_build(&file, output.as_deref()) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
    }
}

fn run_check(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;

    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    let result = torqc_semantic::analyzer::analyze(&program);

    for diag in &result.diagnostics {
        eprintln!("{}", diag);
    }

    let errors = result.error_count();
    let warnings = result.warning_count();

    if errors > 0 {
        eprintln!("\n\u{2717} {} error(s), {} warning(s)", errors, warnings);
        process::exit(1);
    } else if warnings > 0 {
        eprintln!("\n\u{26A0} {} warning(s)", warnings);
    } else {
        eprintln!("\u{2713} No issues found");
    }

    Ok(())
}

fn run_build(path: &str, output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;

    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    // Semantic analysis
    let analysis = torqc_semantic::analyzer::analyze(&program);
    for diag in &analysis.diagnostics {
        eprintln!("{}", diag);
    }
    if analysis.has_errors() {
        return Err(format!("{} error(s), cannot compile", analysis.error_count()).into());
    }

    eprintln!("\u{2713} Parsed {} block(s)", program.blocks.len());

    // Codegen
    let compiler = torqc_codegen::codegen::Compiler::new()
        .map_err(|e| format!("codegen init: {}", e))?;
    let object_bytes = compiler.compile(&program)
        .map_err(|e| format!("codegen: {}", e))?;

    // Determine output path
    let output_path = match output {
        Some(o) => std::path::PathBuf::from(o),
        None => {
            let stem = std::path::Path::new(path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            std::path::PathBuf::from(stem)
        }
    };

    // Link
    torqc_codegen::linker::link(&object_bytes, &output_path)
        .map_err(|e| format!("link: {}", e))?;

    eprintln!("\u{2713} Built {}", output_path.display());

    Ok(())
}

fn run_parse(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;

    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    let json = serde_json::to_string_pretty(&program)?;
    println!("{}", json);

    let block_count = program.blocks.len();
    eprintln!("\u{2713} Parsed {} block(s)", block_count);

    Ok(())
}
