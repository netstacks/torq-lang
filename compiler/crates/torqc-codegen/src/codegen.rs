use std::fmt;

use cranelift_codegen::ir::{types, AbiParam, InstBuilder};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use torqc_ast::ast::{Expr, Literal, Program, Statement};

// ---------------------------------------------------------------------------
// CodegenError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CodegenError {
    pub message: String,
}

impl CodegenError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "codegen error: {}", self.message)
    }
}

impl std::error::Error for CodegenError {}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

pub struct Compiler {
    module: ObjectModule,
}

impl Compiler {
    /// Create a new Compiler targeting the host machine.
    pub fn new() -> Result<Self, CodegenError> {
        // 1. Build Cranelift settings (flags)
        let mut flag_builder = settings::builder();
        flag_builder
            .set("is_pic", "true")
            .map_err(|e| CodegenError::new(format!("failed to set is_pic: {}", e)))?;
        flag_builder
            .set("opt_level", "speed")
            .map_err(|e| CodegenError::new(format!("failed to set opt_level: {}", e)))?;
        let flags = settings::Flags::new(flag_builder);

        // 2. Detect the host ISA
        let isa_builder = cranelift_native::builder()
            .map_err(|msg| CodegenError::new(format!("failed to detect host ISA: {}", msg)))?;
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| CodegenError::new(format!("failed to build ISA: {}", e)))?;

        // 3. Create the ObjectModule
        let obj_builder = ObjectBuilder::new(isa, "torq_output", cranelift_module::default_libcall_names())
            .map_err(|e| CodegenError::new(format!("failed to create ObjectBuilder: {}", e)))?;
        let module = ObjectModule::new(obj_builder);

        Ok(Self { module })
    }

    /// Compile a TORQ program into object code bytes.
    pub fn compile(mut self, program: &Program) -> Result<Vec<u8>, CodegenError> {
        // 1. Find ::main block
        let main_block = program
            .blocks
            .iter()
            .find(|b| b.name == "main")
            .ok_or_else(|| CodegenError::new("no ::main block found"))?;

        // 2. Declare libc puts function
        let pointer_type = self.module.target_config().pointer_type();

        let mut puts_sig = self.module.make_signature();
        puts_sig.params.push(AbiParam::new(pointer_type));
        puts_sig.returns.push(AbiParam::new(types::I32));

        let puts_id = self
            .module
            .declare_function("puts", Linkage::Import, &puts_sig)
            .map_err(|e| CodegenError::new(format!("failed to declare puts: {}", e)))?;

        // 3. Declare main function: fn() -> i32
        let mut main_sig = self.module.make_signature();
        main_sig.returns.push(AbiParam::new(types::I32));

        let main_id = self
            .module
            .declare_function("main", Linkage::Export, &main_sig)
            .map_err(|e| CodegenError::new(format!("failed to declare main: {}", e)))?;

        let mut ctx = self.module.make_context();
        ctx.func.signature = main_sig;

        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        // 4. Walk ::main block body and generate code
        let mut str_counter: usize = 0;

        for stmt in &main_block.body {
            match stmt {
                Statement::Expression(Expr::Call(call)) if call.name == "print" => {
                    if let Some(arg) = call.args.first() {
                        self.emit_print_literal(
                            arg,
                            &mut str_counter,
                            pointer_type,
                            puts_id,
                            &mut builder,
                        )?;
                    }
                }
                Statement::Pipeline(pipeline) => {
                    // If last stage is print(...), treat first stage as the argument
                    if let Some(Expr::Call(last_call)) = pipeline.stages.last() {
                        if last_call.name == "print" {
                            if let Some(first_stage) = pipeline.stages.first() {
                                self.emit_print_literal(
                                    first_stage,
                                    &mut str_counter,
                                    pointer_type,
                                    puts_id,
                                    &mut builder,
                                )?;
                            }
                        }
                    }
                }
                _ => {
                    // Skip unsupported statements for Phase 3
                }
            }
        }

        // 5. Return 0 from main
        let zero = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[zero]);
        builder.finalize();

        // 6. Define and finalize
        self.module
            .define_function(main_id, &mut ctx)
            .map_err(|e| CodegenError::new(format!("failed to define main: {}", e)))?;
        self.module.clear_context(&mut ctx);

        let product = self.module.finish();
        let bytes = product
            .emit()
            .map_err(|e| CodegenError::new(format!("failed to emit object: {}", e)))?;
        Ok(bytes)
    }

    /// Emit a print call for a literal expression. Converts the literal to a
    /// string, stores it as data, and emits a call to puts.
    fn emit_print_literal(
        &mut self,
        expr: &Expr,
        str_counter: &mut usize,
        pointer_type: cranelift_codegen::ir::Type,
        puts_id: cranelift_module::FuncId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let text = match expr {
            Expr::Literal(Literal::String(s, _)) => s.clone(),
            Expr::Literal(Literal::Int(n, _)) => n.to_string(),
            Expr::Literal(Literal::Bool(b, _)) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Expr::Literal(Literal::Float(f, _)) => f.to_string(),
            Expr::Literal(Literal::Null(_)) => "null".to_string(),
            _ => return Ok(()), // Skip non-literal expressions for now
        };

        let data_id = create_string_data(&mut self.module, str_counter, &text)?;

        let gv = self
            .module
            .declare_data_in_func(data_id, builder.func);
        let ptr = builder.ins().global_value(pointer_type, gv);

        let puts_ref = self
            .module
            .declare_func_in_func(puts_id, builder.func);
        builder.ins().call(puts_ref, &[ptr]);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper: create a null-terminated string in the data section
// ---------------------------------------------------------------------------

fn create_string_data(
    module: &mut ObjectModule,
    counter: &mut usize,
    value: &str,
) -> Result<cranelift_module::DataId, CodegenError> {
    let name = format!(".str.{}", counter);
    *counter += 1;

    let data_id = module
        .declare_data(&name, Linkage::Local, false, false)
        .map_err(|e| CodegenError::new(format!("failed to declare data '{}': {}", name, e)))?;

    let mut desc = DataDescription::new();
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(0); // null terminate for puts
    desc.define(bytes.into_boxed_slice());

    module
        .define_data(data_id, &desc)
        .map_err(|e| CodegenError::new(format!("failed to define data '{}': {}", name, e)))?;

    Ok(data_id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use torqc_ast::ast::{Block, Call, Span};

    fn span() -> Span {
        Span {
            file: "test.torq".into(),
            line: 1,
            col: 1,
        }
    }

    #[test]
    fn compiler_creates_successfully() {
        let compiler = Compiler::new();
        assert!(compiler.is_ok());
    }

    #[test]
    fn missing_main_errors() {
        let compiler = Compiler::new().unwrap();
        let program = Program { blocks: vec![] };
        let result = compiler.compile(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no ::main block"));
    }

    #[test]
    fn empty_main_emits() {
        let compiler = Compiler::new().unwrap();
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let bytes = compiler.compile(&program).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn main_with_print_string() {
        let compiler = Compiler::new().unwrap();
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::Call(Call {
                    name: "print".to_string(),
                    args: vec![Expr::Literal(Literal::String("hello".into(), span()))],
                    span: span(),
                }))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let bytes = compiler.compile(&program).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn main_with_print_integer() {
        let compiler = Compiler::new().unwrap();
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::Call(Call {
                    name: "print".to_string(),
                    args: vec![Expr::Literal(Literal::Int(42, span()))],
                    span: span(),
                }))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let bytes = compiler.compile(&program).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn multiple_prints() {
        let compiler = Compiler::new().unwrap();
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![
                    Statement::Expression(Expr::Call(Call {
                        name: "print".to_string(),
                        args: vec![Expr::Literal(Literal::String("one".into(), span()))],
                        span: span(),
                    })),
                    Statement::Expression(Expr::Call(Call {
                        name: "print".to_string(),
                        args: vec![Expr::Literal(Literal::String("two".into(), span()))],
                        span: span(),
                    })),
                ],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let bytes = compiler.compile(&program).unwrap();
        assert!(!bytes.is_empty());
    }
}
