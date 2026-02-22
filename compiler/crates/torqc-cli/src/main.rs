use std::process;

use clap::{Parser as ClapParser, Subcommand};
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;

#[derive(ClapParser)]
#[command(name = "torqc", version = "0.1.0", about = "TORQ language compiler")]
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
        /// Enable full AI analysis and suggestions
        #[arg(long)]
        ai: bool,
        /// Disable AI error recovery entirely
        #[arg(long, conflicts_with = "ai")]
        no_ai: bool,
        /// Show AI fix details
        #[arg(long)]
        verbose: bool,
    },
    /// Compile and run a .torq file immediately
    Run {
        /// Path to the .torq source file
        file: String,
        /// Enable full AI analysis
        #[arg(long)]
        ai: bool,
        /// Disable AI error recovery
        #[arg(long, conflicts_with = "ai")]
        no_ai: bool,
        /// Show AI fix details
        #[arg(long)]
        verbose: bool,
    },
    /// Run ::test_ blocks in a .torq file
    Test {
        /// Path to the .torq source file
        file: String,
        /// Run only tests matching this filter
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// Initialize a new TORQ project
    Init {
        /// Project name (defaults to current directory name)
        name: Option<String>,
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
        Commands::Build { file, output, ai, no_ai, verbose } => {
            let mode = if no_ai { AiMode::Off } else if ai { AiMode::Full } else { AiMode::Default };
            if let Err(e) = run_build(&file, output.as_deref(), mode, verbose) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Commands::Run { file, ai, no_ai, verbose } => {
            let mode = if no_ai { AiMode::Off } else if ai { AiMode::Full } else { AiMode::Default };
            if let Err(e) = run_run(&file, mode, verbose) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Commands::Test { file, filter } => {
            if let Err(e) = run_test(&file, filter.as_deref()) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Commands::Init { name } => {
            if let Err(e) = run_init(name.as_deref()) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum AiMode {
    Default,
    Full,
    Off,
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

fn run_build(
    path: &str,
    output: Option<&str>,
    ai_mode: AiMode,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let source = try_compile_with_ai(path, &source, ai_mode, verbose)?;

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

    // Codegen
    let compiler = torqc_codegen::codegen::Compiler::new()
        .map_err(|e| format!("codegen init: {}", e))?;
    let program = parse_source(&source, path)?;
    let object_bytes = compiler.compile(&program)
        .map_err(|e| format!("codegen: {}", e))?;

    // Link
    torqc_codegen::linker::link(&object_bytes, &output_path)
        .map_err(|e| format!("link: {}", e))?;

    eprintln!("\u{2713} Built {}", output_path.display());

    Ok(())
}

fn run_run(
    path: &str,
    ai_mode: AiMode,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let source = try_compile_with_ai(path, &source, ai_mode, verbose)?;

    // Codegen
    let compiler = torqc_codegen::codegen::Compiler::new()
        .map_err(|e| format!("codegen init: {}", e))?;
    let program = parse_source(&source, path)?;
    let object_bytes = compiler.compile(&program)
        .map_err(|e| format!("codegen: {}", e))?;

    // Link to temp file
    let temp = std::env::temp_dir().join(format!("torq_run_{}", std::process::id()));
    torqc_codegen::linker::link(&object_bytes, &temp)
        .map_err(|e| format!("link: {}", e))?;

    // Execute
    let status = std::process::Command::new(&temp)
        .status()
        .map_err(|e| format!("failed to execute: {}", e))?;

    let _ = std::fs::remove_file(&temp);

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn parse_source(
    source: &str,
    path: &str,
) -> Result<torqc_ast::ast::Program, Box<dyn std::error::Error>> {
    let tokens = Lexer::tokenize(source, path)
        .map_err(|e| format!("lex error: {}", e))?;
    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;
    Ok(program)
}

/// Try to compile the source, using AI to fix errors if enabled.
/// Returns the (possibly fixed) source code.
fn try_compile_with_ai(
    path: &str,
    source: &str,
    ai_mode: AiMode,
    verbose: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    // First, try to parse and analyze without AI
    let tokens = Lexer::tokenize(source, path)
        .map_err(|e| format!("lex error: {}", e))?;
    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    let analysis = torqc_semantic::analyzer::analyze(&program);
    for diag in &analysis.diagnostics {
        eprintln!("{}", diag);
    }

    eprintln!("\u{2713} Parsed {} block(s)", program.blocks.len());

    if !analysis.has_errors() {
        return Ok(source.to_string());
    }

    // There are errors — try AI fix if enabled
    if matches!(ai_mode, AiMode::Off) {
        return Err(format!("{} error(s), cannot compile", analysis.error_count()).into());
    }

    let config = torqc_ai::config::AiConfig::load_global();
    if config.key.is_none() {
        // No AI configured — fall back to normal error reporting
        if verbose {
            eprintln!("  (AI not configured — run `torqc ai setup` to enable)");
        }
        return Err(format!("{} error(s), cannot compile", analysis.error_count()).into());
    }

    let error = torqc_ai::provider::CompileError {
        stage: "semantic".into(),
        message: analysis
            .diagnostics
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        source: source.to_string(),
        file: path.to_string(),
    };

    eprintln!(
        "\u{26A0} {} error(s) — attempting AI fix...",
        analysis.error_count()
    );

    let fixer = torqc_ai::fixer::create_fixer(&config, verbose)
        .map_err(|e| format!("AI setup failed: {}", e))?;

    let result = fixer.try_fix(&error, |fixed_source| {
        let tokens = Lexer::tokenize(fixed_source, path)
            .map_err(|e| format!("lex error: {}", e))?;
        let program = parser::parse(tokens, path)
            .map_err(|e| format!("parse error: {}", e))?;
        let analysis = torqc_semantic::analyzer::analyze(&program);
        if analysis.has_errors() {
            Err(format!("{} error(s) remain", analysis.error_count()))
        } else {
            Ok(())
        }
    });

    match result {
        torqc_ai::fixer::FixResult::Fixed {
            source: fixed,
            attempts,
            ..
        } => {
            eprintln!(
                "  \u{2192} AI fix applied (attempt {}/{})",
                attempts,
                config.max_retries
            );
            Ok(fixed)
        }
        torqc_ai::fixer::FixResult::Failed {
            original_error,
            attempts,
        } => Err(format!(
            "AI fix failed after {} attempts:\n{}",
            attempts, original_error
        )
        .into()),
    }
}

fn run_test(path: &str, filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;

    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    // Find all ::test_ blocks
    let test_blocks: Vec<&torqc_ast::ast::Block> = program
        .blocks
        .iter()
        .filter(|b| b.name.starts_with("test_"))
        .filter(|b| {
            if let Some(f) = filter {
                b.name.contains(f)
            } else {
                true
            }
        })
        .collect();

    if test_blocks.is_empty() {
        eprintln!("No ::test_ blocks found in {}", path);
        return Ok(());
    }

    eprintln!("Running {} test(s) from {}\n", test_blocks.len(), path);

    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<String> = Vec::new();

    let start_time = std::time::Instant::now();

    for test_block in &test_blocks {
        let test_name = &test_block.name;
        let test_start = std::time::Instant::now();

        // Create a program with this test block renamed to "main"
        let mut test_program = torqc_ast::ast::Program {
            blocks: program.blocks.clone(),
        };

        // Replace the test block with one named "main" (and remove existing main if any)
        test_program.blocks.retain(|b| b.name != "main");
        for block in &mut test_program.blocks {
            if block.name == *test_name {
                block.name = "main".to_string();
            }
        }

        // Compile
        let compiler = match torqc_codegen::codegen::Compiler::new() {
            Ok(c) => c,
            Err(e) => {
                failed += 1;
                let msg = format!("::{}  — compile init error: {}", test_name, e);
                failures.push(msg.clone());
                eprintln!("  \u{2717} {}", msg);
                continue;
            }
        };

        let object_bytes = match compiler.compile(&test_program) {
            Ok(b) => b,
            Err(e) => {
                failed += 1;
                let msg = format!("::{}  — compile error: {}", test_name, e);
                failures.push(msg.clone());
                eprintln!("  \u{2717} {}", msg);
                continue;
            }
        };

        // Link to temp file
        let temp = std::env::temp_dir().join(format!(
            "torq_test_{}_{}", test_name, std::process::id()
        ));

        if let Err(e) = torqc_codegen::linker::link(&object_bytes, &temp) {
            failed += 1;
            let msg = format!("::{}  — link error: {}", test_name, e);
            failures.push(msg.clone());
            eprintln!("  \u{2717} {}", msg);
            continue;
        }

        // Run
        let output = std::process::Command::new(&temp)
            .output();

        let _ = std::fs::remove_file(&temp);

        let elapsed = test_start.elapsed();

        match output {
            Ok(out) if out.status.success() => {
                passed += 1;
                eprintln!("  \u{2713} ::{} ({:.0?})", test_name, elapsed);
            }
            Ok(out) => {
                failed += 1;
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                let detail = if !stderr.is_empty() {
                    stderr.trim().to_string()
                } else if !stdout.is_empty() {
                    stdout.trim().to_string()
                } else {
                    format!("exit code {}", out.status.code().unwrap_or(-1))
                };
                let msg = format!("::{} — {}", test_name, detail);
                failures.push(msg);
                eprintln!("  \u{2717} ::{} ({:.0?})", test_name, elapsed);
            }
            Err(e) => {
                failed += 1;
                let msg = format!("::{} — execution error: {}", test_name, e);
                failures.push(msg);
                eprintln!("  \u{2717} ::{} ({:.0?})", test_name, elapsed);
            }
        }
    }

    let total_elapsed = start_time.elapsed();
    eprintln!();

    if !failures.is_empty() {
        eprintln!("Failures:");
        for f in &failures {
            eprintln!("  {}", f);
        }
        eprintln!();
    }

    eprintln!(
        "{}/{} passed ({} failed) in {:.2?}",
        passed,
        passed + failed,
        failed,
        total_elapsed
    );

    if failed > 0 {
        process::exit(1);
    }

    Ok(())
}

fn run_init(name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = match name {
        Some(n) => n.to_string(),
        None => {
            std::env::current_dir()?
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        }
    };

    // Create torq.yaml
    let yaml = format!(
        "name: {}\nversion: 0.1.0\n\n# services:\n#   example:\n#     spec: https://api.example.com/openapi.json\n#     type: openapi\n#     auth: ${{API_KEY}}\n\n# environment:\n#   production:\n#     log_level: error\n#     port: 8080\n#   development:\n#     log_level: debug\n#     port: 3000\n",
        project_name
    );

    if std::path::Path::new("torq.yaml").exists() {
        return Err("torq.yaml already exists".into());
    }

    std::fs::write("torq.yaml", &yaml)?;
    eprintln!("\u{2713} Created torq.yaml");

    // Create src/main.torq
    std::fs::create_dir_all("src")?;
    if !std::path::Path::new("src/main.torq").exists() {
        std::fs::write(
            "src/main.torq",
            format!("# {}\n\n::main\n  \"Hello from {}!\" | print\n", project_name, project_name),
        )?;
        eprintln!("\u{2713} Created src/main.torq");
    }

    // Create .gitignore for torq_modules
    if !std::path::Path::new(".gitignore").exists() {
        std::fs::write(".gitignore", "torq_modules/\n*.o\n")?;
        eprintln!("\u{2713} Created .gitignore");
    }

    eprintln!("\n\u{2713} Initialized TORQ project '{}'", project_name);
    eprintln!("\n  Build:  torqc build src/main.torq");
    eprintln!("  Run:    torqc run src/main.torq");
    eprintln!("  Test:   torqc test src/main.torq");

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
