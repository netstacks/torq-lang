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

// ===== Math =====

TorqValue* torq_math_abs(TorqValue* v) {
    if (!v) return torq_int(0);
    if (v->type == TV_INT) return torq_int(llabs(v->integer));
    if (v->type == TV_FLOAT) return torq_float(fabs(v->floating));
    return v;
}

TorqValue* torq_math_sqrt(TorqValue* v) {
    if (!v) return torq_float(0.0);
    double n = (v->type == TV_FLOAT) ? v->floating : (double)torq_as_int(v);
    return torq_float(sqrt(n));
}

TorqValue* torq_math_floor(TorqValue* v) {
    if (!v) return torq_int(0);
    if (v->type == TV_INT) return v;
    if (v->type == TV_FLOAT) return torq_int((int64_t)floor(v->floating));
    return torq_int(0);
}

TorqValue* torq_math_ceil(TorqValue* v) {
    if (!v) return torq_int(0);
    if (v->type == TV_INT) return v;
    if (v->type == TV_FLOAT) return torq_int((int64_t)ceil(v->floating));
    return torq_int(0);
}

TorqValue* torq_math_round(TorqValue* v) {
    if (!v) return torq_int(0);
    if (v->type == TV_INT) return v;
    if (v->type == TV_FLOAT) return torq_int((int64_t)round(v->floating));
    return torq_int(0);
}

TorqValue* torq_math_min(TorqValue* a, TorqValue* b) {
    if (!a || !b) return a ? a : (b ? b : torq_int(0));
    double va = (a->type == TV_FLOAT) ? a->floating : (double)torq_as_int(a);
    double vb = (b->type == TV_FLOAT) ? b->floating : (double)torq_as_int(b);
    return (va <= vb) ? a : b;
}

TorqValue* torq_math_max(TorqValue* a, TorqValue* b) {
    if (!a || !b) return a ? a : (b ? b : torq_int(0));
    double va = (a->type == TV_FLOAT) ? a->floating : (double)torq_as_int(a);
    double vb = (b->type == TV_FLOAT) ? b->floating : (double)torq_as_int(b);
    return (va >= vb) ? a : b;
}

// ===== I/O =====

TorqValue* torq_fs_read(TorqValue* path) {
    if (!path || path->type != TV_STR) return torq_null();
    FILE* f = fopen(path->string, "r");
    if (!f) return torq_null();
    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc(len + 1);
    fread(buf, 1, len, f);
    buf[len] = '\0';
    fclose(f);
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

void torq_fs_write(TorqValue* path, TorqValue* content) {
    if (!path || path->type != TV_STR || !content || content->type != TV_STR) return;
    FILE* f = fopen(path->string, "w");
    if (f) { fputs(content->string, f); fclose(f); }
}

TorqValue* torq_env(TorqValue* name) {
    if (!name || name->type != TV_STR) return torq_null();
    const char* val = getenv(name->string);
    return val ? torq_str(val) : torq_null();
}

void torq_log(TorqValue* v) {
    if (!v) { fprintf(stderr, "null\n"); return; }
    switch (v->type) {
        case TV_NULL:  fprintf(stderr, "null\n"); break;
        case TV_INT:   fprintf(stderr, "%lld\n", (long long)v->integer); break;
        case TV_FLOAT: fprintf(stderr, "%g\n", v->floating); break;
        case TV_BOOL:  fprintf(stderr, "%s\n", v->boolean ? "true" : "false"); break;
        case TV_STR:   fprintf(stderr, "%s\n", v->string); break;
        default:       fprintf(stderr, "[value]\n"); break;
    }
}

void torq_exit(TorqValue* code) {
    exit((int)torq_as_int(code));
}

// ===== JSON =====

// Helper: append a string to a dynamically growing buffer
static void json_buf_append(char** buf, size_t* len, size_t* cap, const char* s) {
    size_t slen = strlen(s);
    while (*len + slen >= *cap) {
        *cap *= 2;
        *buf = (char*)realloc(*buf, *cap);
    }
    memcpy(*buf + *len, s, slen);
    *len += slen;
    (*buf)[*len] = '\0';
}

static void json_serialize(TorqValue* v, char** buf, size_t* len, size_t* cap) {
    if (!v || v->type == TV_NULL) {
        json_buf_append(buf, len, cap, "null");
        return;
    }
    switch (v->type) {
        case TV_INT: {
            char tmp[32];
            snprintf(tmp, sizeof(tmp), "%lld", (long long)v->integer);
            json_buf_append(buf, len, cap, tmp);
            break;
        }
        case TV_FLOAT: {
            char tmp[64];
            snprintf(tmp, sizeof(tmp), "%g", v->floating);
            json_buf_append(buf, len, cap, tmp);
            break;
        }
        case TV_BOOL:
            json_buf_append(buf, len, cap, v->boolean ? "true" : "false");
            break;
        case TV_STR: {
            json_buf_append(buf, len, cap, "\"");
            // Escape special characters
            for (const char* p = v->string; *p; p++) {
                switch (*p) {
                    case '"':  json_buf_append(buf, len, cap, "\\\""); break;
                    case '\\': json_buf_append(buf, len, cap, "\\\\"); break;
                    case '\n': json_buf_append(buf, len, cap, "\\n"); break;
                    case '\t': json_buf_append(buf, len, cap, "\\t"); break;
                    case '\r': json_buf_append(buf, len, cap, "\\r"); break;
                    default: {
                        char ch[2] = { *p, '\0' };
                        json_buf_append(buf, len, cap, ch);
                    }
                }
            }
            json_buf_append(buf, len, cap, "\"");
            break;
        }
        case TV_ARRAY: {
            json_buf_append(buf, len, cap, "[");
            TorqArray* a = v->array;
            for (int64_t i = 0; i < a->length; i++) {
                if (i > 0) json_buf_append(buf, len, cap, ", ");
                json_serialize(a->elements[i], buf, len, cap);
            }
            json_buf_append(buf, len, cap, "]");
            break;
        }
        case TV_DICT: {
            json_buf_append(buf, len, cap, "{");
            TorqDict* d = v->dict;
            for (int64_t i = 0; i < d->length; i++) {
                if (i > 0) json_buf_append(buf, len, cap, ", ");
                // Key is always a string
                json_buf_append(buf, len, cap, "\"");
                json_buf_append(buf, len, cap, d->entries[i].key);
                json_buf_append(buf, len, cap, "\": ");
                json_serialize(d->entries[i].value, buf, len, cap);
            }
            json_buf_append(buf, len, cap, "}");
            break;
        }
        default:
            json_buf_append(buf, len, cap, "null");
            break;
    }
}

TorqValue* torq_to_json(TorqValue* v) {
    size_t cap = 256;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    buf[0] = '\0';
    json_serialize(v, &buf, &len, &cap);
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

// ===== Advanced Array Operations =====

TorqValue* torq_array_push(TorqValue* arr, TorqValue* val) {
    if (!arr || arr->type != TV_ARRAY) return arr ? arr : torq_array_new();
    // Create a new array with the element appended (non-mutating)
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length; i++) {
        torq_array_push_mut(result, arr->array->elements[i]);
    }
    torq_array_push_mut(result, val);
    return result;
}

TorqValue* torq_array_pop(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY || arr->array->length == 0) return arr ? arr : torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length - 1; i++) {
        torq_array_push_mut(result, arr->array->elements[i]);
    }
    return result;
}

TorqValue* torq_array_shift(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY || arr->array->length == 0) return arr ? arr : torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 1; i < arr->array->length; i++) {
        torq_array_push_mut(result, arr->array->elements[i]);
    }
    return result;
}

TorqValue* torq_array_at(TorqValue* arr, TorqValue* index) {
    return torq_array_get(arr, index);
}

static int torq_compare_values(const void* a, const void* b) {
    TorqValue* va = *(TorqValue**)a;
    TorqValue* vb = *(TorqValue**)b;
    if (!va || !vb) return 0;
    if (va->type == TV_INT && vb->type == TV_INT) {
        if (va->integer < vb->integer) return -1;
        if (va->integer > vb->integer) return 1;
        return 0;
    }
    if (va->type == TV_FLOAT && vb->type == TV_FLOAT) {
        if (va->floating < vb->floating) return -1;
        if (va->floating > vb->floating) return 1;
        return 0;
    }
    if (va->type == TV_STR && vb->type == TV_STR) {
        return strcmp(va->string, vb->string);
    }
    return 0;
}

TorqValue* torq_array_sort(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return arr ? arr : torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length; i++) {
        torq_array_push_mut(result, arr->array->elements[i]);
    }
    qsort(result->array->elements, result->array->length, sizeof(TorqValue*), torq_compare_values);
    return result;
}

TorqValue* torq_array_reverse(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return arr ? arr : torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = arr->array->length - 1; i >= 0; i--) {
        torq_array_push_mut(result, arr->array->elements[i]);
    }
    return result;
}

TorqValue* torq_array_sum(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return torq_int(0);
    int64_t isum = 0;
    double fsum = 0.0;
    int has_float = 0;
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* elem = arr->array->elements[i];
        if (!elem) continue;
        if (elem->type == TV_FLOAT) { has_float = 1; fsum += elem->floating; }
        else if (elem->type == TV_INT) { isum += elem->integer; fsum += (double)elem->integer; }
    }
    if (has_float) return torq_float(fsum);
    return torq_int(isum);
}

TorqValue* torq_array_unique(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return arr ? arr : torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* elem = arr->array->elements[i];
        int found = 0;
        for (int64_t j = 0; j < result->array->length; j++) {
            TorqValue* eq = torq_eq(elem, result->array->elements[j]);
            if (eq && eq->type == TV_BOOL && eq->boolean) { found = 1; break; }
        }
        if (!found) torq_array_push_mut(result, elem);
    }
    return result;
}

TorqValue* torq_array_flatten(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return arr ? arr : torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* elem = arr->array->elements[i];
        if (elem && elem->type == TV_ARRAY) {
            for (int64_t j = 0; j < elem->array->length; j++) {
                torq_array_push_mut(result, elem->array->elements[j]);
            }
        } else {
            torq_array_push_mut(result, elem);
        }
    }
    return result;
}

TorqValue* torq_array_contains(TorqValue* arr, TorqValue* val) {
    if (!arr || arr->type != TV_ARRAY) return torq_bool(0);
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* eq = torq_eq(arr->array->elements[i], val);
        if (eq && eq->type == TV_BOOL && eq->boolean) return torq_bool(1);
    }
    return torq_bool(0);
}

TorqValue* torq_array_index_of(TorqValue* arr, TorqValue* val) {
    if (!arr || arr->type != TV_ARRAY) return torq_int(-1);
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* eq = torq_eq(arr->array->elements[i], val);
        if (eq && eq->type == TV_BOOL && eq->boolean) return torq_int(i);
    }
    return torq_int(-1);
}

TorqValue* torq_array_slice(TorqValue* arr, TorqValue* start, TorqValue* end_) {
    if (!arr || arr->type != TV_ARRAY) return torq_array_new();
    int64_t s = torq_as_int(start);
    int64_t e = torq_as_int(end_);
    int64_t len = arr->array->length;
    if (s < 0) s = 0;
    if (e > len) e = len;
    if (s >= e) return torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = s; i < e; i++) {
        torq_array_push_mut(result, arr->array->elements[i]);
    }
    return result;
}

TorqValue* torq_array_map_field(TorqValue* arr, TorqValue* field) {
    if (!arr || arr->type != TV_ARRAY || !field || field->type != TV_STR) return torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* elem = arr->array->elements[i];
        if (elem && elem->type == TV_DICT) {
            torq_array_push_mut(result, torq_dict_get(elem, field->string));
        } else {
            torq_array_push_mut(result, torq_null());
        }
    }
    return result;
}

TorqValue* torq_array_filter_field(TorqValue* arr, TorqValue* field) {
    if (!arr || arr->type != TV_ARRAY || !field || field->type != TV_STR) return torq_array_new();
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < arr->array->length; i++) {
        TorqValue* elem = arr->array->elements[i];
        if (elem && elem->type == TV_DICT) {
            TorqValue* val = torq_dict_get(elem, field->string);
            if (torq_is_truthy(val)) torq_array_push_mut(result, elem);
        }
    }
    return result;
}

TorqValue* torq_array_empty(TorqValue* arr) {
    if (!arr || arr->type != TV_ARRAY) return torq_bool(1);
    return torq_bool(arr->array->length == 0);
}

// ===== Advanced Dict Operations =====

TorqValue* torq_dict_set(TorqValue* d, TorqValue* key, TorqValue* val) {
    if (!d || d->type != TV_DICT || !key || key->type != TV_STR) return d ? d : torq_dict_new();
    // Create a copy with the key set
    TorqValue* result = torq_dict_new();
    TorqDict* src = d->dict;
    for (int64_t i = 0; i < src->length; i++) {
        torq_dict_set_mut(result, src->entries[i].key, src->entries[i].value);
    }
    torq_dict_set_mut(result, key->string, val);
    return result;
}

TorqValue* torq_dict_drop(TorqValue* d, TorqValue* key) {
    if (!d || d->type != TV_DICT || !key || key->type != TV_STR) return d ? d : torq_dict_new();
    TorqValue* result = torq_dict_new();
    TorqDict* src = d->dict;
    for (int64_t i = 0; i < src->length; i++) {
        if (strcmp(src->entries[i].key, key->string) != 0) {
            torq_dict_set_mut(result, src->entries[i].key, src->entries[i].value);
        }
    }
    return result;
}

TorqValue* torq_dict_merge(TorqValue* a, TorqValue* b) {
    if (!a || a->type != TV_DICT) return b && b->type == TV_DICT ? b : torq_dict_new();
    if (!b || b->type != TV_DICT) return a;
    TorqValue* result = torq_dict_new();
    TorqDict* da = a->dict;
    for (int64_t i = 0; i < da->length; i++) {
        torq_dict_set_mut(result, da->entries[i].key, da->entries[i].value);
    }
    TorqDict* db = b->dict;
    for (int64_t i = 0; i < db->length; i++) {
        torq_dict_set_mut(result, db->entries[i].key, db->entries[i].value);
    }
    return result;
}

TorqValue* torq_dict_pick(TorqValue* d, TorqValue* keys) {
    if (!d || d->type != TV_DICT || !keys || keys->type != TV_ARRAY) return torq_dict_new();
    TorqValue* result = torq_dict_new();
    for (int64_t i = 0; i < keys->array->length; i++) {
        TorqValue* k = keys->array->elements[i];
        if (k && k->type == TV_STR) {
            TorqDict* dict = d->dict;
            for (int64_t j = 0; j < dict->length; j++) {
                if (strcmp(dict->entries[j].key, k->string) == 0) {
                    torq_dict_set_mut(result, dict->entries[j].key, dict->entries[j].value);
                    break;
                }
            }
        }
    }
    return result;
}

TorqValue* torq_dict_omit(TorqValue* d, TorqValue* keys) {
    if (!d || d->type != TV_DICT) return torq_dict_new();
    if (!keys || keys->type != TV_ARRAY) return d;
    TorqValue* result = torq_dict_new();
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        int skip = 0;
        for (int64_t j = 0; j < keys->array->length; j++) {
            TorqValue* k = keys->array->elements[j];
            if (k && k->type == TV_STR && strcmp(dict->entries[i].key, k->string) == 0) {
                skip = 1;
                break;
            }
        }
        if (!skip) {
            torq_dict_set_mut(result, dict->entries[i].key, dict->entries[i].value);
        }
    }
    return result;
}

TorqValue* torq_dict_entries(TorqValue* d) {
    TorqValue* arr = torq_array_new();
    if (!d || d->type != TV_DICT) return arr;
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        TorqValue* pair = torq_array_new();
        torq_array_push_mut(pair, torq_str(dict->entries[i].key));
        torq_array_push_mut(pair, dict->entries[i].value);
        torq_array_push_mut(arr, pair);
    }
    return arr;
}

TorqValue* torq_dict_empty(TorqValue* d) {
    if (!d || d->type != TV_DICT) return torq_bool(1);
    return torq_bool(d->dict->length == 0);
}

// ===== Power operator =====

TorqValue* torq_pow(TorqValue* a, TorqValue* b) {
    if (!a || !b) return torq_int(0);
    double base = (a->type == TV_FLOAT) ? a->floating : (double)torq_as_int(a);
    double exp = (b->type == TV_FLOAT) ? b->floating : (double)torq_as_int(b);
    double result = pow(base, exp);
    // If both inputs are int and result is integer-valued, return int
    if (a->type == TV_INT && b->type == TV_INT && exp >= 0 && result == (double)(int64_t)result) {
        return torq_int((int64_t)result);
    }
    return torq_float(result);
}

// ===== JSON Parsing =====

// Forward declarations for JSON parser
static TorqValue* json_parse_value(const char** p);
static void json_skip_ws(const char** p);

static void json_skip_ws(const char** p) {
    while (**p == ' ' || **p == '\t' || **p == '\n' || **p == '\r') (*p)++;
}

static TorqValue* json_parse_string(const char** p) {
    if (**p != '"') return torq_null();
    (*p)++; // skip opening quote
    size_t cap = 64;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    while (**p && **p != '"') {
        if (**p == '\\') {
            (*p)++;
            switch (**p) {
                case '"': buf[len++] = '"'; break;
                case '\\': buf[len++] = '\\'; break;
                case 'n': buf[len++] = '\n'; break;
                case 't': buf[len++] = '\t'; break;
                case 'r': buf[len++] = '\r'; break;
                case '/': buf[len++] = '/'; break;
                default: buf[len++] = **p; break;
            }
        } else {
            buf[len++] = **p;
        }
        if (len >= cap - 1) { cap *= 2; buf = (char*)realloc(buf, cap); }
        (*p)++;
    }
    if (**p == '"') (*p)++; // skip closing quote
    buf[len] = '\0';
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

static TorqValue* json_parse_number(const char** p) {
    const char* start = *p;
    int is_float = 0;
    if (**p == '-') (*p)++;
    while (**p >= '0' && **p <= '9') (*p)++;
    if (**p == '.') { is_float = 1; (*p)++; while (**p >= '0' && **p <= '9') (*p)++; }
    if (**p == 'e' || **p == 'E') { is_float = 1; (*p)++; if (**p == '+' || **p == '-') (*p)++; while (**p >= '0' && **p <= '9') (*p)++; }
    if (is_float) return torq_float(strtod(start, NULL));
    return torq_int(strtoll(start, NULL, 10));
}

static TorqValue* json_parse_array(const char** p) {
    (*p)++; // skip '['
    json_skip_ws(p);
    TorqValue* arr = torq_array_new();
    if (**p == ']') { (*p)++; return arr; }
    while (1) {
        json_skip_ws(p);
        TorqValue* val = json_parse_value(p);
        torq_array_push_mut(arr, val);
        json_skip_ws(p);
        if (**p == ',') { (*p)++; continue; }
        if (**p == ']') { (*p)++; break; }
        break; // malformed
    }
    return arr;
}

static TorqValue* json_parse_object(const char** p) {
    (*p)++; // skip '{'
    json_skip_ws(p);
    TorqValue* dict = torq_dict_new();
    if (**p == '}') { (*p)++; return dict; }
    while (1) {
        json_skip_ws(p);
        if (**p != '"') break;
        TorqValue* key = json_parse_string(p);
        json_skip_ws(p);
        if (**p == ':') (*p)++;
        json_skip_ws(p);
        TorqValue* val = json_parse_value(p);
        if (key->type == TV_STR) {
            torq_dict_set_mut(dict, key->string, val);
        }
        json_skip_ws(p);
        if (**p == ',') { (*p)++; continue; }
        if (**p == '}') { (*p)++; break; }
        break; // malformed
    }
    return dict;
}

static TorqValue* json_parse_value(const char** p) {
    json_skip_ws(p);
    if (**p == '"') return json_parse_string(p);
    if (**p == '{') return json_parse_object(p);
    if (**p == '[') return json_parse_array(p);
    if (**p == 't' && strncmp(*p, "true", 4) == 0) { *p += 4; return torq_bool(1); }
    if (**p == 'f' && strncmp(*p, "false", 5) == 0) { *p += 5; return torq_bool(0); }
    if (**p == 'n' && strncmp(*p, "null", 4) == 0) { *p += 4; return torq_null(); }
    if (**p == '-' || (**p >= '0' && **p <= '9')) return json_parse_number(p);
    return torq_null();
}

TorqValue* torq_from_json(TorqValue* v) {
    if (!v || v->type != TV_STR) return torq_null();
    const char* p = v->string;
    return json_parse_value(&p);
}

// ===== System operations =====

TorqValue* torq_sys_exec(TorqValue* cmd) {
    if (!cmd || cmd->type != TV_STR) return torq_null();
    FILE* fp = popen(cmd->string, "r");
    if (!fp) return torq_null();
    size_t cap = 1024;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    char tmp[256];
    while (fgets(tmp, sizeof(tmp), fp)) {
        size_t tlen = strlen(tmp);
        while (len + tlen >= cap) { cap *= 2; buf = (char*)realloc(buf, cap); }
        memcpy(buf + len, tmp, tlen);
        len += tlen;
    }
    buf[len] = '\0';
    pclose(fp);
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

TorqValue* torq_sys_args(void) {
    // Returns empty array — CLI arg handling requires main(argc, argv) support
    return torq_array_new();
}

// ===== Type checking =====

TorqValue* torq_type_of(TorqValue* v) {
    if (!v) return torq_str("null");
    switch (v->type) {
        case TV_NULL: return torq_str("null");
        case TV_INT: return torq_str("int");
        case TV_FLOAT: return torq_str("float");
        case TV_BOOL: return torq_str("bool");
        case TV_STR: return torq_str("string");
        case TV_ARRAY: return torq_str("array");
        case TV_DICT: return torq_str("dict");
    }
    return torq_str("unknown");
}

// ===== TorqValue*-key wrappers for pipeline use =====
// These accept TorqValue* keys instead of raw const char*

TorqValue* torq_dict_get_tv(TorqValue* d, TorqValue* key) {
    if (!key || key->type != TV_STR) return torq_null();
    return torq_dict_get(d, key->string);
}

TorqValue* torq_dict_has_tv(TorqValue* d, TorqValue* key) {
    if (!key || key->type != TV_STR) return torq_bool(0);
    return torq_dict_has(d, key->string);
}
