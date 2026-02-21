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
// Runtime function IDs
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct RuntimeFuncs {
    // Constructors
    torq_int: FuncId,       // (i64) -> ptr
    torq_float: FuncId,     // (f64) -> ptr
    torq_bool: FuncId,      // (i64) -> ptr
    torq_str: FuncId,       // (ptr) -> ptr
    torq_null: FuncId,      // () -> ptr
    // Print
    torq_print: FuncId,     // (ptr) -> void
    // Arithmetic
    torq_add: FuncId,       // (ptr, ptr) -> ptr
    torq_sub: FuncId,
    torq_mul: FuncId,
    torq_div: FuncId,
    torq_mod: FuncId,
    // Comparison
    torq_eq: FuncId,        // (ptr, ptr) -> ptr
    torq_neq: FuncId,
    torq_gt: FuncId,
    torq_lt: FuncId,
    torq_gte: FuncId,
    torq_lte: FuncId,
    // Utilities
    torq_is_truthy: FuncId, // (ptr) -> i64
    torq_as_int: FuncId,    // (ptr) -> i64
    // Array
    torq_array_new: FuncId,       // () -> ptr
    torq_array_push_mut: FuncId,  // (ptr, ptr) -> void
    torq_array_len: FuncId,       // (ptr) -> ptr
    torq_array_first: FuncId,     // (ptr) -> ptr
    torq_array_last: FuncId,      // (ptr) -> ptr
    torq_array_get: FuncId,       // (ptr, ptr) -> ptr
    // Dict
    torq_dict_new: FuncId,        // () -> ptr
    torq_dict_set_mut: FuncId,    // (ptr, ptr, ptr) -> void
    torq_dict_get: FuncId,        // (ptr, ptr) -> ptr
    // Unified len
    torq_len: FuncId,             // (ptr) -> ptr
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

pub struct Compiler {
    module: ObjectModule,
    str_counter: usize,
    variables: HashMap<String, CraneliftVar>,
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
        let ptr = self.module.target_config().pointer_type();

        // Signature: (I64) -> ptr  — for torq_int, torq_bool
        let mut sig_i64_to_ptr = self.module.make_signature();
        sig_i64_to_ptr.params.push(AbiParam::new(I64));
        sig_i64_to_ptr.returns.push(AbiParam::new(ptr));

        // Signature: (F64) -> ptr  — for torq_float
        let mut sig_f64_to_ptr = self.module.make_signature();
        sig_f64_to_ptr.params.push(AbiParam::new(types::F64));
        sig_f64_to_ptr.returns.push(AbiParam::new(ptr));

        // Signature: (ptr) -> ptr  — for torq_str
        let mut sig_ptr_to_ptr = self.module.make_signature();
        sig_ptr_to_ptr.params.push(AbiParam::new(ptr));
        sig_ptr_to_ptr.returns.push(AbiParam::new(ptr));

        // Signature: () -> ptr  — for torq_null
        let mut sig_void_to_ptr = self.module.make_signature();
        sig_void_to_ptr.returns.push(AbiParam::new(ptr));

        // Signature: (ptr) -> void  — for torq_print
        let mut sig_ptr_to_void = self.module.make_signature();
        sig_ptr_to_void.params.push(AbiParam::new(ptr));

        // Signature: (ptr, ptr) -> ptr  — for arithmetic and comparison
        let mut sig_pp_to_ptr = self.module.make_signature();
        sig_pp_to_ptr.params.push(AbiParam::new(ptr));
        sig_pp_to_ptr.params.push(AbiParam::new(ptr));
        sig_pp_to_ptr.returns.push(AbiParam::new(ptr));

        // Signature: (ptr) -> I64  — for torq_is_truthy, torq_as_int
        let mut sig_ptr_to_i64 = self.module.make_signature();
        sig_ptr_to_i64.params.push(AbiParam::new(ptr));
        sig_ptr_to_i64.returns.push(AbiParam::new(I64));

        let torq_int = self.module
            .declare_function("torq_int", Linkage::Import, &sig_i64_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_int: {}", e)))?;
        let torq_float = self.module
            .declare_function("torq_float", Linkage::Import, &sig_f64_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_float: {}", e)))?;
        let torq_bool = self.module
            .declare_function("torq_bool", Linkage::Import, &sig_i64_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_bool: {}", e)))?;
        let torq_str = self.module
            .declare_function("torq_str", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str: {}", e)))?;
        let torq_null = self.module
            .declare_function("torq_null", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_null: {}", e)))?;
        let torq_print = self.module
            .declare_function("torq_print", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_print: {}", e)))?;
        let torq_add = self.module
            .declare_function("torq_add", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_add: {}", e)))?;
        let torq_sub = self.module
            .declare_function("torq_sub", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_sub: {}", e)))?;
        let torq_mul = self.module
            .declare_function("torq_mul", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_mul: {}", e)))?;
        let torq_div = self.module
            .declare_function("torq_div", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_div: {}", e)))?;
        let torq_mod = self.module
            .declare_function("torq_mod", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_mod: {}", e)))?;
        let torq_eq = self.module
            .declare_function("torq_eq", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_eq: {}", e)))?;
        let torq_neq = self.module
            .declare_function("torq_neq", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_neq: {}", e)))?;
        let torq_gt = self.module
            .declare_function("torq_gt", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_gt: {}", e)))?;
        let torq_lt = self.module
            .declare_function("torq_lt", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_lt: {}", e)))?;
        let torq_gte = self.module
            .declare_function("torq_gte", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_gte: {}", e)))?;
        let torq_lte = self.module
            .declare_function("torq_lte", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_lte: {}", e)))?;
        let torq_is_truthy = self.module
            .declare_function("torq_is_truthy", Linkage::Import, &sig_ptr_to_i64)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_is_truthy: {}", e)))?;
        let torq_as_int = self.module
            .declare_function("torq_as_int", Linkage::Import, &sig_ptr_to_i64)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_as_int: {}", e)))?;

        // Signature: (ptr, ptr) -> void  — for torq_array_push_mut
        let mut sig_pp_to_void = self.module.make_signature();
        sig_pp_to_void.params.push(AbiParam::new(ptr));
        sig_pp_to_void.params.push(AbiParam::new(ptr));

        let torq_array_new = self.module
            .declare_function("torq_array_new", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_new: {}", e)))?;
        let torq_array_push_mut = self.module
            .declare_function("torq_array_push_mut", Linkage::Import, &sig_pp_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_push_mut: {}", e)))?;
        let torq_array_len = self.module
            .declare_function("torq_array_len", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_len: {}", e)))?;
        let torq_array_first = self.module
            .declare_function("torq_array_first", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_first: {}", e)))?;
        let torq_array_last = self.module
            .declare_function("torq_array_last", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_last: {}", e)))?;
        let torq_array_get = self.module
            .declare_function("torq_array_get", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_get: {}", e)))?;

        // Signature: (ptr, ptr, ptr) -> void  — for torq_dict_set_mut
        let mut sig_ppp_to_void = self.module.make_signature();
        sig_ppp_to_void.params.push(AbiParam::new(ptr));
        sig_ppp_to_void.params.push(AbiParam::new(ptr));
        sig_ppp_to_void.params.push(AbiParam::new(ptr));

        let torq_dict_new = self.module
            .declare_function("torq_dict_new", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_new: {}", e)))?;
        let torq_dict_set_mut = self.module
            .declare_function("torq_dict_set_mut", Linkage::Import, &sig_ppp_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_set_mut: {}", e)))?;
        let torq_dict_get = self.module
            .declare_function("torq_dict_get", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_get: {}", e)))?;
        let torq_len = self.module
            .declare_function("torq_len", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_len: {}", e)))?;

        Ok(RuntimeFuncs {
            torq_int,
            torq_float,
            torq_bool,
            torq_str,
            torq_null,
            torq_print,
            torq_add,
            torq_sub,
            torq_mul,
            torq_div,
            torq_mod,
            torq_eq,
            torq_neq,
            torq_gt,
            torq_lt,
            torq_gte,
            torq_lte,
            torq_is_truthy,
            torq_as_int,
            torq_array_new,
            torq_array_push_mut,
            torq_array_len,
            torq_array_first,
            torq_array_last,
            torq_array_get,
            torq_dict_new,
            torq_dict_set_mut,
            torq_dict_get,
            torq_len,
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
                // ::main -> fn() -> i32, exported
                let mut sig = self.module.make_signature();
                sig.returns.push(AbiParam::new(types::I32));
                let func_id = self
                    .module
                    .declare_function("main", Linkage::Export, &sig)
                    .map_err(|e| CodegenError::new(format!("failed to declare main: {}", e)))?;
                self.block_funcs.insert("main".to_string(), (func_id, 0));
            } else {
                // Other blocks -> fn(ptr, ptr, ...) -> ptr, local
                // Params and return are TorqValue* pointers
                let ptr = self.module.target_config().pointer_type();
                let mut sig = self.module.make_signature();
                for _ in &block.params {
                    sig.params.push(AbiParam::new(ptr));
                }
                sig.returns.push(AbiParam::new(ptr));

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
            let ptr = self.module.target_config().pointer_type();
            let mut sig = self.module.make_signature();
            for _ in &block.params {
                sig.params.push(AbiParam::new(ptr));
            }
            sig.returns.push(AbiParam::new(ptr));
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
                self.variables.insert(param.name.clone(), cl_var);
            }
        }

        // Compile the body statements, tracking the last expression result
        let mut last_result: Option<Value> = None;
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
                let ret_val = if let Some(val) = last_result {
                    val
                } else {
                    // No expression result — return torq_null()
                    let func_ref = self.module.declare_func_in_func(rt.torq_null, builder.func);
                    let inst = builder.ins().call(func_ref, &[]);
                    builder.inst_results(inst)[0]
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
    ) -> Result<Option<Value>, CodegenError> {
        match stmt {
            Statement::Expression(Expr::Call(call)) if call.name == "print" => {
                // Direct print call: `print "hello"` or `print 42`
                if let Some(arg) = call.args.first() {
                    let val = self.compile_expr(arg, rt, builder, None)?;
                    let func_ref = self.module.declare_func_in_func(rt.torq_print, builder.func);
                    builder.ins().call(func_ref, &[val]);
                }
                Ok(None)
            }
            Statement::Pipeline(pipeline) => {
                let result = self.compile_pipeline(&pipeline.stages, rt, builder)?;
                Ok(result)
            }
            Statement::Assignment(assign) => {
                // Compile the RHS expression
                let val = self.compile_expr(&assign.value, rt, builder, None)?;

                if let Some(&cl_var) = self.variables.get(&assign.target.name) {
                    // Variable already exists — reuse the Cranelift Variable
                    builder.def_var(cl_var, val);
                } else {
                    // New variable — declare_var returns a fresh Variable
                    let cl_var = builder.declare_var(I64);
                    builder.def_var(cl_var, val);
                    self.variables.insert(assign.target.name.clone(), cl_var);
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
            Statement::Each(each) => {
                if !each.sequential {
                    return Err(CodegenError::new("parallel each not supported in Phase 4"));
                }

                // Only support range(...) as iterable for Phase 4
                let (start_val, end_val) = match &*each.iterable {
                    Expr::Call(call) if call.name == "range" && call.args.len() == 2 => {
                        // Compile range args (they produce TorqValue*)
                        let start_ptr = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let end_ptr = self.compile_expr(&call.args[1], rt, builder, None)?;
                        // Extract raw i64 for efficient counter
                        let as_int_fn = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                        let inst = builder.ins().call(as_int_fn, &[start_ptr]);
                        let start = builder.inst_results(inst)[0];
                        let as_int_fn2 = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                        let inst = builder.ins().call(as_int_fn2, &[end_ptr]);
                        let end = builder.inst_results(inst)[0];
                        (start, end)
                    }
                    _ => return Err(CodegenError::new(
                        "each sequential only supports range() iterable in Phase 4"
                    )),
                };

                let loop_header = builder.create_block();
                builder.append_block_param(loop_header, types::I64); // counter param
                let body_block = builder.create_block();
                let exit_block = builder.create_block();

                self.loop_exit_stack.push(exit_block);

                // Jump to header with start value
                builder.ins().jump(loop_header, &[BlockArg::Value(start_val)]);
                builder.switch_to_block(loop_header);

                let counter = builder.block_params(loop_header)[0];

                // Check: counter >= end -> exit
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, counter, end_val);
                builder.ins().brif(cmp, exit_block, &[], body_block, &[]);

                builder.switch_to_block(body_block);
                builder.seal_block(body_block);

                // Box the counter as TorqValue* for the loop variable
                let int_fn = self.module.declare_func_in_func(rt.torq_int, builder.func);
                let inst = builder.ins().call(int_fn, &[counter]);
                let boxed_counter = builder.inst_results(inst)[0];

                let cl_var = builder.declare_var(types::I64);
                builder.def_var(cl_var, boxed_counter);
                self.variables.insert(each.binding.name.clone(), cl_var);

                // Compile body
                for stmt in &each.body {
                    self.compile_statement(stmt, rt, builder)?;
                }

                // Increment counter and loop back
                if !block_is_terminated(builder) {
                    let one = builder.ins().iconst(types::I64, 1);
                    let next = builder.ins().iadd(counter, one);
                    builder.ins().jump(loop_header, &[BlockArg::Value(next)]);
                }

                builder.seal_block(loop_header); // seal after back-edge
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
            // All Statement variants are now covered; no wildcard needed.
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
    ) -> Result<Option<Value>, CodegenError> {
        let mut pipe_val: Option<Value> = None;

        for stage in stages {
            match stage {
                Expr::Call(call) if call.name == "print" => {
                    if call.args.is_empty() {
                        // `| print` — print the pipe value
                        if let Some(val) = pipe_val {
                            let func_ref = self.module.declare_func_in_func(rt.torq_print, builder.func);
                            builder.ins().call(func_ref, &[val]);
                        }
                    } else {
                        // `| print <arg>` — print the explicit argument
                        let val = self.compile_expr(call.args.first().unwrap(), rt, builder, pipe_val)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_print, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                Expr::Call(call) if call.name == "len" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_len, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "first" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_first, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "last" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_last, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
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
        pipe_value: Option<Value>,
    ) -> Result<Value, CodegenError> {
        match expr {
            Expr::Literal(Literal::Int(n, _)) => {
                let val = builder.ins().iconst(I64, *n);
                let func_ref = self.module.declare_func_in_func(rt.torq_int, builder.func);
                let inst = builder.ins().call(func_ref, &[val]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Literal(Literal::String(s, _)) => {
                let pointer_type = self.module.target_config().pointer_type();
                let data_id = create_string_data(&mut self.module, &mut self.str_counter, s)?;
                let gv = self.module.declare_data_in_func(data_id, builder.func);
                let ptr = builder.ins().global_value(pointer_type, gv);
                let func_ref = self.module.declare_func_in_func(rt.torq_str, builder.func);
                let inst = builder.ins().call(func_ref, &[ptr]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Literal(Literal::Bool(b, _)) => {
                let val = builder.ins().iconst(I64, if *b { 1 } else { 0 });
                let func_ref = self.module.declare_func_in_func(rt.torq_bool, builder.func);
                let inst = builder.ins().call(func_ref, &[val]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Literal(Literal::Null(_)) => {
                let func_ref = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(func_ref, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Literal(Literal::Float(f, _)) => {
                let val = builder.ins().f64const(*f);
                let func_ref = self.module.declare_func_in_func(rt.torq_float, builder.func);
                let inst = builder.ins().call(func_ref, &[val]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Variable(var) => {
                if let Some(&cl_var) = self.variables.get(&var.name) {
                    Ok(builder.use_var(cl_var))
                } else {
                    Err(CodegenError::new(format!(
                        "undefined variable: ${}",
                        var.name
                    )))
                }
            }
            Expr::BinOp(binop) => {
                let left = self.compile_expr(&binop.left, rt, builder, pipe_value)?;
                let right = self.compile_expr(&binop.right, rt, builder, pipe_value)?;

                let func_id = match binop.op {
                    BinOpKind::Add => rt.torq_add,
                    BinOpKind::Sub => rt.torq_sub,
                    BinOpKind::Mul => rt.torq_mul,
                    BinOpKind::Div => rt.torq_div,
                    BinOpKind::Mod => rt.torq_mod,
                    BinOpKind::Gt => rt.torq_gt,
                    BinOpKind::Lt => rt.torq_lt,
                    BinOpKind::GtEq => rt.torq_gte,
                    BinOpKind::LtEq => rt.torq_lte,
                    BinOpKind::Eq => rt.torq_eq,
                    BinOpKind::NotEq => rt.torq_neq,
                    BinOpKind::And => {
                        // Logical AND: check truthiness of both, return bool
                        let truthy_fn = self.module.declare_func_in_func(rt.torq_is_truthy, builder.func);
                        let inst_a = builder.ins().call(truthy_fn, &[left]);
                        let ta = builder.inst_results(inst_a)[0];
                        let truthy_fn2 = self.module.declare_func_in_func(rt.torq_is_truthy, builder.func);
                        let inst_b = builder.ins().call(truthy_fn2, &[right]);
                        let tb = builder.inst_results(inst_b)[0];
                        let result = builder.ins().band(ta, tb);
                        let bool_fn = self.module.declare_func_in_func(rt.torq_bool, builder.func);
                        let inst = builder.ins().call(bool_fn, &[result]);
                        return Ok(builder.inst_results(inst)[0]);
                    }
                    BinOpKind::Pow => {
                        return Err(CodegenError::new("pow operator not yet supported"));
                    }
                };
                let func_ref = self.module.declare_func_in_func(func_id, builder.func);
                let inst = builder.ins().call(func_ref, &[left, right]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Group(inner, _span) => {
                self.compile_expr(inner, rt, builder, pipe_value)
            }
            Expr::Call(call) if call.name == "print" => {
                // print called as expression (e.g. inside a pipeline)
                if let Some(arg) = call.args.first() {
                    let val = self.compile_expr(arg, rt, builder, None)?;
                    let func_ref = self.module.declare_func_in_func(rt.torq_print, builder.func);
                    builder.ins().call(func_ref, &[val]);
                }
                // Return null as the expression result
                let func_ref = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(func_ref, &[]);
                Ok(builder.inst_results(inst)[0])
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
                    let val = self.compile_expr(arg_expr, rt, builder, pipe_value)?;
                    args.push(val);
                }

                // Emit the call instruction
                let inst = builder.ins().call(func_ref, &args);
                let result = builder.inst_results(inst)[0];

                Ok(result)
            }
            Expr::Match(m) => {
                // Subject comes from pipe_value
                let subject = pipe_value
                    .ok_or_else(|| CodegenError::new("match requires pipe input"))?;

                // Create merge block with a block parameter for the result
                let merge_block = builder.create_block();
                builder.append_block_param(merge_block, types::I64);

                for arm in &m.arms {
                    match &arm.pattern {
                        Pattern::Literal(Literal::Int(n, _)) => {
                            let arm_block = builder.create_block();
                            let next_block = builder.create_block();

                            // Create pattern value as TorqValue*
                            let pat_raw = builder.ins().iconst(types::I64, *n);
                            let int_fn = self.module.declare_func_in_func(rt.torq_int, builder.func);
                            let inst = builder.ins().call(int_fn, &[pat_raw]);
                            let pat_val = builder.inst_results(inst)[0];

                            // Compare: torq_eq(subject, pattern) -> TorqValue* bool
                            let eq_fn = self.module.declare_func_in_func(rt.torq_eq, builder.func);
                            let inst = builder.ins().call(eq_fn, &[subject, pat_val]);
                            let eq_result = builder.inst_results(inst)[0];

                            // Extract truthy: torq_is_truthy(eq_result) -> raw i64
                            let truthy_fn = self.module.declare_func_in_func(rt.torq_is_truthy, builder.func);
                            let inst = builder.ins().call(truthy_fn, &[eq_result]);
                            let cmp = builder.inst_results(inst)[0];

                            builder.ins().brif(cmp, arm_block, &[], next_block, &[]);

                            // Arm body
                            builder.switch_to_block(arm_block);
                            builder.seal_block(arm_block);
                            let result = self.compile_expr(&arm.body, rt, builder, None)?;
                            if !block_is_terminated(builder) {
                                builder.ins().jump(merge_block, &[BlockArg::Value(result)]);
                            }

                            // Next test
                            builder.switch_to_block(next_block);
                            builder.seal_block(next_block);
                        }
                        Pattern::Wildcard => {
                            let result = self.compile_expr(&arm.body, rt, builder, None)?;
                            if !block_is_terminated(builder) {
                                builder.ins().jump(merge_block, &[BlockArg::Value(result)]);
                            }
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
                Ok(result)
            }
            Expr::Break(_) => {
                let exit_block = self.loop_exit_stack.last()
                    .ok_or_else(|| CodegenError::new("break outside of loop"))?;
                builder.ins().jump(*exit_block, &[]);

                // Create unreachable block for subsequent instructions in the same arm
                let dead_block = builder.create_block();
                builder.switch_to_block(dead_block);
                builder.seal_block(dead_block);

                // Dummy value (never used -- unreachable code)
                Ok(builder.ins().iconst(types::I64, 0))
            }
            Expr::Array(elements, _) => {
                // Create new array
                let new_fn = self.module.declare_func_in_func(rt.torq_array_new, builder.func);
                let inst = builder.ins().call(new_fn, &[]);
                let arr = builder.inst_results(inst)[0];

                // Push each element
                let push_fn = self.module.declare_func_in_func(rt.torq_array_push_mut, builder.func);
                for elem in elements {
                    let val = self.compile_expr(elem, rt, builder, None)?;
                    builder.ins().call(push_fn, &[arr, val]);
                }

                Ok(arr)
            }
            Expr::Dict(entries, _) => {
                let new_fn = self.module.declare_func_in_func(rt.torq_dict_new, builder.func);
                let inst = builder.ins().call(new_fn, &[]);
                let dict = builder.inst_results(inst)[0];

                let set_fn = self.module.declare_func_in_func(rt.torq_dict_set_mut, builder.func);
                let pointer_type = self.module.target_config().pointer_type();
                for entry in entries {
                    // Create string data for key
                    let key_data = create_string_data(&mut self.module, &mut self.str_counter, &entry.key)?;
                    let key_gv = self.module.declare_data_in_func(key_data, builder.func);
                    let key_ptr = builder.ins().global_value(pointer_type, key_gv);

                    // Compile value expression
                    let val = self.compile_expr(&entry.value, rt, builder, None)?;

                    builder.ins().call(set_fn, &[dict, key_ptr, val]);
                }

                Ok(dict)
            }
            Expr::MemberAccess(access) => {
                let obj = self.compile_expr(&access.object, rt, builder, pipe_value)?;
                let pointer_type = self.module.target_config().pointer_type();
                let key_data = create_string_data(&mut self.module, &mut self.str_counter, &access.field)?;
                let key_gv = self.module.declare_data_in_func(key_data, builder.func);
                let key_ptr = builder.ins().global_value(pointer_type, key_gv);

                let get_fn = self.module.declare_func_in_func(rt.torq_dict_get, builder.func);
                let inst = builder.ins().call(get_fn, &[obj, key_ptr]);
                Ok(builder.inst_results(inst)[0])
            }
            _ => Err(CodegenError::new(format!(
                "unsupported expression: {:?}",
                expr
            ))),
        }
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
