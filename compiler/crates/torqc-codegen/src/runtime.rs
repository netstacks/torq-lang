pub const RUNTIME_C_SOURCE: &str = r#"
#include <stdio.h>
#include <stdint.h>

void torq_print_int(int64_t n) {
    printf("%lld\n", (long long)n);
}

void torq_print_str(const char* s) {
    puts(s);
}

void torq_print_bool(int64_t b) {
    puts(b ? "true" : "false");
}

void torq_print_float(double f) {
    printf("%g\n", f);
}

void torq_print_null(void) {
    puts("null");
}
"#;
