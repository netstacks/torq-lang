use std::fmt;

use cranelift_codegen::ir::types::{self, I64};
use cranelift_codegen::ir::{AbiParam, InstBuilder, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
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
// TorqType — tracks the type of a compiled value
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TorqType {
    I64,
    Ptr,
    Bool,
    F64,
    Void,
}

// ---------------------------------------------------------------------------
// Runtime function IDs
// ---------------------------------------------------------------------------

struct RuntimeFuncs {
    print_int: FuncId,
    print_str: FuncId,
    print_bool: FuncId,
    print_float: FuncId,
    print_null: FuncId,
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

pub struct Compiler {
    module: ObjectModule,
    str_counter: usize,
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

        Ok(Self {
            module,
            str_counter: 0,
        })
    }

    /// Declare all TORQ runtime functions and return their FuncIds.
    fn declare_runtime_funcs(&mut self) -> Result<RuntimeFuncs, CodegenError> {
        let pointer_type = self.module.target_config().pointer_type();

        // torq_print_int(int64_t) -> void
        let mut sig_int = self.module.make_signature();
        sig_int.params.push(AbiParam::new(I64));
        let print_int = self
            .module
            .declare_function("torq_print_int", Linkage::Import, &sig_int)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_print_int: {}", e)))?;

        // torq_print_str(const char*) -> void
        let mut sig_str = self.module.make_signature();
        sig_str.params.push(AbiParam::new(pointer_type));
        let print_str = self
            .module
            .declare_function("torq_print_str", Linkage::Import, &sig_str)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_print_str: {}", e)))?;

        // torq_print_bool(int64_t) -> void
        let mut sig_bool = self.module.make_signature();
        sig_bool.params.push(AbiParam::new(I64));
        let print_bool = self
            .module
            .declare_function("torq_print_bool", Linkage::Import, &sig_bool)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_print_bool: {}", e)))?;

        // torq_print_float(double) -> void
        let mut sig_float = self.module.make_signature();
        sig_float.params.push(AbiParam::new(types::F64));
        let print_float = self
            .module
            .declare_function("torq_print_float", Linkage::Import, &sig_float)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_print_float: {}", e)))?;

        // torq_print_null() -> void
        let sig_null = self.module.make_signature();
        let print_null = self
            .module
            .declare_function("torq_print_null", Linkage::Import, &sig_null)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_print_null: {}", e)))?;

        Ok(RuntimeFuncs {
            print_int,
            print_str,
            print_bool,
            print_float,
            print_null,
        })
    }

    /// Compile a TORQ program into object code bytes.
    pub fn compile(mut self, program: &Program) -> Result<Vec<u8>, CodegenError> {
        // 1. Find ::main block
        let main_block = program
            .blocks
            .iter()
            .find(|b| b.name == "main")
            .ok_or_else(|| CodegenError::new("no ::main block found"))?;

        // 2. Declare runtime functions
        let rt = self.declare_runtime_funcs()?;

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
        // Clone the body so we don't hold a borrow on self through main_block
        let body = main_block.body.clone();
        for stmt in &body {
            self.compile_statement(stmt, &rt, &mut builder)?;
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

    // -----------------------------------------------------------------------
    // Statement compilation
    // -----------------------------------------------------------------------

    fn compile_statement(
        &mut self,
        stmt: &Statement,
        rt: &RuntimeFuncs,
        builder: &mut FunctionBuilder,
    ) -> Result<Option<(Value, TorqType)>, CodegenError> {
        match stmt {
            Statement::Expression(Expr::Call(call)) if call.name == "print" => {
                // Direct print call: `print "hello"` or `print 42`
                if let Some(arg) = call.args.first() {
                    let (val, ty) = self.compile_expr(arg, rt, builder, None)?;
                    self.emit_print(val, ty, rt, builder)?;
                }
                Ok(None)
            }
            Statement::Pipeline(pipeline) => {
                self.compile_pipeline(&pipeline.stages, rt, builder)?;
                Ok(None)
            }
            Statement::Expression(expr) => {
                // Compile the expression for its side effects
                let result = self.compile_expr(expr, rt, builder, None)?;
                Ok(Some(result))
            }
            _ => {
                // Skip unsupported statements for now
                Ok(None)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Pipeline compilation
    // -----------------------------------------------------------------------

    fn compile_pipeline(
        &mut self,
        stages: &[Expr],
        rt: &RuntimeFuncs,
        builder: &mut FunctionBuilder,
    ) -> Result<Option<(Value, TorqType)>, CodegenError> {
        let mut pipe_val: Option<(Value, TorqType)> = None;

        for stage in stages {
            match stage {
                Expr::Call(call) if call.name == "print" => {
                    if call.args.is_empty() {
                        // `| print` — print the pipe value
                        if let Some((val, ty)) = pipe_val {
                            self.emit_print(val, ty, rt, builder)?;
                        }
                    } else {
                        // `| print <arg>` — print the explicit argument
                        let (val, ty) = self.compile_expr(call.args.first().unwrap(), rt, builder, pipe_val)?;
                        self.emit_print(val, ty, rt, builder)?;
                    }
                    pipe_val = None;
                }
                _ => {
                    let result = self.compile_expr(stage, rt, builder, pipe_val)?;
                    pipe_val = Some(result);
                }
            }
        }

        Ok(pipe_val)
    }

    // -----------------------------------------------------------------------
    // Expression compilation
    // -----------------------------------------------------------------------

    fn compile_expr(
        &mut self,
        expr: &Expr,
        rt: &RuntimeFuncs,
        builder: &mut FunctionBuilder,
        _pipe_value: Option<(Value, TorqType)>,
    ) -> Result<(Value, TorqType), CodegenError> {
        match expr {
            Expr::Literal(Literal::Int(n, _)) => {
                let val = builder.ins().iconst(I64, *n);
                Ok((val, TorqType::I64))
            }
            Expr::Literal(Literal::String(s, _)) => {
                let pointer_type = self.module.target_config().pointer_type();
                let data_id = create_string_data(&mut self.module, &mut self.str_counter, s)?;
                let gv = self.module.declare_data_in_func(data_id, builder.func);
                let ptr = builder.ins().global_value(pointer_type, gv);
                Ok((ptr, TorqType::Ptr))
            }
            Expr::Literal(Literal::Bool(b, _)) => {
                let val = builder.ins().iconst(I64, if *b { 1 } else { 0 });
                Ok((val, TorqType::Bool))
            }
            Expr::Literal(Literal::Null(_)) => {
                let val = builder.ins().iconst(I64, 0);
                Ok((val, TorqType::Void))
            }
            Expr::Literal(Literal::Float(f, _)) => {
                let val = builder.ins().f64const(*f);
                Ok((val, TorqType::F64))
            }
            Expr::Call(call) if call.name == "print" => {
                // print called as expression (e.g. inside a pipeline)
                if let Some(arg) = call.args.first() {
                    let (val, ty) = self.compile_expr(arg, rt, builder, None)?;
                    self.emit_print(val, ty, rt, builder)?;
                }
                let zero = builder.ins().iconst(I64, 0);
                Ok((zero, TorqType::Void))
            }
            _ => Err(CodegenError::new(format!(
                "unsupported expression: {:?}",
                expr
            ))),
        }
    }

    // -----------------------------------------------------------------------
    // emit_print — dispatch to the correct runtime print function
    // -----------------------------------------------------------------------

    fn emit_print(
        &mut self,
        value: Value,
        ty: TorqType,
        rt: &RuntimeFuncs,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        match ty {
            TorqType::I64 => {
                let func_ref = self.module.declare_func_in_func(rt.print_int, builder.func);
                builder.ins().call(func_ref, &[value]);
            }
            TorqType::Ptr => {
                let func_ref = self.module.declare_func_in_func(rt.print_str, builder.func);
                builder.ins().call(func_ref, &[value]);
            }
            TorqType::Bool => {
                let func_ref = self.module.declare_func_in_func(rt.print_bool, builder.func);
                builder.ins().call(func_ref, &[value]);
            }
            TorqType::F64 => {
                let func_ref = self.module.declare_func_in_func(rt.print_float, builder.func);
                builder.ins().call(func_ref, &[value]);
            }
            TorqType::Void => {
                let func_ref = self.module.declare_func_in_func(rt.print_null, builder.func);
                builder.ins().call(func_ref, &[]);
            }
        }
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
    bytes.push(0); // null terminate
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
