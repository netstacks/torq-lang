use std::collections::HashMap;
use std::fmt;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types::{self, I64};
use cranelift_codegen::ir::instructions::BlockArg;
use cranelift_codegen::ir::{AbiParam, InstBuilder, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable as CraneliftVar};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use torqc_ast::ast::{BinOpKind, Block, Expr, Literal, Pattern, Program, Statement};

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
    variables: HashMap<String, (CraneliftVar, TorqType)>,
    /// Map from block name -> (FuncId, param_count) for user-defined blocks.
    block_funcs: HashMap<String, (FuncId, usize)>,
    /// Stack of loop exit blocks for resolving `break` statements.
    loop_exit_stack: Vec<cranelift_codegen::ir::Block>,
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
            variables: HashMap::new(),
            block_funcs: HashMap::new(),
            loop_exit_stack: Vec::new(),
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
        // Verify that a ::main block exists
        if !program.blocks.iter().any(|b| b.name == "main") {
            return Err(CodegenError::new("no ::main block found"));
        }

        // 1. Declare runtime functions (once, shared across all blocks)
        let rt = self.declare_runtime_funcs()?;

        // ---------------------------------------------------------------
        // PASS 1 — Declare all functions
        // ---------------------------------------------------------------
        for block in &program.blocks {
            if block.name == "main" {
                // ::main → fn() -> i32, exported
                let mut sig = self.module.make_signature();
                sig.returns.push(AbiParam::new(types::I32));
                let func_id = self
                    .module
                    .declare_function("main", Linkage::Export, &sig)
                    .map_err(|e| CodegenError::new(format!("failed to declare main: {}", e)))?;
                self.block_funcs.insert("main".to_string(), (func_id, 0));
            } else {
                // Other blocks → fn(i64, i64, ...) -> i64, local
                let mut sig = self.module.make_signature();
                for _ in &block.params {
                    sig.params.push(AbiParam::new(I64));
                }
                sig.returns.push(AbiParam::new(I64));

                let func_name = format!("torq_block_{}", block.name);
                let func_id = self
                    .module
                    .declare_function(&func_name, Linkage::Local, &sig)
                    .map_err(|e| {
                        CodegenError::new(format!(
                            "failed to declare block '{}': {}",
                            block.name, e
                        ))
                    })?;
                self.block_funcs
                    .insert(block.name.clone(), (func_id, block.params.len()));
            }
        }

        // ---------------------------------------------------------------
        // PASS 2 — Define all functions
        // ---------------------------------------------------------------
        // Clone blocks so we don't hold a borrow on program while mutating self
        let blocks: Vec<Block> = program.blocks.clone();

        for block in &blocks {
            self.compile_block(block, &rt)?;
        }

        let product = self.module.finish();
        let bytes = product
            .emit()
            .map_err(|e| CodegenError::new(format!("failed to emit object: {}", e)))?;
        Ok(bytes)
    }

    /// Compile a single block into its corresponding Cranelift function.
    fn compile_block(&mut self, block: &Block, rt: &RuntimeFuncs) -> Result<(), CodegenError> {
        let is_main = block.name == "main";
        let (func_id, _param_count) = *self
            .block_funcs
            .get(&block.name)
            .ok_or_else(|| CodegenError::new(format!("block '{}' not declared", block.name)))?;

        // Build the function signature (must match what was declared in pass 1)
        let sig = if is_main {
            let mut sig = self.module.make_signature();
            sig.returns.push(AbiParam::new(types::I32));
            sig
        } else {
            let mut sig = self.module.make_signature();
            for _ in &block.params {
                sig.params.push(AbiParam::new(I64));
            }
            sig.returns.push(AbiParam::new(I64));
            sig
        };

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        // Clear per-function variable scope
        self.variables.clear();

        // For non-main blocks: bind block parameters to Cranelift variables
        if !is_main {
            let block_params: Vec<Value> = builder.block_params(entry).to_vec();
            for (i, param) in block.params.iter().enumerate() {
                let cl_var = builder.declare_var(I64);
                builder.def_var(cl_var, block_params[i]);
                self.variables
                    .insert(param.name.clone(), (cl_var, TorqType::I64));
            }
        }

        // Compile the body statements, tracking the last expression result
        let mut last_result: Option<(Value, TorqType)> = None;
        for stmt in &block.body {
            let result = self.compile_statement(stmt, rt, &mut builder)?;
            if result.is_some() {
                last_result = result;
            }
        }

        // Emit return (only if current block is not already terminated)
        if !block_is_terminated(&builder) {
            if is_main {
                // ::main always returns i32 0
                let zero = builder.ins().iconst(types::I32, 0);
                builder.ins().return_(&[zero]);
            } else {
                // Non-main blocks return the last expression value as i64
                let ret_val = if let Some((val, _ty)) = last_result {
                    val
                } else {
                    // No expression result — return 0
                    builder.ins().iconst(I64, 0)
                };
                builder.ins().return_(&[ret_val]);
            }
        }

        builder.finalize();

        self.module
            .define_function(func_id, &mut ctx)
            .map_err(|e| {
                CodegenError::new(format!("failed to define block '{}': {}", block.name, e))
            })?;
        self.module.clear_context(&mut ctx);

        Ok(())
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
                let result = self.compile_pipeline(&pipeline.stages, rt, builder)?;
                Ok(result)
            }
            Statement::Assignment(assign) => {
                // Compile the RHS expression
                let (val, ty) = self.compile_expr(&assign.value, rt, builder, None)?;

                if let Some(&(cl_var, _)) = self.variables.get(&assign.target.name) {
                    // Variable already exists — reuse the Cranelift Variable
                    builder.def_var(cl_var, val);
                    // Update the type in case it changed
                    self.variables.insert(assign.target.name.clone(), (cl_var, ty));
                } else {
                    // New variable — declare_var returns a fresh Variable
                    let cl_type = torq_type_to_cranelift(ty, &self.module);
                    let cl_var = builder.declare_var(cl_type);
                    builder.def_var(cl_var, val);

                    self.variables.insert(assign.target.name.clone(), (cl_var, ty));
                }

                Ok(None)
            }
            Statement::Loop(loop_stmt) => {
                let loop_header = builder.create_block();
                let exit_block = builder.create_block();

                self.loop_exit_stack.push(exit_block);

                // Jump to loop header
                builder.ins().jump(loop_header, &[]);
                builder.switch_to_block(loop_header);
                // Don't seal loop_header yet — it has a back-edge from the end of the body

                // Compile body
                for stmt in &loop_stmt.body {
                    self.compile_statement(stmt, rt, builder)?;
                }

                // Back-edge if not already terminated (e.g., by break in all paths)
                if !block_is_terminated(builder) {
                    builder.ins().jump(loop_header, &[]);
                }

                // Now seal the loop header (all predecessors known: entry + back-edge)
                builder.seal_block(loop_header);

                // Switch to exit block
                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);

                self.loop_exit_stack.pop();
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
        pipe_value: Option<(Value, TorqType)>,
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
            Expr::Variable(var) => {
                if let Some(&(cl_var, ty)) = self.variables.get(&var.name) {
                    let val = builder.use_var(cl_var);
                    Ok((val, ty))
                } else {
                    Err(CodegenError::new(format!(
                        "undefined variable: ${}",
                        var.name
                    )))
                }
            }
            Expr::BinOp(binop) => {
                let (left, _left_ty) = self.compile_expr(&binop.left, rt, builder, pipe_value)?;
                let (right, _right_ty) = self.compile_expr(&binop.right, rt, builder, pipe_value)?;

                let result = match binop.op {
                    BinOpKind::Add => builder.ins().iadd(left, right),
                    BinOpKind::Sub => builder.ins().isub(left, right),
                    BinOpKind::Mul => builder.ins().imul(left, right),
                    BinOpKind::Div => builder.ins().sdiv(left, right),
                    BinOpKind::Mod => builder.ins().srem(left, right),
                    BinOpKind::Gt => {
                        let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, left, right);
                        builder.ins().uextend(types::I64, cmp)
                    }
                    BinOpKind::Lt => {
                        let cmp = builder.ins().icmp(IntCC::SignedLessThan, left, right);
                        builder.ins().uextend(types::I64, cmp)
                    }
                    BinOpKind::GtEq => {
                        let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, left, right);
                        builder.ins().uextend(types::I64, cmp)
                    }
                    BinOpKind::LtEq => {
                        let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, left, right);
                        builder.ins().uextend(types::I64, cmp)
                    }
                    BinOpKind::Eq => {
                        let cmp = builder.ins().icmp(IntCC::Equal, left, right);
                        builder.ins().uextend(types::I64, cmp)
                    }
                    BinOpKind::NotEq => {
                        let cmp = builder.ins().icmp(IntCC::NotEqual, left, right);
                        builder.ins().uextend(types::I64, cmp)
                    }
                    BinOpKind::And => {
                        // Logical AND: both operands are truthy (non-zero)
                        builder.ins().band(left, right)
                    }
                    BinOpKind::Pow => {
                        return Err(CodegenError::new("pow operator not yet supported"));
                    }
                };

                let result_ty = match binop.op {
                    BinOpKind::Gt | BinOpKind::Lt | BinOpKind::GtEq
                    | BinOpKind::LtEq | BinOpKind::Eq | BinOpKind::NotEq => TorqType::Bool,
                    _ => TorqType::I64,
                };
                Ok((result, result_ty))
            }
            Expr::Group(inner, _span) => {
                self.compile_expr(inner, rt, builder, pipe_value)
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
            Expr::BlockCall(call) => {
                // Look up the callee in the block_funcs map
                let (func_id, _expected_params) =
                    *self.block_funcs.get(&call.name).ok_or_else(|| {
                        CodegenError::new(format!("undefined block: ::{}", call.name))
                    })?;

                // Get a function reference usable inside the current function
                let func_ref = self.module.declare_func_in_func(func_id, builder.func);

                // Compile each argument expression
                let mut args = Vec::new();
                for arg_expr in &call.args {
                    let (val, _ty) = self.compile_expr(arg_expr, rt, builder, pipe_value)?;
                    args.push(val);
                }

                // Emit the call instruction
                let inst = builder.ins().call(func_ref, &args);
                let result = builder.inst_results(inst)[0];

                Ok((result, TorqType::I64))
            }
            Expr::Match(m) => {
                // Subject comes from pipe_value
                let (subject, _) = pipe_value
                    .ok_or_else(|| CodegenError::new("match requires pipe input"))?;

                // Create merge block with a block parameter for the result
                let merge_block = builder.create_block();
                builder.append_block_param(merge_block, types::I64);

                for arm in &m.arms {
                    match &arm.pattern {
                        Pattern::Literal(Literal::Int(n, _)) => {
                            let arm_block = builder.create_block();
                            let next_block = builder.create_block();

                            let pat_val = builder.ins().iconst(types::I64, *n);
                            let cmp = builder.ins().icmp(IntCC::Equal, subject, pat_val);
                            builder.ins().brif(cmp, arm_block, &[], next_block, &[]);

                            // Arm body
                            builder.switch_to_block(arm_block);
                            builder.seal_block(arm_block);
                            let (result, _) = self.compile_expr(&arm.body, rt, builder, None)?;
                            builder.ins().jump(merge_block, &[BlockArg::Value(result)]);

                            // Next test
                            builder.switch_to_block(next_block);
                            builder.seal_block(next_block);
                        }
                        Pattern::Wildcard => {
                            let (result, _) = self.compile_expr(&arm.body, rt, builder, None)?;
                            builder.ins().jump(merge_block, &[BlockArg::Value(result)]);
                        }
                        _ => {
                            return Err(CodegenError::new("unsupported match pattern in Phase 4"));
                        }
                    }
                }

                // Switch to merge block
                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);
                let result = builder.block_params(merge_block)[0];
                Ok((result, TorqType::I64))
            }
            Expr::Break(_) => {
                let exit_block = self.loop_exit_stack.last()
                    .ok_or_else(|| CodegenError::new("break outside of loop"))?;
                builder.ins().jump(*exit_block, &[]);

                // Create unreachable block for subsequent instructions in the same arm
                let dead_block = builder.create_block();
                builder.switch_to_block(dead_block);
                builder.seal_block(dead_block);

                Ok((builder.ins().iconst(types::I64, 0), TorqType::Void))
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
// Helper: check if current block is terminated (has a branch/return)
// ---------------------------------------------------------------------------

fn block_is_terminated(builder: &FunctionBuilder) -> bool {
    if let Some(block) = builder.current_block() {
        if let Some(last_inst) = builder.func.layout.last_inst(block) {
            return builder.func.dfg.insts[last_inst].opcode().is_terminator();
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Helper: map TorqType to a Cranelift IR type
// ---------------------------------------------------------------------------

fn torq_type_to_cranelift(ty: TorqType, module: &ObjectModule) -> cranelift_codegen::ir::Type {
    match ty {
        TorqType::I64 => types::I64,
        TorqType::Bool => types::I64,
        TorqType::F64 => types::F64,
        TorqType::Ptr => module.target_config().pointer_type(),
        TorqType::Void => types::I64,
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
