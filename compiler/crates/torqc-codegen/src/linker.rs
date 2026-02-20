use std::path::{Path, PathBuf};
use std::process::Command;

use crate::codegen::CodegenError;
use crate::runtime::RUNTIME_C_SOURCE;

/// Detect the architecture of a Mach-O object file from its header bytes.
/// Returns the `-arch` flag value for Apple's linker (e.g., "arm64" or "x86_64").
#[cfg(target_os = "macos")]
fn detect_macho_arch(object_bytes: &[u8]) -> Option<&'static str> {
    if object_bytes.len() < 4 {
        return None;
    }
    let magic = u32::from_le_bytes([
        object_bytes[0],
        object_bytes[1],
        object_bytes[2],
        object_bytes[3],
    ]);
    match magic {
        // MH_MAGIC_64 (little-endian) — check cputype at offset 4
        0xFEED_FACF => {
            if object_bytes.len() < 8 {
                return None;
            }
            let cputype = u32::from_le_bytes([
                object_bytes[4],
                object_bytes[5],
                object_bytes[6],
                object_bytes[7],
            ]);
            match cputype {
                0x0100_000C => Some("arm64"),   // CPU_TYPE_ARM64
                0x0100_0007 => Some("x86_64"),  // CPU_TYPE_X86_64
                _ => None,
            }
        }
        _ => None,
    }
}

/// Link an object file into a native executable using the system C compiler.
/// Writes the TORQ runtime C source alongside the object file and compiles both.
pub fn link(object_bytes: &[u8], output_path: &Path) -> Result<PathBuf, CodegenError> {
    // 1. Write object bytes and runtime C source to unique temp files
    let temp_dir = std::env::temp_dir();
    let pid = std::process::id();
    let obj_path = temp_dir.join(format!("torq_output_{}.o", pid));
    let runtime_path = temp_dir.join(format!("torq_runtime_{}.c", pid));

    std::fs::write(&obj_path, object_bytes)
        .map_err(|e| CodegenError::new(format!("failed to write object file: {}", e)))?;
    std::fs::write(&runtime_path, RUNTIME_C_SOURCE)
        .map_err(|e| CodegenError::new(format!("failed to write runtime.c: {}", e)))?;

    // 2. Invoke cc to compile runtime and link with object file
    let mut cmd = Command::new("cc");
    cmd.arg(&obj_path)
        .arg(&runtime_path)
        .arg("-o")
        .arg(output_path);

    // On macOS, explicitly pass the architecture so the linker matches the
    // object file even when running under Rosetta (x86_64 emulation on arm64).
    #[cfg(target_os = "macos")]
    if let Some(arch) = detect_macho_arch(object_bytes) {
        cmd.arg("-arch").arg(arch);
    }

    let output = cmd
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_file(&obj_path);
            let _ = std::fs::remove_file(&runtime_path);
            CodegenError::new(format!("failed to invoke linker (cc): {}", e))
        })?;

    // 3. Clean up temp files (always, regardless of link success/failure)
    let _ = std::fs::remove_file(&obj_path);
    let _ = std::fs::remove_file(&runtime_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CodegenError::new(format!("linker failed: {}", stderr)));
    }

    Ok(output_path.to_path_buf())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Compiler;
    use torqc_ast::ast::*;

    #[test]
    fn link_produces_executable() {
        let compiler = Compiler::new().unwrap();
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![],
                doc_comments: vec![],
                span: Span {
                    file: "t".into(),
                    line: 1,
                    col: 1,
                },
            }],
        };
        let bytes = compiler.compile(&program).unwrap();

        let temp = std::env::temp_dir().join("torq_link_test");
        let result = link(&bytes, &temp);
        assert!(result.is_ok(), "link failed: {:?}", result.err());
        assert!(temp.exists());

        // Verify it runs (exit code 0)
        let output = std::process::Command::new(&temp).output().unwrap();
        assert!(output.status.success());

        let _ = std::fs::remove_file(&temp);
    }
}
