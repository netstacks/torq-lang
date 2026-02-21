#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>

// ===== Type system =====

typedef enum {
    TV_NULL = 0,
    TV_INT,
    TV_FLOAT,
    TV_BOOL,
    TV_STR,
    TV_ARRAY,
    TV_DICT
} TorqTypeTag;

typedef struct TorqValue TorqValue;
typedef struct TorqArray TorqArray;
typedef struct TorqDict TorqDict;

struct TorqArray {
    int64_t capacity;
    int64_t length;
    TorqValue** elements;
};

typedef struct {
    char* key;
    TorqValue* value;
} TorqDictEntry;

struct TorqDict {
    int64_t capacity;
    int64_t length;
    TorqDictEntry* entries;
};

struct TorqValue {
    TorqTypeTag type;
    union {
        int64_t integer;
        double floating;
        int64_t boolean;
        char* string;
        TorqArray* array;
        TorqDict* dict;
    };
};

// ===== Constructors =====

TorqValue* torq_int(int64_t n) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_INT;
    v->integer = n;
    return v;
}

TorqValue* torq_float(double f) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_FLOAT;
    v->floating = f;
    return v;
}

TorqValue* torq_bool(int64_t b) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_BOOL;
    v->boolean = b;
    return v;
}

TorqValue* torq_str(const char* s) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_STR;
    v->string = strdup(s);
    return v;
}

TorqValue* torq_null(void) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_NULL;
    v->integer = 0;
    return v;
}

// ===== Type checking =====

int64_t torq_is_truthy(TorqValue* v) {
    if (!v) return 0;
    switch (v->type) {
        case TV_NULL: return 0;
        case TV_INT: return v->integer != 0;
        case TV_FLOAT: return v->floating != 0.0;
        case TV_BOOL: return v->boolean;
        case TV_STR: return v->string && v->string[0] != '\0';
        case TV_ARRAY: return v->array && v->array->length > 0;
        case TV_DICT: return v->dict && v->dict->length > 0;
    }
    return 0;
}

// ===== Extraction =====

int64_t torq_as_int(TorqValue* v) {
    if (!v) return 0;
    switch (v->type) {
        case TV_INT: return v->integer;
        case TV_FLOAT: return (int64_t)v->floating;
        case TV_BOOL: return v->boolean;
        default: return 0;
    }
}

// ===== Print =====

// Forward declarations for collection printing (implemented in Tasks 4 and 5)
static void torq_print_array_value(TorqValue* v);
static void torq_print_dict_value(TorqValue* v);
static void torq_fprint_value(FILE* f, TorqValue* v);

void torq_print(TorqValue* v) {
    if (!v) { puts("null"); return; }
    switch (v->type) {
        case TV_NULL:  puts("null"); break;
        case TV_INT:   printf("%lld\n", (long long)v->integer); break;
        case TV_FLOAT: printf("%g\n", v->floating); break;
        case TV_BOOL:  puts(v->boolean ? "true" : "false"); break;
        case TV_STR:   puts(v->string); break;
        case TV_ARRAY: torq_print_array_value(v); putchar('\n'); break;
        case TV_DICT:  torq_print_dict_value(v); putchar('\n'); break;
    }
}

// Stub implementations for collection printing (will be replaced in Tasks 4 and 5)
static void torq_print_array_value(TorqValue* v) {
    (void)v;
    printf("[array]");
}

static void torq_print_dict_value(TorqValue* v) {
    (void)v;
    printf("{dict}");
}

static void torq_fprint_value(FILE* f, TorqValue* v) {
    if (!v) { fprintf(f, "null"); return; }
    switch (v->type) {
        case TV_NULL:  fprintf(f, "null"); break;
        case TV_INT:   fprintf(f, "%lld", (long long)v->integer); break;
        case TV_FLOAT: fprintf(f, "%g", v->floating); break;
        case TV_BOOL:  fprintf(f, "%s", v->boolean ? "true" : "false"); break;
        case TV_STR:   fprintf(f, "%s", v->string); break;
        case TV_ARRAY: fprintf(f, "[array]"); break;
        case TV_DICT:  fprintf(f, "{dict}"); break;
    }
}

// ===== Arithmetic =====

TorqValue* torq_add(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_int(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_int(a->integer + b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_float(fa + fb);
    }
    if (a->type == TV_STR && b->type == TV_STR) {
        size_t la = strlen(a->string), lb = strlen(b->string);
        char* s = (char*)malloc(la + lb + 1);
        memcpy(s, a->string, la);
        memcpy(s + la, b->string, lb + 1);
        TorqValue* v = torq_str(s);
        free(s);
        return v;
    }
    return torq_int(0);
}

TorqValue* torq_sub(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_int(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_int(a->integer - b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_float(fa - fb);
    }
    return torq_int(0);
}

TorqValue* torq_mul(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_int(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_int(a->integer * b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_float(fa * fb);
    }
    return torq_int(0);
}

TorqValue* torq_div(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_int(0);
    if (a->type == TV_INT && b->type == TV_INT) {
        // Division by zero returns zero (TORQ language convention)
        if (b->integer == 0) return torq_int(0);
        return torq_int(a->integer / b->integer);
    }
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        if (fb == 0.0) return torq_float(0.0);
        return torq_float(fa / fb);
    }
    return torq_int(0);
}

TorqValue* torq_mod(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_int(0);
    if (a->type == TV_INT && b->type == TV_INT) {
        if (b->integer == 0) return torq_int(0);
        return torq_int(a->integer % b->integer);
    }
    return torq_int(0);
}

// ===== Comparison =====

TorqValue* torq_eq(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_bool(a == b);
    // Bool-int coercion: allow comparing bool and int values
    if ((a->type == TV_BOOL && b->type == TV_INT) ||
        (a->type == TV_INT && b->type == TV_BOOL)) {
        int64_t va = (a->type == TV_BOOL) ? a->boolean : a->integer;
        int64_t vb = (b->type == TV_BOOL) ? b->boolean : b->integer;
        return torq_bool(va == vb);
    }
    if (a->type != b->type) return torq_bool(0);
    switch (a->type) {
        case TV_INT:   return torq_bool(a->integer == b->integer);
        case TV_FLOAT: return torq_bool(a->floating == b->floating); // exact comparison
        case TV_BOOL:  return torq_bool(a->boolean == b->boolean);
        case TV_STR:   return torq_bool(strcmp(a->string, b->string) == 0);
        case TV_NULL:  return torq_bool(1);
        case TV_ARRAY: // stub: array equality not yet implemented
        case TV_DICT:  // stub: dict equality not yet implemented
        default: return torq_bool(0);
    }
}

TorqValue* torq_neq(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_bool(a != b);
    if ((a->type == TV_BOOL && b->type == TV_INT) ||
        (a->type == TV_INT && b->type == TV_BOOL)) {
        int64_t va = (a->type == TV_BOOL) ? a->boolean : a->integer;
        int64_t vb = (b->type == TV_BOOL) ? b->boolean : b->integer;
        return torq_bool(va != vb);
    }
    if (a->type != b->type) return torq_bool(1);
    switch (a->type) {
        case TV_INT:   return torq_bool(a->integer != b->integer);
        case TV_FLOAT: return torq_bool(a->floating != b->floating);
        case TV_BOOL:  return torq_bool(a->boolean != b->boolean);
        case TV_STR:   return torq_bool(strcmp(a->string, b->string) != 0);
        case TV_NULL:  return torq_bool(0);
        default:       return torq_bool(1);
    }
}

TorqValue* torq_gt(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_bool(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_bool(a->integer > b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_bool(fa > fb);
    }
    return torq_bool(0);
}

TorqValue* torq_lt(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_bool(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_bool(a->integer < b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_bool(fa < fb);
    }
    return torq_bool(0);
}

TorqValue* torq_gte(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_bool(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_bool(a->integer >= b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_bool(fa >= fb);
    }
    return torq_bool(0);
}

TorqValue* torq_lte(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_bool(0);
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_bool(a->integer <= b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_bool(fa <= fb);
    }
    return torq_bool(0);
}

