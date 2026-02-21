#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <ctype.h>

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

// Array printing — proper implementation
static void torq_print_array_value(TorqValue* v) {
    if (!v || v->type != TV_ARRAY) { printf("[array]"); return; }
    TorqArray* a = v->array;
    putchar('[');
    for (int64_t i = 0; i < a->length; i++) {
        if (i > 0) printf(", ");
        TorqValue* elem = a->elements[i];
        if (elem && elem->type == TV_STR) {
            // Strings are quoted in array display
            printf("\"%s\"", elem->string);
        } else {
            torq_fprint_value(stdout, elem);
        }
    }
    putchar(']');
}

static void torq_print_dict_value(TorqValue* v) {
    if (!v || v->type != TV_DICT) { printf("{dict}"); return; }
    TorqDict* d = v->dict;
    putchar('{');
    for (int64_t i = 0; i < d->length; i++) {
        if (i > 0) printf(", ");
        printf("%s: ", d->entries[i].key);
        TorqValue* val = d->entries[i].value;
        if (val && val->type == TV_STR) {
            printf("\"%s\"", val->string);
        } else {
            torq_fprint_value(stdout, val);
        }
    }
    putchar('}');
}

static void torq_fprint_value(FILE* f, TorqValue* v) {
    if (!v) { fprintf(f, "null"); return; }
    switch (v->type) {
        case TV_NULL:  fprintf(f, "null"); break;
        case TV_INT:   fprintf(f, "%lld", (long long)v->integer); break;
        case TV_FLOAT: fprintf(f, "%g", v->floating); break;
        case TV_BOOL:  fprintf(f, "%s", v->boolean ? "true" : "false"); break;
        case TV_STR:   fprintf(f, "%s", v->string); break;
        case TV_ARRAY: torq_print_array_value(v); break;
        case TV_DICT: {
            TorqDict* d = v->dict;
            fprintf(f, "{");
            for (int64_t i = 0; i < d->length; i++) {
                if (i > 0) fprintf(f, ", ");
                fprintf(f, "%s: ", d->entries[i].key);
                TorqValue* val = d->entries[i].value;
                if (val && val->type == TV_STR) fprintf(f, "\"%s\"", val->string);
                else torq_fprint_value(f, val);
            }
            fprintf(f, "}");
            break;
        }
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

// ===== Array =====

TorqValue* torq_array_new(void) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_ARRAY;
    v->array = (TorqArray*)malloc(sizeof(TorqArray));
    v->array->capacity = 8;
    v->array->length = 0;
    v->array->elements = (TorqValue**)calloc(8, sizeof(TorqValue*));
    return v;
}

void torq_array_push_mut(TorqValue* arr, TorqValue* val) {
    if (!arr || arr->type != TV_ARRAY) return;
    TorqArray* a = arr->array;
    if (a->length >= a->capacity) {
        a->capacity *= 2;
        a->elements = (TorqValue**)realloc(a->elements, a->capacity * sizeof(TorqValue*));
    }
    a->elements[a->length++] = val;
}

TorqValue* torq_array_len(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return torq_int(0);
    return torq_int(arr->array->length);
}

TorqValue* torq_array_first(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY || arr->array->length == 0) return torq_null();
    return arr->array->elements[0];
}

TorqValue* torq_array_last(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY || arr->array->length == 0) return torq_null();
    return arr->array->elements[arr->array->length - 1];
}

TorqValue* torq_array_get(TorqValue* arr, TorqValue* index) {
    if (!arr || arr->type != TV_ARRAY) return torq_null();
    int64_t i = torq_as_int(index);
    if (i < 0 || i >= arr->array->length) return torq_null();
    return arr->array->elements[i];
}

// ===== Dict =====

TorqValue* torq_dict_new(void) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_DICT;
    v->dict = (TorqDict*)malloc(sizeof(TorqDict));
    v->dict->capacity = 16;
    v->dict->length = 0;
    v->dict->entries = (TorqDictEntry*)calloc(16, sizeof(TorqDictEntry));
    return v;
}

void torq_dict_set_mut(TorqValue* d, const char* key, TorqValue* val) {
    if (!d || d->type != TV_DICT) return;
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        if (strcmp(dict->entries[i].key, key) == 0) {
            dict->entries[i].value = val;
            return;
        }
    }
    if (dict->length >= dict->capacity) {
        dict->capacity *= 2;
        dict->entries = (TorqDictEntry*)realloc(dict->entries, dict->capacity * sizeof(TorqDictEntry));
    }
    dict->entries[dict->length].key = strdup(key);
    dict->entries[dict->length].value = val;
    dict->length++;
}

TorqValue* torq_dict_get(TorqValue* d, const char* key) {
    if (!d || d->type != TV_DICT) return torq_null();
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        if (strcmp(dict->entries[i].key, key) == 0) {
            return dict->entries[i].value;
        }
    }
    return torq_null();
}

TorqValue* torq_dict_len(TorqValue* d) {
    if (!d || d->type != TV_DICT) return torq_int(0);
    return torq_int(d->dict->length);
}

TorqValue* torq_dict_has(TorqValue* d, const char* key) {
    if (!d || d->type != TV_DICT) return torq_bool(0);
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        if (strcmp(dict->entries[i].key, key) == 0) return torq_bool(1);
    }
    return torq_bool(0);
}

TorqValue* torq_dict_keys(TorqValue* d) {
    TorqValue* arr = torq_array_new();
    if (!d || d->type != TV_DICT) return arr;
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        torq_array_push_mut(arr, torq_str(dict->entries[i].key));
    }
    return arr;
}

TorqValue* torq_dict_values(TorqValue* d) {
    TorqValue* arr = torq_array_new();
    if (!d || d->type != TV_DICT) return arr;
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        torq_array_push_mut(arr, dict->entries[i].value);
    }
    return arr;
}

// ===== Unified len =====

TorqValue* torq_len(TorqValue* v) {
    if (!v) return torq_int(0);
    switch (v->type) {
        case TV_ARRAY: return torq_int(v->array->length);
        case TV_DICT: return torq_int(v->dict->length);
        case TV_STR: return torq_int((int64_t)strlen(v->string));
        default: return torq_int(0);
    }
}

// ===== String operations =====

TorqValue* torq_str_upper(TorqValue* v) {
    if (!v || v->type != TV_STR) return v ? v : torq_str("");
    char* s = strdup(v->string);
    for (char* p = s; *p; p++) *p = toupper((unsigned char)*p);
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_str_lower(TorqValue* v) {
    if (!v || v->type != TV_STR) return v ? v : torq_str("");
    char* s = strdup(v->string);
    for (char* p = s; *p; p++) *p = tolower((unsigned char)*p);
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_str_trim(TorqValue* v) {
    if (!v || v->type != TV_STR) return v ? v : torq_str("");
    const char* start = v->string;
    while (*start && isspace((unsigned char)*start)) start++;
    const char* end = v->string + strlen(v->string) - 1;
    while (end > start && isspace((unsigned char)*end)) end--;
    size_t len = end - start + 1;
    char* s = (char*)malloc(len + 1);
    memcpy(s, start, len);
    s[len] = '\0';
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_str_contains(TorqValue* v, TorqValue* substr) {
    if (!v || v->type != TV_STR || !substr || substr->type != TV_STR) return torq_bool(0);
    return torq_bool(strstr(v->string, substr->string) != NULL);
}

TorqValue* torq_str_replace(TorqValue* v, TorqValue* old, TorqValue* new_) {
    if (!v || v->type != TV_STR || !old || old->type != TV_STR || !new_ || new_->type != TV_STR)
        return v ? v : torq_str("");
    char* pos = strstr(v->string, old->string);
    if (!pos) return torq_str(v->string);
    size_t prefix_len = pos - v->string;
    size_t old_len = strlen(old->string);
    size_t new_len = strlen(new_->string);
    size_t tail_len = strlen(pos + old_len);
    char* s = (char*)malloc(prefix_len + new_len + tail_len + 1);
    memcpy(s, v->string, prefix_len);
    memcpy(s + prefix_len, new_->string, new_len);
    memcpy(s + prefix_len + new_len, pos + old_len, tail_len + 1);
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_str_split(TorqValue* v, TorqValue* delim) {
    TorqValue* arr = torq_array_new();
    if (!v || v->type != TV_STR || !delim || delim->type != TV_STR) return arr;
    char* input = strdup(v->string);
    size_t delim_len = strlen(delim->string);
    if (delim_len == 0) {
        // Split into individual characters
        for (char* p = input; *p; p++) {
            char ch[2] = { *p, '\0' };
            torq_array_push_mut(arr, torq_str(ch));
        }
        free(input);
        return arr;
    }
    char* cur = input;
    char* found;
    while ((found = strstr(cur, delim->string)) != NULL) {
        *found = '\0';
        torq_array_push_mut(arr, torq_str(cur));
        cur = found + delim_len;
    }
    torq_array_push_mut(arr, torq_str(cur));
    free(input);
    return arr;
}

TorqValue* torq_str_starts(TorqValue* v, TorqValue* prefix) {
    if (!v || v->type != TV_STR || !prefix || prefix->type != TV_STR) return torq_bool(0);
    size_t plen = strlen(prefix->string);
    return torq_bool(strncmp(v->string, prefix->string, plen) == 0);
}

TorqValue* torq_str_ends(TorqValue* v, TorqValue* suffix) {
    if (!v || v->type != TV_STR || !suffix || suffix->type != TV_STR) return torq_bool(0);
    size_t slen = strlen(v->string);
    size_t xlen = strlen(suffix->string);
    if (xlen > slen) return torq_bool(0);
    return torq_bool(strcmp(v->string + slen - xlen, suffix->string) == 0);
}

TorqValue* torq_str_slice(TorqValue* v, TorqValue* start, TorqValue* end_) {
    if (!v || v->type != TV_STR) return torq_str("");
    int64_t s = torq_as_int(start);
    int64_t e = torq_as_int(end_);
    int64_t len = (int64_t)strlen(v->string);
    if (s < 0) s = 0;
    if (e > len) e = len;
    if (s >= e) return torq_str("");
    size_t slice_len = e - s;
    char* buf = (char*)malloc(slice_len + 1);
    memcpy(buf, v->string + s, slice_len);
    buf[slice_len] = '\0';
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

TorqValue* torq_str_reverse(TorqValue* v) {
    if (!v || v->type != TV_STR) return torq_str("");
    size_t len = strlen(v->string);
    char* s = (char*)malloc(len + 1);
    for (size_t i = 0; i < len; i++) s[i] = v->string[len - 1 - i];
    s[len] = '\0';
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_to_string(TorqValue* v) {
    if (!v) return torq_str("null");
    switch (v->type) {
        case TV_STR: return v;  // Already a string
        case TV_INT: {
            char buf[32];
            snprintf(buf, sizeof(buf), "%lld", (long long)v->integer);
            return torq_str(buf);
        }
        case TV_FLOAT: {
            char buf[64];
            snprintf(buf, sizeof(buf), "%g", v->floating);
            return torq_str(buf);
        }
        case TV_BOOL: return torq_str(v->boolean ? "true" : "false");
        case TV_NULL: return torq_str("null");
        default: return torq_str("");
    }
}

TorqValue* torq_str_concat(TorqValue* a, TorqValue* b) {
    TorqValue* sa = torq_to_string(a);
    TorqValue* sb = torq_to_string(b);
    size_t la = strlen(sa->string);
    size_t lb = strlen(sb->string);
    char* s = (char*)malloc(la + lb + 1);
    memcpy(s, sa->string, la);
    memcpy(s + la, sb->string, lb + 1);
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_join(TorqValue* arr, TorqValue* delim) {
    if (!arr || arr->type != TV_ARRAY || arr->array->length == 0) return torq_str("");
    const char* d = (delim && delim->type == TV_STR) ? delim->string : "";
    size_t dlen = strlen(d);
    // Calculate total length
    size_t total = 0;
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* elem = arr->array->elements[i];
        if (elem && elem->type == TV_STR) total += strlen(elem->string);
        if (i > 0) total += dlen;
    }
    char* buf = (char*)malloc(total + 1);
    char* pos = buf;
    for (int64_t i = 0; i < arr->array->length; i++) {
        if (i > 0) { memcpy(pos, d, dlen); pos += dlen; }
        TorqValue* elem = arr->array->elements[i];
        if (elem && elem->type == TV_STR) {
            size_t elen = strlen(elem->string);
            memcpy(pos, elem->string, elen);
            pos += elen;
        }
    }
    *pos = '\0';
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

