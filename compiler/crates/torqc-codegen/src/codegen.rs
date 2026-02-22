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

use torqc_ast::ast::{BinOpKind, Block, Expr, Literal, Pattern, Program, Statement, StringPart};

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
    // String operations
    torq_str_upper: FuncId,       // (ptr) -> ptr
    torq_str_lower: FuncId,       // (ptr) -> ptr
    torq_str_trim: FuncId,        // (ptr) -> ptr
    torq_str_contains: FuncId,    // (ptr, ptr) -> ptr
    torq_str_replace: FuncId,     // (ptr, ptr, ptr) -> ptr
    torq_str_split: FuncId,       // (ptr, ptr) -> ptr
    torq_str_starts: FuncId,      // (ptr, ptr) -> ptr
    torq_str_ends: FuncId,        // (ptr, ptr) -> ptr
    torq_str_slice: FuncId,       // (ptr, ptr, ptr) -> ptr
    torq_str_reverse: FuncId,     // (ptr) -> ptr
    torq_join: FuncId,            // (ptr, ptr) -> ptr
    // String interpolation support
    torq_to_string: FuncId,       // (ptr) -> ptr
    torq_str_concat: FuncId,      // (ptr, ptr) -> ptr
    // Math
    torq_math_abs: FuncId,        // (ptr) -> ptr
    torq_math_sqrt: FuncId,       // (ptr) -> ptr
    torq_math_floor: FuncId,      // (ptr) -> ptr
    torq_math_ceil: FuncId,       // (ptr) -> ptr
    torq_math_round: FuncId,      // (ptr) -> ptr
    torq_math_min: FuncId,        // (ptr, ptr) -> ptr
    torq_math_max: FuncId,        // (ptr, ptr) -> ptr
    // I/O
    torq_fs_read: FuncId,         // (ptr) -> ptr
    torq_fs_write: FuncId,        // (ptr, ptr) -> void
    torq_env: FuncId,             // (ptr) -> ptr
    torq_log: FuncId,             // (ptr) -> void
    torq_exit: FuncId,            // (ptr) -> void
    // JSON
    torq_to_json: FuncId,         // (ptr) -> ptr
    torq_from_json: FuncId,       // (ptr) -> ptr
    // Advanced array ops
    torq_array_push: FuncId,      // (ptr, ptr) -> ptr
    torq_array_pop: FuncId,       // (ptr) -> ptr
    torq_array_shift: FuncId,     // (ptr) -> ptr
    torq_array_at: FuncId,        // (ptr, ptr) -> ptr
    torq_array_sort: FuncId,      // (ptr) -> ptr
    torq_array_reverse: FuncId,   // (ptr) -> ptr
    torq_array_sum: FuncId,       // (ptr) -> ptr
    torq_array_unique: FuncId,    // (ptr) -> ptr
    torq_array_flatten: FuncId,   // (ptr) -> ptr
    torq_array_contains: FuncId,  // (ptr, ptr) -> ptr
    torq_array_slice: FuncId,     // (ptr, ptr, ptr) -> ptr
    torq_array_map_field: FuncId, // (ptr, ptr) -> ptr
    torq_array_filter_field: FuncId, // (ptr, ptr) -> ptr
    torq_array_empty: FuncId,     // (ptr) -> ptr
    // Advanced dict ops
    torq_dict_set: FuncId,        // (ptr, ptr, ptr) -> ptr
    torq_dict_drop: FuncId,       // (ptr, ptr) -> ptr
    torq_dict_merge: FuncId,      // (ptr, ptr) -> ptr
    torq_dict_pick: FuncId,       // (ptr, ptr) -> ptr
    torq_dict_omit: FuncId,       // (ptr, ptr) -> ptr
    torq_dict_entries: FuncId,    // (ptr) -> ptr
    torq_dict_empty: FuncId,      // (ptr) -> ptr
    torq_dict_keys: FuncId,       // (ptr) -> ptr
    torq_dict_values: FuncId,     // (ptr) -> ptr
    torq_dict_has_tv: FuncId,     // (ptr, ptr) -> ptr  (TorqValue* key wrapper)
    torq_dict_get_tv: FuncId,     // (ptr, ptr) -> ptr  (TorqValue* key wrapper)
    // Power
    torq_pow: FuncId,             // (ptr, ptr) -> ptr
    // Unified reverse (dispatches on type)
    torq_reverse: FuncId,         // (ptr) -> ptr
    // Parallel each
    torq_parallel_each_array: FuncId,  // (ptr, ptr) -> void
    torq_parallel_each_range: FuncId,  // (i64, i64, ptr) -> void
    // System
    torq_sys_exec: FuncId,        // (ptr) -> ptr
    torq_type_of: FuncId,         // (ptr) -> ptr
    // Time
    torq_time_now: FuncId,        // () -> ptr
    torq_time_unix: FuncId,       // () -> ptr
    torq_time_format: FuncId,     // (ptr, ptr) -> ptr
    torq_time_parse: FuncId,      // (ptr, ptr) -> ptr
    torq_time_diff: FuncId,       // (ptr, ptr) -> ptr
    torq_time_add: FuncId,        // (ptr, ptr) -> ptr
    torq_time_sleep: FuncId,      // (ptr) -> void
    // HTTP
    torq_http_get: FuncId,        // (ptr) -> ptr
    torq_http_post: FuncId,       // (ptr, ptr) -> ptr
    torq_http_put: FuncId,        // (ptr, ptr) -> ptr
    torq_http_delete: FuncId,     // (ptr) -> ptr
    // Crypto
    torq_crypto_hash: FuncId,     // (ptr, ptr) -> ptr
    torq_crypto_uuid: FuncId,     // () -> ptr
    // Logging
    torq_log_info: FuncId,        // (ptr) -> void
    torq_log_warn: FuncId,        // (ptr) -> void
    torq_log_err: FuncId,         // (ptr) -> void
    torq_log_debug: FuncId,       // (ptr) -> void
    // Math (additional)
    torq_math_random: FuncId,     // () -> ptr
    // Assert
    torq_assert: FuncId,          // (ptr, ptr) -> void
    torq_assert_eq: FuncId,       // (ptr, ptr) -> void
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
    /// Counter for generating unique parallel-each body function names.
    each_body_counter: usize,
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
            each_body_counter: 0,
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

        // Signature: (ptr, ptr, ptr) -> ptr  — for torq_str_replace, torq_str_slice
        let mut sig_ppp_to_ptr = self.module.make_signature();
        sig_ppp_to_ptr.params.push(AbiParam::new(ptr));
        sig_ppp_to_ptr.params.push(AbiParam::new(ptr));
        sig_ppp_to_ptr.params.push(AbiParam::new(ptr));
        sig_ppp_to_ptr.returns.push(AbiParam::new(ptr));

        let torq_str_upper = self.module
            .declare_function("torq_str_upper", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_upper: {}", e)))?;
        let torq_str_lower = self.module
            .declare_function("torq_str_lower", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_lower: {}", e)))?;
        let torq_str_trim = self.module
            .declare_function("torq_str_trim", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_trim: {}", e)))?;
        let torq_str_contains = self.module
            .declare_function("torq_str_contains", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_contains: {}", e)))?;
        let torq_str_replace = self.module
            .declare_function("torq_str_replace", Linkage::Import, &sig_ppp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_replace: {}", e)))?;
        let torq_str_split = self.module
            .declare_function("torq_str_split", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_split: {}", e)))?;
        let torq_str_starts = self.module
            .declare_function("torq_str_starts", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_starts: {}", e)))?;
        let torq_str_ends = self.module
            .declare_function("torq_str_ends", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_ends: {}", e)))?;
        let torq_str_slice = self.module
            .declare_function("torq_str_slice", Linkage::Import, &sig_ppp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_slice: {}", e)))?;
        let torq_str_reverse = self.module
            .declare_function("torq_str_reverse", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_reverse: {}", e)))?;
        let torq_join = self.module
            .declare_function("torq_join", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_join: {}", e)))?;
        let torq_to_string = self.module
            .declare_function("torq_to_string", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_to_string: {}", e)))?;
        let torq_str_concat = self.module
            .declare_function("torq_str_concat", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_str_concat: {}", e)))?;

        // Math functions
        let torq_math_abs = self.module
            .declare_function("torq_math_abs", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_abs: {}", e)))?;
        let torq_math_sqrt = self.module
            .declare_function("torq_math_sqrt", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_sqrt: {}", e)))?;
        let torq_math_floor = self.module
            .declare_function("torq_math_floor", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_floor: {}", e)))?;
        let torq_math_ceil = self.module
            .declare_function("torq_math_ceil", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_ceil: {}", e)))?;
        let torq_math_round = self.module
            .declare_function("torq_math_round", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_round: {}", e)))?;
        let torq_math_min = self.module
            .declare_function("torq_math_min", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_min: {}", e)))?;
        let torq_math_max = self.module
            .declare_function("torq_math_max", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_max: {}", e)))?;

        // I/O functions
        let torq_fs_read = self.module
            .declare_function("torq_fs_read", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_fs_read: {}", e)))?;
        let torq_fs_write = self.module
            .declare_function("torq_fs_write", Linkage::Import, &sig_pp_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_fs_write: {}", e)))?;
        let torq_env = self.module
            .declare_function("torq_env", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_env: {}", e)))?;
        let torq_log = self.module
            .declare_function("torq_log", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_log: {}", e)))?;
        let torq_exit = self.module
            .declare_function("torq_exit", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_exit: {}", e)))?;

        // JSON functions
        let torq_to_json = self.module
            .declare_function("torq_to_json", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_to_json: {}", e)))?;
        let torq_from_json = self.module
            .declare_function("torq_from_json", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_from_json: {}", e)))?;

        // Advanced array operations
        let torq_array_push = self.module
            .declare_function("torq_array_push", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_push: {}", e)))?;
        let torq_array_pop = self.module
            .declare_function("torq_array_pop", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_pop: {}", e)))?;
        let torq_array_shift = self.module
            .declare_function("torq_array_shift", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_shift: {}", e)))?;
        let torq_array_at = self.module
            .declare_function("torq_array_at", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_at: {}", e)))?;
        let torq_array_sort = self.module
            .declare_function("torq_array_sort", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_sort: {}", e)))?;
        let torq_array_reverse = self.module
            .declare_function("torq_array_reverse", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_reverse: {}", e)))?;
        let torq_array_sum = self.module
            .declare_function("torq_array_sum", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_sum: {}", e)))?;
        let torq_array_unique = self.module
            .declare_function("torq_array_unique", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_unique: {}", e)))?;
        let torq_array_flatten = self.module
            .declare_function("torq_array_flatten", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_flatten: {}", e)))?;
        let torq_array_contains = self.module
            .declare_function("torq_array_contains", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_contains: {}", e)))?;
        let torq_array_slice = self.module
            .declare_function("torq_array_slice", Linkage::Import, &sig_ppp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_slice: {}", e)))?;
        let torq_array_map_field = self.module
            .declare_function("torq_array_map_field", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_map_field: {}", e)))?;
        let torq_array_filter_field = self.module
            .declare_function("torq_array_filter_field", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_filter_field: {}", e)))?;
        let torq_array_empty = self.module
            .declare_function("torq_array_empty", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_array_empty: {}", e)))?;

        // Advanced dict operations
        let torq_dict_set = self.module
            .declare_function("torq_dict_set", Linkage::Import, &sig_ppp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_set: {}", e)))?;
        let torq_dict_drop = self.module
            .declare_function("torq_dict_drop", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_drop: {}", e)))?;
        let torq_dict_merge = self.module
            .declare_function("torq_dict_merge", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_merge: {}", e)))?;
        let torq_dict_pick = self.module
            .declare_function("torq_dict_pick", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_pick: {}", e)))?;
        let torq_dict_omit = self.module
            .declare_function("torq_dict_omit", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_omit: {}", e)))?;
        let torq_dict_entries = self.module
            .declare_function("torq_dict_entries", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_entries: {}", e)))?;
        let torq_dict_empty = self.module
            .declare_function("torq_dict_empty", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_empty: {}", e)))?;
        let torq_dict_keys = self.module
            .declare_function("torq_dict_keys", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_keys: {}", e)))?;
        let torq_dict_values = self.module
            .declare_function("torq_dict_values", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_values: {}", e)))?;
        let torq_dict_has_tv = self.module
            .declare_function("torq_dict_has_tv", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_has_tv: {}", e)))?;
        let torq_dict_get_tv = self.module
            .declare_function("torq_dict_get_tv", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_dict_get_tv: {}", e)))?;

        // Unified reverse
        let torq_reverse = self.module
            .declare_function("torq_reverse", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_reverse: {}", e)))?;

        // Parallel each: torq_parallel_each_array(ptr_array, ptr_fn_ptr) -> void
        let mut sig_pp_to_void_par = self.module.make_signature();
        sig_pp_to_void_par.params.push(AbiParam::new(ptr));
        sig_pp_to_void_par.params.push(AbiParam::new(ptr));
        let torq_parallel_each_array = self.module
            .declare_function("torq_parallel_each_array", Linkage::Import, &sig_pp_to_void_par)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_parallel_each_array: {}", e)))?;

        // Parallel each range: torq_parallel_each_range(i64_start, i64_end, ptr_fn_ptr) -> void
        let mut sig_iip_to_void = self.module.make_signature();
        sig_iip_to_void.params.push(AbiParam::new(I64));
        sig_iip_to_void.params.push(AbiParam::new(I64));
        sig_iip_to_void.params.push(AbiParam::new(ptr));
        let torq_parallel_each_range = self.module
            .declare_function("torq_parallel_each_range", Linkage::Import, &sig_iip_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_parallel_each_range: {}", e)))?;

        // Power operator
        let torq_pow = self.module
            .declare_function("torq_pow", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_pow: {}", e)))?;

        // System operations
        let torq_sys_exec = self.module
            .declare_function("torq_sys_exec", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_sys_exec: {}", e)))?;
        let torq_type_of = self.module
            .declare_function("torq_type_of", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_type_of: {}", e)))?;

        // Time functions
        let torq_time_now = self.module
            .declare_function("torq_time_now", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_now: {}", e)))?;
        let torq_time_unix = self.module
            .declare_function("torq_time_unix", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_unix: {}", e)))?;
        let torq_time_format = self.module
            .declare_function("torq_time_format", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_format: {}", e)))?;
        let torq_time_parse = self.module
            .declare_function("torq_time_parse", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_parse: {}", e)))?;
        let torq_time_diff = self.module
            .declare_function("torq_time_diff", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_diff: {}", e)))?;
        let torq_time_add = self.module
            .declare_function("torq_time_add", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_add: {}", e)))?;
        let torq_time_sleep = self.module
            .declare_function("torq_time_sleep", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_time_sleep: {}", e)))?;

        // HTTP functions
        let torq_http_get = self.module
            .declare_function("torq_http_get", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_http_get: {}", e)))?;
        let torq_http_post = self.module
            .declare_function("torq_http_post", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_http_post: {}", e)))?;
        let torq_http_put = self.module
            .declare_function("torq_http_put", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_http_put: {}", e)))?;
        let torq_http_delete = self.module
            .declare_function("torq_http_delete", Linkage::Import, &sig_ptr_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_http_delete: {}", e)))?;

        // Crypto functions
        let torq_crypto_hash = self.module
            .declare_function("torq_crypto_hash", Linkage::Import, &sig_pp_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_crypto_hash: {}", e)))?;
        let torq_crypto_uuid = self.module
            .declare_function("torq_crypto_uuid", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_crypto_uuid: {}", e)))?;

        // Logging functions
        let torq_log_info = self.module
            .declare_function("torq_log_info", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_log_info: {}", e)))?;
        let torq_log_warn = self.module
            .declare_function("torq_log_warn", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_log_warn: {}", e)))?;
        let torq_log_err = self.module
            .declare_function("torq_log_err", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_log_err: {}", e)))?;
        let torq_log_debug = self.module
            .declare_function("torq_log_debug", Linkage::Import, &sig_ptr_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_log_debug: {}", e)))?;

        // Math random
        let torq_math_random = self.module
            .declare_function("torq_math_random", Linkage::Import, &sig_void_to_ptr)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_math_random: {}", e)))?;

        // Assert functions
        let torq_assert = self.module
            .declare_function("torq_assert", Linkage::Import, &sig_pp_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_assert: {}", e)))?;
        let torq_assert_eq = self.module
            .declare_function("torq_assert_eq", Linkage::Import, &sig_pp_to_void)
            .map_err(|e| CodegenError::new(format!("failed to declare torq_assert_eq: {}", e)))?;

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
            torq_str_upper,
            torq_str_lower,
            torq_str_trim,
            torq_str_contains,
            torq_str_replace,
            torq_str_split,
            torq_str_starts,
            torq_str_ends,
            torq_str_slice,
            torq_str_reverse,
            torq_join,
            torq_to_string,
            torq_str_concat,
            torq_math_abs,
            torq_math_sqrt,
            torq_math_floor,
            torq_math_ceil,
            torq_math_round,
            torq_math_min,
            torq_math_max,
            torq_fs_read,
            torq_fs_write,
            torq_env,
            torq_log,
            torq_exit,
            torq_to_json,
            torq_from_json,
            torq_array_push,
            torq_array_pop,
            torq_array_shift,
            torq_array_at,
            torq_array_sort,
            torq_array_reverse,
            torq_array_sum,
            torq_array_unique,
            torq_array_flatten,
            torq_array_contains,
            torq_array_slice,
            torq_array_map_field,
            torq_array_filter_field,
            torq_array_empty,
            torq_dict_set,
            torq_dict_drop,
            torq_dict_merge,
            torq_dict_pick,
            torq_dict_omit,
            torq_dict_entries,
            torq_dict_empty,
            torq_dict_keys,
            torq_dict_values,
            torq_dict_has_tv,
            torq_dict_get_tv,
            torq_reverse,
            torq_parallel_each_array,
            torq_parallel_each_range,
            torq_pow,
            torq_sys_exec,
            torq_type_of,
            torq_time_now,
            torq_time_unix,
            torq_time_format,
            torq_time_parse,
            torq_time_diff,
            torq_time_add,
            torq_time_sleep,
            torq_http_get,
            torq_http_post,
            torq_http_put,
            torq_http_delete,
            torq_crypto_hash,
            torq_crypto_uuid,
            torq_log_info,
            torq_log_warn,
            torq_log_err,
            torq_log_debug,
            torq_math_random,
            torq_assert,
            torq_assert_eq,
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
            Statement::Expression(Expr::Call(call)) if call.name == "log" => {
                // Direct log call: `log "message"` or `log $var`
                if let Some(arg) = call.args.first() {
                    let val = self.compile_expr(arg, rt, builder, None)?;
                    let func_ref = self.module.declare_func_in_func(rt.torq_log, builder.func);
                    builder.ins().call(func_ref, &[val]);
                }
                Ok(None)
            }
            Statement::Expression(Expr::Call(call)) if call.name == "fs_write" && call.args.len() == 2 => {
                // Direct fs_write call: `fs_write "/path" $content`
                let path = self.compile_expr(&call.args[0], rt, builder, None)?;
                let content = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_fs_write, builder.func);
                builder.ins().call(func_ref, &[path, content]);
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
                    // === Parallel each ===
                    // Compile the body as a separate function and pass it to the runtime.

                    let is_range = matches!(&*each.iterable, Expr::Call(call) if call.name == "range" && call.args.len() == 2);

                    // 1. Declare & define the body function: fn(TorqValue*) -> TorqValue*
                    let body_name = format!("torq_each_body_{}", self.each_body_counter);
                    self.each_body_counter += 1;

                    let ptr_ty = self.module.target_config().pointer_type();
                    let mut body_sig = self.module.make_signature();
                    body_sig.params.push(AbiParam::new(ptr_ty)); // element
                    body_sig.returns.push(AbiParam::new(ptr_ty));

                    let body_func_id = self.module
                        .declare_function(&body_name, Linkage::Local, &body_sig)
                        .map_err(|e| CodegenError::new(format!("failed to declare parallel each body: {}", e)))?;

                    // Save outer variable scope
                    let saved_vars = self.variables.clone();
                    self.variables.clear();

                    // Build the body function
                    {
                        let mut body_ctx = self.module.make_context();
                        body_ctx.func.signature = body_sig;
                        let mut body_func_ctx = FunctionBuilderContext::new();
                        let mut body_builder = FunctionBuilder::new(&mut body_ctx.func, &mut body_func_ctx);

                        let body_entry = body_builder.create_block();
                        body_builder.append_block_params_for_function_params(body_entry);
                        body_builder.switch_to_block(body_entry);
                        body_builder.seal_block(body_entry);

                        // Bind the element parameter to the each binding variable
                        let elem_param = body_builder.block_params(body_entry)[0];
                        let cl_var = body_builder.declare_var(I64);
                        body_builder.def_var(cl_var, elem_param);
                        self.variables.insert(each.binding.name.clone(), cl_var);

                        // Compile body statements
                        let mut last_val: Option<Value> = None;
                        for stmt in &each.body {
                            let r = self.compile_statement(stmt, rt, &mut body_builder)?;
                            if r.is_some() {
                                last_val = r;
                            }
                        }

                        // Return last value or null
                        if !block_is_terminated(&body_builder) {
                            let ret = match last_val {
                                Some(v) => v,
                                None => {
                                    let null_fn = self.module.declare_func_in_func(rt.torq_null, body_builder.func);
                                    let inst = body_builder.ins().call(null_fn, &[]);
                                    body_builder.inst_results(inst)[0]
                                }
                            };
                            body_builder.ins().return_(&[ret]);
                        }

                        body_builder.finalize();
                        self.module
                            .define_function(body_func_id, &mut body_ctx)
                            .map_err(|e| CodegenError::new(format!("failed to define parallel each body: {}", e)))?;
                    }

                    // Restore outer scope
                    self.variables = saved_vars;

                    // 2. Get function pointer and call runtime
                    let body_func_ref = self.module.declare_func_in_func(body_func_id, builder.func);
                    let body_fn_ptr = builder.ins().func_addr(ptr_ty, body_func_ref);

                    if is_range {
                        if let Expr::Call(call) = &*each.iterable {
                            let start_ptr = self.compile_expr(&call.args[0], rt, builder, None)?;
                            let end_ptr = self.compile_expr(&call.args[1], rt, builder, None)?;
                            let as_int_fn = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                            let inst = builder.ins().call(as_int_fn, &[start_ptr]);
                            let start = builder.inst_results(inst)[0];
                            let as_int_fn2 = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                            let inst = builder.ins().call(as_int_fn2, &[end_ptr]);
                            let end = builder.inst_results(inst)[0];

                            let par_range_fn = self.module.declare_func_in_func(rt.torq_parallel_each_range, builder.func);
                            builder.ins().call(par_range_fn, &[start, end, body_fn_ptr]);
                        }
                    } else {
                        let arr = self.compile_expr(&each.iterable, rt, builder, None)?;
                        let par_arr_fn = self.module.declare_func_in_func(rt.torq_parallel_each_array, builder.func);
                        builder.ins().call(par_arr_fn, &[arr, body_fn_ptr]);
                    }

                    return Ok(None);
                }

                // Determine start/end values — range() or array iterable
                let is_range = matches!(&*each.iterable, Expr::Call(call) if call.name == "range" && call.args.len() == 2);

                // For array iteration: compile iterable, get length
                // For range: extract start/end as raw i64
                let (start_val, end_val, arr_val) = if is_range {
                    if let Expr::Call(call) = &*each.iterable {
                        let start_ptr = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let end_ptr = self.compile_expr(&call.args[1], rt, builder, None)?;
                        let as_int_fn = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                        let inst = builder.ins().call(as_int_fn, &[start_ptr]);
                        let start = builder.inst_results(inst)[0];
                        let as_int_fn2 = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                        let inst = builder.ins().call(as_int_fn2, &[end_ptr]);
                        let end = builder.inst_results(inst)[0];
                        (start, end, None)
                    } else {
                        unreachable!()
                    }
                } else {
                    // Array iterable: compile it, get its length
                    let arr = self.compile_expr(&each.iterable, rt, builder, None)?;
                    let len_fn = self.module.declare_func_in_func(rt.torq_len, builder.func);
                    let inst = builder.ins().call(len_fn, &[arr]);
                    let len_val = builder.inst_results(inst)[0];
                    let as_int_fn = self.module.declare_func_in_func(rt.torq_as_int, builder.func);
                    let inst = builder.ins().call(as_int_fn, &[len_val]);
                    let end = builder.inst_results(inst)[0];
                    let start = builder.ins().iconst(types::I64, 0);
                    (start, end, Some(arr))
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

                // Bind the loop variable
                let binding_val = if let Some(arr) = arr_val {
                    // Array iteration: get element at index
                    let int_fn = self.module.declare_func_in_func(rt.torq_int, builder.func);
                    let inst = builder.ins().call(int_fn, &[counter]);
                    let idx = builder.inst_results(inst)[0];
                    let get_fn = self.module.declare_func_in_func(rt.torq_array_get, builder.func);
                    let inst = builder.ins().call(get_fn, &[arr, idx]);
                    builder.inst_results(inst)[0]
                } else {
                    // Range iteration: box counter as TorqValue*
                    let int_fn = self.module.declare_func_in_func(rt.torq_int, builder.func);
                    let inst = builder.ins().call(int_fn, &[counter]);
                    builder.inst_results(inst)[0]
                };

                let cl_var = builder.declare_var(types::I64);
                builder.def_var(cl_var, binding_val);
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
                // String operations — no extra args (pipe value only)
                Expr::Call(call) if call.name == "upper" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_upper, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "lower" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_lower, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "trim" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_trim, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "reverse" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_reverse, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // String operations — one extra arg (pipe value + 1 arg)
                Expr::Call(call) if call.name == "contains" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_contains, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "split" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_split, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "starts" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_starts, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "ends" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_ends, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "join" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_join, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // String operations — two extra args (pipe value + 2 args)
                Expr::Call(call) if call.name == "replace" && call.args.len() == 2 => {
                    if let Some(val) = pipe_val {
                        let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_replace, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg0, arg1]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "slice" && call.args.len() == 2 => {
                    if let Some(val) = pipe_val {
                        let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_str_slice, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg0, arg1]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Math operations — no extra args (pipe value only)
                Expr::Call(call) if call.name == "abs" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_abs, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "sqrt" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_sqrt, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "floor" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_floor, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "ceil" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_ceil, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "round" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_round, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Math operations — one extra arg (pipe value + 1 arg)
                Expr::Call(call) if call.name == "min" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_min, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "max" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_math_max, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // I/O operations
                Expr::Call(call) if call.name == "fs_read" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_fs_read, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "fs_write" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let path = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_fs_write, builder.func);
                        builder.ins().call(func_ref, &[path, val]);
                        pipe_val = None;
                    }
                }
                Expr::Call(call) if call.name == "log" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_log, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                // JSON serialization
                Expr::Call(call) if call.name == "to_json" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_to_json, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "from_json" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_from_json, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Array operations — no extra args
                Expr::Call(call) if call.name == "sort" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_sort, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "sum" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_sum, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "unique" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_unique, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "flatten" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_flatten, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "pop" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_pop, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "shift" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_shift, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "empty?" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        // Works for both arrays and dicts
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_empty, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Array operations — one extra arg
                Expr::Call(call) if call.name == "push" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_push, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "at" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_array_at, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Dict operations — no extra args
                Expr::Call(call) if call.name == "keys" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_keys, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "values" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_values, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "entries" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_entries, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Dict operations — one extra arg
                Expr::Call(call) if call.name == "has" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_has_tv, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "get" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_get_tv, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "drop" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_drop, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "merge" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_merge, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "pick" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_pick, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "omit" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_omit, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Dict set — two extra args
                Expr::Call(call) if call.name == "set" && call.args.len() == 2 => {
                    if let Some(val) = pipe_val {
                        let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_dict_set, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg0, arg1]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Type checking
                Expr::Call(call) if call.name == "type_of" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_type_of, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Time operations — no args (produce value from nothing)
                Expr::Call(call) if call.name == "time_now" && call.args.is_empty() => {
                    let func_ref = self.module.declare_func_in_func(rt.torq_time_now, builder.func);
                    let inst = builder.ins().call(func_ref, &[]);
                    pipe_val = Some(builder.inst_results(inst)[0]);
                }
                Expr::Call(call) if call.name == "time_unix" && call.args.is_empty() => {
                    let func_ref = self.module.declare_func_in_func(rt.torq_time_unix, builder.func);
                    let inst = builder.ins().call(func_ref, &[]);
                    pipe_val = Some(builder.inst_results(inst)[0]);
                }
                // Time operations — one extra arg (pipe value + 1 arg)
                Expr::Call(call) if call.name == "time_format" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_time_format, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "time_parse" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_time_parse, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "time_diff" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_time_diff, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "time_add" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_time_add, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Time sleep — consumes pipe value, returns nothing
                Expr::Call(call) if call.name == "time_sleep" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_time_sleep, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                // HTTP operations
                Expr::Call(call) if call.name == "http_get" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_http_get, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "http_post" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_http_post, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "http_put" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_http_put, builder.func);
                        let inst = builder.ins().call(func_ref, &[val, arg]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "http_delete" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_http_delete, builder.func);
                        let inst = builder.ins().call(func_ref, &[val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                // Crypto operations
                Expr::Call(call) if call.name == "crypto_hash" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let algo = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_crypto_hash, builder.func);
                        let inst = builder.ins().call(func_ref, &[algo, val]);
                        pipe_val = Some(builder.inst_results(inst)[0]);
                    }
                }
                Expr::Call(call) if call.name == "crypto_uuid" && call.args.is_empty() => {
                    let func_ref = self.module.declare_func_in_func(rt.torq_crypto_uuid, builder.func);
                    let inst = builder.ins().call(func_ref, &[]);
                    pipe_val = Some(builder.inst_results(inst)[0]);
                }
                // Logging operations — consume pipe value
                Expr::Call(call) if call.name == "log_info" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_log_info, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                Expr::Call(call) if call.name == "log_warn" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_log_warn, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                Expr::Call(call) if call.name == "log_err" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_log_err, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                Expr::Call(call) if call.name == "log_debug" && call.args.is_empty() => {
                    if let Some(val) = pipe_val {
                        let func_ref = self.module.declare_func_in_func(rt.torq_log_debug, builder.func);
                        builder.ins().call(func_ref, &[val]);
                    }
                    pipe_val = None;
                }
                // Math random — no args, produces value
                Expr::Call(call) if call.name == "math_random" && call.args.is_empty() => {
                    let func_ref = self.module.declare_func_in_func(rt.torq_math_random, builder.func);
                    let inst = builder.ins().call(func_ref, &[]);
                    pipe_val = Some(builder.inst_results(inst)[0]);
                }
                // Assert operations — consume pipe value + 1 arg
                Expr::Call(call) if call.name == "assert" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_assert, builder.func);
                        builder.ins().call(func_ref, &[val, arg]);
                    }
                    pipe_val = None;
                }
                Expr::Call(call) if call.name == "assert_eq" && call.args.len() == 1 => {
                    if let Some(val) = pipe_val {
                        let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                        let func_ref = self.module.declare_func_in_func(rt.torq_assert_eq, builder.func);
                        builder.ins().call(func_ref, &[val, arg]);
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
                    BinOpKind::Pow => rt.torq_pow,
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
                        Pattern::Literal(lit) => {
                            let arm_block = builder.create_block();
                            let next_block = builder.create_block();

                            // Create pattern value as TorqValue* based on literal type
                            let pat_val = match lit {
                                Literal::Int(n, _) => {
                                    let pat_raw = builder.ins().iconst(types::I64, *n);
                                    let int_fn = self.module.declare_func_in_func(rt.torq_int, builder.func);
                                    let inst = builder.ins().call(int_fn, &[pat_raw]);
                                    builder.inst_results(inst)[0]
                                }
                                Literal::String(s, _) => {
                                    let pointer_type = self.module.target_config().pointer_type();
                                    let data_id = create_string_data(&mut self.module, &mut self.str_counter, s)?;
                                    let gv = self.module.declare_data_in_func(data_id, builder.func);
                                    let ptr = builder.ins().global_value(pointer_type, gv);
                                    let str_fn = self.module.declare_func_in_func(rt.torq_str, builder.func);
                                    let inst = builder.ins().call(str_fn, &[ptr]);
                                    builder.inst_results(inst)[0]
                                }
                                Literal::Bool(b, _) => {
                                    let val = builder.ins().iconst(I64, if *b { 1 } else { 0 });
                                    let bool_fn = self.module.declare_func_in_func(rt.torq_bool, builder.func);
                                    let inst = builder.ins().call(bool_fn, &[val]);
                                    builder.inst_results(inst)[0]
                                }
                                Literal::Float(f, _) => {
                                    let val = builder.ins().f64const(*f);
                                    let float_fn = self.module.declare_func_in_func(rt.torq_float, builder.func);
                                    let inst = builder.ins().call(float_fn, &[val]);
                                    builder.inst_results(inst)[0]
                                }
                                Literal::Null(_) => {
                                    let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                                    let inst = builder.ins().call(null_fn, &[]);
                                    builder.inst_results(inst)[0]
                                }
                                _ => return Err(CodegenError::new("unsupported literal in match pattern")),
                            };

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
                        Pattern::Comparison(op, rhs_expr) => {
                            let arm_block = builder.create_block();
                            let next_block = builder.create_block();

                            // Compile the RHS expression of the comparison
                            let rhs = self.compile_expr(rhs_expr, rt, builder, None)?;

                            // Select comparison function based on operator
                            let cmp_func_id = match op {
                                torqc_ast::ast::ComparisonOp::Gt => rt.torq_gt,
                                torqc_ast::ast::ComparisonOp::Lt => rt.torq_lt,
                                torqc_ast::ast::ComparisonOp::GtEq => rt.torq_gte,
                                torqc_ast::ast::ComparisonOp::LtEq => rt.torq_lte,
                                torqc_ast::ast::ComparisonOp::Eq => rt.torq_eq,
                                torqc_ast::ast::ComparisonOp::NotEq => rt.torq_neq,
                            };

                            let cmp_fn = self.module.declare_func_in_func(cmp_func_id, builder.func);
                            let inst = builder.ins().call(cmp_fn, &[subject, rhs]);
                            let cmp_result = builder.inst_results(inst)[0];

                            let truthy_fn = self.module.declare_func_in_func(rt.torq_is_truthy, builder.func);
                            let inst = builder.ins().call(truthy_fn, &[cmp_result]);
                            let cmp = builder.inst_results(inst)[0];

                            builder.ins().brif(cmp, arm_block, &[], next_block, &[]);

                            builder.switch_to_block(arm_block);
                            builder.seal_block(arm_block);
                            let result = self.compile_expr(&arm.body, rt, builder, None)?;
                            if !block_is_terminated(builder) {
                                builder.ins().jump(merge_block, &[BlockArg::Value(result)]);
                            }

                            builder.switch_to_block(next_block);
                            builder.seal_block(next_block);
                        }
                        Pattern::Variable(var) => {
                            // Bind the subject to the variable name, then execute body
                            let cl_var = builder.declare_var(I64);
                            builder.def_var(cl_var, subject);
                            self.variables.insert(var.name.clone(), cl_var);

                            let result = self.compile_expr(&arm.body, rt, builder, None)?;
                            if !block_is_terminated(builder) {
                                builder.ins().jump(merge_block, &[BlockArg::Value(result)]);
                            }
                        }
                        Pattern::Wildcard => {
                            let result = self.compile_expr(&arm.body, rt, builder, None)?;
                            if !block_is_terminated(builder) {
                                builder.ins().jump(merge_block, &[BlockArg::Value(result)]);
                            }
                        }
                        _ => {
                            return Err(CodegenError::new(format!(
                                "unsupported match pattern: {:?}",
                                arm.pattern
                            )));
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
            // I/O functions as expressions
            Expr::Call(call) if call.name == "fs_read" && call.args.len() == 1 => {
                let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_fs_read, builder.func);
                let inst = builder.ins().call(func_ref, &[arg]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "fs_write" && call.args.len() == 2 => {
                let path = self.compile_expr(&call.args[0], rt, builder, None)?;
                let content = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_fs_write, builder.func);
                builder.ins().call(func_ref, &[path, content]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "env" && call.args.len() == 1 => {
                let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_env, builder.func);
                let inst = builder.ins().call(func_ref, &[arg]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "log" => {
                let val = if let Some(arg) = call.args.first() {
                    self.compile_expr(arg, rt, builder, None)?
                } else if let Some(pv) = pipe_value {
                    pv
                } else {
                    let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                    let inst = builder.ins().call(null_fn, &[]);
                    builder.inst_results(inst)[0]
                };
                let func_ref = self.module.declare_func_in_func(rt.torq_log, builder.func);
                builder.ins().call(func_ref, &[val]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            // Time functions as expressions
            Expr::Call(call) if call.name == "time_now" && call.args.is_empty() => {
                let func_ref = self.module.declare_func_in_func(rt.torq_time_now, builder.func);
                let inst = builder.ins().call(func_ref, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "time_unix" && call.args.is_empty() => {
                let func_ref = self.module.declare_func_in_func(rt.torq_time_unix, builder.func);
                let inst = builder.ins().call(func_ref, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "time_format" && call.args.len() == 2 => {
                let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_time_format, builder.func);
                let inst = builder.ins().call(func_ref, &[arg0, arg1]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "time_parse" && call.args.len() == 2 => {
                let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_time_parse, builder.func);
                let inst = builder.ins().call(func_ref, &[arg0, arg1]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "time_diff" && call.args.len() == 2 => {
                let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_time_diff, builder.func);
                let inst = builder.ins().call(func_ref, &[arg0, arg1]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "time_add" && call.args.len() == 2 => {
                let arg0 = self.compile_expr(&call.args[0], rt, builder, None)?;
                let arg1 = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_time_add, builder.func);
                let inst = builder.ins().call(func_ref, &[arg0, arg1]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "time_sleep" && call.args.len() == 1 => {
                let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_time_sleep, builder.func);
                builder.ins().call(func_ref, &[arg]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            // HTTP functions as expressions
            Expr::Call(call) if call.name == "http_get" && call.args.len() == 1 => {
                let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_http_get, builder.func);
                let inst = builder.ins().call(func_ref, &[arg]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "http_post" && call.args.len() == 2 => {
                let url = self.compile_expr(&call.args[0], rt, builder, None)?;
                let body = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_http_post, builder.func);
                let inst = builder.ins().call(func_ref, &[url, body]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "http_put" && call.args.len() == 2 => {
                let url = self.compile_expr(&call.args[0], rt, builder, None)?;
                let body = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_http_put, builder.func);
                let inst = builder.ins().call(func_ref, &[url, body]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "http_delete" && call.args.len() == 1 => {
                let arg = self.compile_expr(&call.args[0], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_http_delete, builder.func);
                let inst = builder.ins().call(func_ref, &[arg]);
                Ok(builder.inst_results(inst)[0])
            }
            // Crypto functions as expressions
            Expr::Call(call) if call.name == "crypto_hash" && call.args.len() == 2 => {
                let data = self.compile_expr(&call.args[0], rt, builder, None)?;
                let algo = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_crypto_hash, builder.func);
                let inst = builder.ins().call(func_ref, &[data, algo]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "crypto_uuid" && call.args.is_empty() => {
                let func_ref = self.module.declare_func_in_func(rt.torq_crypto_uuid, builder.func);
                let inst = builder.ins().call(func_ref, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            // Logging functions as expressions
            Expr::Call(call) if call.name == "log_info" => {
                let val = if let Some(arg) = call.args.first() {
                    self.compile_expr(arg, rt, builder, None)?
                } else if let Some(pv) = pipe_value {
                    pv
                } else {
                    let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                    let inst = builder.ins().call(null_fn, &[]);
                    builder.inst_results(inst)[0]
                };
                let func_ref = self.module.declare_func_in_func(rt.torq_log_info, builder.func);
                builder.ins().call(func_ref, &[val]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "log_warn" => {
                let val = if let Some(arg) = call.args.first() {
                    self.compile_expr(arg, rt, builder, None)?
                } else if let Some(pv) = pipe_value {
                    pv
                } else {
                    let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                    let inst = builder.ins().call(null_fn, &[]);
                    builder.inst_results(inst)[0]
                };
                let func_ref = self.module.declare_func_in_func(rt.torq_log_warn, builder.func);
                builder.ins().call(func_ref, &[val]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "log_err" => {
                let val = if let Some(arg) = call.args.first() {
                    self.compile_expr(arg, rt, builder, None)?
                } else if let Some(pv) = pipe_value {
                    pv
                } else {
                    let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                    let inst = builder.ins().call(null_fn, &[]);
                    builder.inst_results(inst)[0]
                };
                let func_ref = self.module.declare_func_in_func(rt.torq_log_err, builder.func);
                builder.ins().call(func_ref, &[val]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "log_debug" => {
                let val = if let Some(arg) = call.args.first() {
                    self.compile_expr(arg, rt, builder, None)?
                } else if let Some(pv) = pipe_value {
                    pv
                } else {
                    let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                    let inst = builder.ins().call(null_fn, &[]);
                    builder.inst_results(inst)[0]
                };
                let func_ref = self.module.declare_func_in_func(rt.torq_log_debug, builder.func);
                builder.ins().call(func_ref, &[val]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            // Math random as expression
            Expr::Call(call) if call.name == "math_random" && call.args.is_empty() => {
                let func_ref = self.module.declare_func_in_func(rt.torq_math_random, builder.func);
                let inst = builder.ins().call(func_ref, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            // Assert functions as expressions
            Expr::Call(call) if call.name == "assert" && call.args.len() == 2 => {
                let val = self.compile_expr(&call.args[0], rt, builder, None)?;
                let msg = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_assert, builder.func);
                builder.ins().call(func_ref, &[val, msg]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::Call(call) if call.name == "assert_eq" && call.args.len() == 2 => {
                let expected = self.compile_expr(&call.args[0], rt, builder, None)?;
                let actual = self.compile_expr(&call.args[1], rt, builder, None)?;
                let func_ref = self.module.declare_func_in_func(rt.torq_assert_eq, builder.func);
                builder.ins().call(func_ref, &[expected, actual]);
                let null_fn = self.module.declare_func_in_func(rt.torq_null, builder.func);
                let inst = builder.ins().call(null_fn, &[]);
                Ok(builder.inst_results(inst)[0])
            }
            Expr::StringInterp(parts, _) => {
                let pointer_type = self.module.target_config().pointer_type();
                let mut result: Option<Value> = None;

                for part in parts {
                    let part_val = match part {
                        StringPart::Literal(s) => {
                            // Create string data and call torq_str
                            let data_id = create_string_data(&mut self.module, &mut self.str_counter, s)?;
                            let gv = self.module.declare_data_in_func(data_id, builder.func);
                            let ptr = builder.ins().global_value(pointer_type, gv);
                            let str_fn = self.module.declare_func_in_func(rt.torq_str, builder.func);
                            let inst = builder.ins().call(str_fn, &[ptr]);
                            builder.inst_results(inst)[0]
                        }
                        StringPart::Interpolation(var) => {
                            // Look up variable and convert to string
                            let cl_var = self.variables.get(&var.name)
                                .ok_or_else(|| CodegenError::new(format!("undefined variable: ${}", var.name)))?;
                            let var_val = builder.use_var(*cl_var);
                            let to_str_fn = self.module.declare_func_in_func(rt.torq_to_string, builder.func);
                            let inst = builder.ins().call(to_str_fn, &[var_val]);
                            builder.inst_results(inst)[0]
                        }
                    };

                    result = Some(match result {
                        None => part_val,
                        Some(prev) => {
                            let concat_fn = self.module.declare_func_in_func(rt.torq_str_concat, builder.func);
                            let inst = builder.ins().call(concat_fn, &[prev, part_val]);
                            builder.inst_results(inst)[0]
                        }
                    });
                }

                // If no parts, return empty string
                match result {
                    Some(val) => Ok(val),
                    None => {
                        let data_id = create_string_data(&mut self.module, &mut self.str_counter, "")?;
                        let gv = self.module.declare_data_in_func(data_id, builder.func);
                        let ptr = builder.ins().global_value(pointer_type, gv);
                        let str_fn = self.module.declare_func_in_func(rt.torq_str, builder.func);
                        let inst = builder.ins().call(str_fn, &[ptr]);
                        Ok(builder.inst_results(inst)[0])
                    }
                }
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
