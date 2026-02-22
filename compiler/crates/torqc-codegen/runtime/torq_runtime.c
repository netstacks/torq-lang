#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <ctype.h>
#include <pthread.h>
#include <time.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/time.h>

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

TorqValue* torq_reverse(TorqValue* v) {
    if (!v) return torq_null();
    if (v->type == TV_ARRAY) return torq_array_reverse(v);
    if (v->type == TV_STR) return torq_str_reverse(v);
    return v;
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

// ===== Parallel each =====

typedef TorqValue* (*TorqEachBodyFn)(TorqValue*);

typedef struct {
    TorqEachBodyFn body_fn;
    TorqValue* element;
} TorqThreadArg;

static void* torq_thread_worker(void* arg) {
    TorqThreadArg* ta = (TorqThreadArg*)arg;
    ta->body_fn(ta->element);
    return NULL;
}

void torq_parallel_each_array(TorqValue* arr, TorqEachBodyFn body_fn) {
    if (!arr || arr->type != TV_ARRAY || arr->array->length == 0) return;

    int64_t n = arr->array->length;
    pthread_t* threads = (pthread_t*)malloc(sizeof(pthread_t) * n);
    TorqThreadArg* args = (TorqThreadArg*)malloc(sizeof(TorqThreadArg) * n);

    for (int64_t i = 0; i < n; i++) {
        args[i].body_fn = body_fn;
        args[i].element = arr->array->elements[i];
        pthread_create(&threads[i], NULL, torq_thread_worker, &args[i]);
    }

    for (int64_t i = 0; i < n; i++) {
        pthread_join(threads[i], NULL);
    }

    free(threads);
    free(args);
}

void torq_parallel_each_range(int64_t start, int64_t end, TorqEachBodyFn body_fn) {
    int64_t n = end - start;
    if (n <= 0) return;

    pthread_t* threads = (pthread_t*)malloc(sizeof(pthread_t) * n);
    TorqThreadArg* args = (TorqThreadArg*)malloc(sizeof(TorqThreadArg) * n);

    for (int64_t i = 0; i < n; i++) {
        args[i].body_fn = body_fn;
        args[i].element = torq_int(start + i);
        pthread_create(&threads[i], NULL, torq_thread_worker, &args[i]);
    }

    for (int64_t i = 0; i < n; i++) {
        pthread_join(threads[i], NULL);
    }

    free(threads);
    free(args);
}

// ===== Shared variables (thread-safe) =====

typedef struct {
    TorqValue* value;
    pthread_mutex_t mutex;
} TorqShared;

TorqShared* torq_shared_new(TorqValue* initial) {
    TorqShared* s = (TorqShared*)malloc(sizeof(TorqShared));
    s->value = initial ? initial : torq_null();
    pthread_mutex_init(&s->mutex, NULL);
    return s;
}

TorqValue* torq_shared_read(TorqShared* s) {
    if (!s) return torq_null();
    pthread_mutex_lock(&s->mutex);
    TorqValue* v = s->value;
    pthread_mutex_unlock(&s->mutex);
    return v;
}

void torq_shared_write(TorqShared* s, TorqValue* v) {
    if (!s) return;
    pthread_mutex_lock(&s->mutex);
    s->value = v;
    pthread_mutex_unlock(&s->mutex);
}

// Atomic add for *shared counters
TorqValue* torq_shared_add(TorqShared* s, TorqValue* v) {
    if (!s) return torq_null();
    pthread_mutex_lock(&s->mutex);
    s->value = torq_add(s->value, v);
    TorqValue* result = s->value;
    pthread_mutex_unlock(&s->mutex);
    return result;
}

// ===== Time functions =====

TorqValue* torq_time_now(void) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    double ts = (double)tv.tv_sec + (double)tv.tv_usec / 1000000.0;
    return torq_float(ts);
}

TorqValue* torq_time_unix(void) {
    return torq_int((int64_t)time(NULL));
}

TorqValue* torq_time_format(TorqValue* ts, TorqValue* fmt) {
    time_t t;
    if (ts && ts->type == TV_FLOAT) t = (time_t)ts->floating;
    else if (ts && ts->type == TV_INT) t = (time_t)ts->integer;
    else t = time(NULL);

    const char* format = (fmt && fmt->type == TV_STR) ? fmt->string : "%Y-%m-%d %H:%M:%S";
    char buf[256];
    struct tm* tm_info = localtime(&t);
    strftime(buf, sizeof(buf), format, tm_info);
    return torq_str(buf);
}

TorqValue* torq_time_parse(TorqValue* str_val, TorqValue* fmt) {
    if (!str_val || str_val->type != TV_STR) return torq_null();
    const char* format = (fmt && fmt->type == TV_STR) ? fmt->string : "%Y-%m-%d %H:%M:%S";
    struct tm tm_info = {0};
    strptime(str_val->string, format, &tm_info);
    time_t t = mktime(&tm_info);
    return torq_float((double)t);
}

TorqValue* torq_time_diff(TorqValue* a, TorqValue* b) {
    double ta = (a && a->type == TV_FLOAT) ? a->floating : (a && a->type == TV_INT) ? (double)a->integer : 0.0;
    double tb = (b && b->type == TV_FLOAT) ? b->floating : (b && b->type == TV_INT) ? (double)b->integer : 0.0;
    return torq_float(ta - tb);
}

TorqValue* torq_time_add(TorqValue* ts, TorqValue* seconds) {
    double t = (ts && ts->type == TV_FLOAT) ? ts->floating : (ts && ts->type == TV_INT) ? (double)ts->integer : 0.0;
    double s = (seconds && seconds->type == TV_FLOAT) ? seconds->floating : (seconds && seconds->type == TV_INT) ? (double)seconds->integer : 0.0;
    return torq_float(t + s);
}

void torq_time_sleep(TorqValue* seconds) {
    double s = 0.0;
    if (seconds && seconds->type == TV_FLOAT) s = seconds->floating;
    else if (seconds && seconds->type == TV_INT) s = (double)seconds->integer;
    if (s > 0) {
        struct timespec req;
        req.tv_sec = (time_t)s;
        req.tv_nsec = (long)((s - (double)req.tv_sec) * 1e9);
        nanosleep(&req, NULL);
    }
}

// ===== HTTP client (via fork/exec curl) =====

static TorqValue* http_request(const char* method, TorqValue* url, TorqValue* body) {
    if (!url || url->type != TV_STR) return torq_null();

    char temp_path[] = "/tmp/torq_http_XXXXXX";
    int fd = mkstemp(temp_path);
    if (fd < 0) return torq_null();
    close(fd);

    // Build curl command
    char cmd[4096];
    if (body && body->type == TV_STR) {
        snprintf(cmd, sizeof(cmd),
            "curl -s -X %s -H 'Content-Type: application/json' -d '%s' -o '%s' -w '%%{http_code}' '%s' 2>/dev/null",
            method, body->string, temp_path, url->string);
    } else {
        snprintf(cmd, sizeof(cmd),
            "curl -s -X %s -o '%s' -w '%%{http_code}' '%s' 2>/dev/null",
            method, temp_path, url->string);
    }

    FILE* fp = popen(cmd, "r");
    if (!fp) { unlink(temp_path); return torq_null(); }

    char status_buf[16];
    if (!fgets(status_buf, sizeof(status_buf), fp)) status_buf[0] = '0';
    pclose(fp);

    // Read response body
    FILE* f = fopen(temp_path, "r");
    if (!f) { unlink(temp_path); return torq_null(); }
    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* response_body = (char*)malloc(len + 1);
    fread(response_body, 1, len, f);
    response_body[len] = '\0';
    fclose(f);
    unlink(temp_path);

    // Return dict with status and body
    TorqValue* result = torq_dict_new();
    torq_dict_set_mut(result, "status", torq_int(atoi(status_buf)));
    torq_dict_set_mut(result, "body", torq_str(response_body));
    free(response_body);
    return result;
}

TorqValue* torq_http_get(TorqValue* url) { return http_request("GET", url, NULL); }
TorqValue* torq_http_post(TorqValue* url, TorqValue* body) { return http_request("POST", url, body); }
TorqValue* torq_http_put(TorqValue* url, TorqValue* body) { return http_request("PUT", url, body); }
TorqValue* torq_http_delete(TorqValue* url) { return http_request("DELETE", url, NULL); }

// ===== Crypto: SHA-256 =====

// Minimal SHA-256 implementation
static const uint32_t sha256_k[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

#define SHA256_ROTR(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define SHA256_CH(x,y,z) (((x)&(y))^((~(x))&(z)))
#define SHA256_MAJ(x,y,z) (((x)&(y))^((x)&(z))^((y)&(z)))
#define SHA256_S0(x) (SHA256_ROTR(x,2)^SHA256_ROTR(x,13)^SHA256_ROTR(x,22))
#define SHA256_S1(x) (SHA256_ROTR(x,6)^SHA256_ROTR(x,11)^SHA256_ROTR(x,25))
#define SHA256_s0(x) (SHA256_ROTR(x,7)^SHA256_ROTR(x,18)^((x)>>3))
#define SHA256_s1(x) (SHA256_ROTR(x,17)^SHA256_ROTR(x,19)^((x)>>10))

static void sha256_hash(const uint8_t* data, size_t len, uint8_t out[32]) {
    uint32_t h[8] = {
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    };

    // Padding
    size_t padded_len = ((len + 8) / 64 + 1) * 64;
    uint8_t* msg = (uint8_t*)calloc(padded_len, 1);
    memcpy(msg, data, len);
    msg[len] = 0x80;
    uint64_t bits = (uint64_t)len * 8;
    for (int i = 0; i < 8; i++) msg[padded_len - 1 - i] = (uint8_t)(bits >> (i * 8));

    for (size_t offset = 0; offset < padded_len; offset += 64) {
        uint32_t w[64];
        for (int i = 0; i < 16; i++)
            w[i] = ((uint32_t)msg[offset+i*4]<<24) | ((uint32_t)msg[offset+i*4+1]<<16) |
                    ((uint32_t)msg[offset+i*4+2]<<8) | (uint32_t)msg[offset+i*4+3];
        for (int i = 16; i < 64; i++)
            w[i] = SHA256_s1(w[i-2]) + w[i-7] + SHA256_s0(w[i-15]) + w[i-16];

        uint32_t a=h[0],b=h[1],c=h[2],d=h[3],e=h[4],f=h[5],g=h[6],hh=h[7];
        for (int i = 0; i < 64; i++) {
            uint32_t t1 = hh + SHA256_S1(e) + SHA256_CH(e,f,g) + sha256_k[i] + w[i];
            uint32_t t2 = SHA256_S0(a) + SHA256_MAJ(a,b,c);
            hh=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
        }
        h[0]+=a; h[1]+=b; h[2]+=c; h[3]+=d; h[4]+=e; h[5]+=f; h[6]+=g; h[7]+=hh;
    }
    free(msg);

    for (int i = 0; i < 8; i++) {
        out[i*4] = (uint8_t)(h[i]>>24); out[i*4+1] = (uint8_t)(h[i]>>16);
        out[i*4+2] = (uint8_t)(h[i]>>8); out[i*4+3] = (uint8_t)h[i];
    }
}

TorqValue* torq_crypto_hash(TorqValue* algo, TorqValue* data) {
    if (!data || data->type != TV_STR) return torq_null();
    // Only SHA-256 supported currently
    uint8_t hash[32];
    sha256_hash((const uint8_t*)data->string, strlen(data->string), hash);
    char hex[65];
    for (int i = 0; i < 32; i++) sprintf(hex + i*2, "%02x", hash[i]);
    hex[64] = '\0';
    return torq_str(hex);
}

TorqValue* torq_crypto_uuid(void) {
    // UUID v4: random with version and variant bits set
    uint8_t bytes[16];
    FILE* f = fopen("/dev/urandom", "r");
    if (f) { fread(bytes, 1, 16, f); fclose(f); }
    else { for (int i = 0; i < 16; i++) bytes[i] = (uint8_t)(rand() & 0xff); }

    bytes[6] = (bytes[6] & 0x0f) | 0x40;  // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80;  // variant 1

    char uuid[37];
    snprintf(uuid, sizeof(uuid),
        "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
        bytes[0],bytes[1],bytes[2],bytes[3],bytes[4],bytes[5],bytes[6],bytes[7],
        bytes[8],bytes[9],bytes[10],bytes[11],bytes[12],bytes[13],bytes[14],bytes[15]);
    return torq_str(uuid);
}

// ===== Extended logging =====

void torq_log_info(TorqValue* v) {
    TorqValue* s = torq_to_string(v);
    fprintf(stderr, "[INFO] %s\n", s->string);
}

void torq_log_warn(TorqValue* v) {
    TorqValue* s = torq_to_string(v);
    fprintf(stderr, "[WARN] %s\n", s->string);
}

void torq_log_err(TorqValue* v) {
    TorqValue* s = torq_to_string(v);
    fprintf(stderr, "[ERROR] %s\n", s->string);
}

void torq_log_debug(TorqValue* v) {
    TorqValue* s = torq_to_string(v);
    fprintf(stderr, "[DEBUG] %s\n", s->string);
}

// ===== Math extras =====

TorqValue* torq_math_random(void) {
    // Random float between 0.0 and 1.0
    FILE* f = fopen("/dev/urandom", "r");
    uint32_t r = 0;
    if (f) { fread(&r, sizeof(r), 1, f); fclose(f); }
    else { r = (uint32_t)rand(); }
    return torq_float((double)r / (double)UINT32_MAX);
}

// ===== String extras =====

TorqValue* torq_str_repeat(TorqValue* s, TorqValue* count) {
    if (!s || s->type != TV_STR || !count) return torq_str("");
    int64_t n = (count->type == TV_INT) ? count->integer : 0;
    if (n <= 0) return torq_str("");
    size_t slen = strlen(s->string);
    char* buf = (char*)malloc(slen * n + 1);
    buf[0] = '\0';
    for (int64_t i = 0; i < n; i++) strcat(buf, s->string);
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

TorqValue* torq_str_pad_left(TorqValue* s, TorqValue* width, TorqValue* fill) {
    if (!s || s->type != TV_STR) return torq_str("");
    int64_t w = (width && width->type == TV_INT) ? width->integer : 0;
    const char* fc = (fill && fill->type == TV_STR) ? fill->string : " ";
    size_t slen = strlen(s->string);
    if ((int64_t)slen >= w) return s;
    size_t pad = (size_t)(w - (int64_t)slen);
    char* buf = (char*)malloc(w + 1);
    size_t flen = strlen(fc);
    for (size_t i = 0; i < pad; i++) buf[i] = fc[i % flen];
    memcpy(buf + pad, s->string, slen);
    buf[w] = '\0';
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

TorqValue* torq_str_pad_right(TorqValue* s, TorqValue* width, TorqValue* fill) {
    if (!s || s->type != TV_STR) return torq_str("");
    int64_t w = (width && width->type == TV_INT) ? width->integer : 0;
    const char* fc = (fill && fill->type == TV_STR) ? fill->string : " ";
    size_t slen = strlen(s->string);
    if ((int64_t)slen >= w) return s;
    char* buf = (char*)malloc(w + 1);
    memcpy(buf, s->string, slen);
    size_t flen = strlen(fc);
    for (size_t i = slen; i < (size_t)w; i++) buf[i] = fc[(i - slen) % flen];
    buf[w] = '\0';
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

// ===== Array extras =====

TorqValue* torq_array_reduce(TorqValue* arr, TorqValue* initial, TorqValue* (*fn)(TorqValue*, TorqValue*)) {
    if (!arr || arr->type != TV_ARRAY) return initial ? initial : torq_null();
    TorqValue* acc = initial ? initial : (arr->array->length > 0 ? arr->array->elements[0] : torq_null());
    int64_t start = initial ? 0 : 1;
    for (int64_t i = start; i < arr->array->length; i++) {
        acc = fn(acc, arr->array->elements[i]);
    }
    return acc;
}

TorqValue* torq_array_zip(TorqValue* a, TorqValue* b) {
    if (!a || a->type != TV_ARRAY || !b || b->type != TV_ARRAY) return torq_array_new();
    int64_t len = a->array->length < b->array->length ? a->array->length : b->array->length;
    TorqValue* result = torq_array_new();
    for (int64_t i = 0; i < len; i++) {
        TorqValue* pair = torq_array_new();
        torq_array_push_mut(pair, a->array->elements[i]);
        torq_array_push_mut(pair, b->array->elements[i]);
        torq_array_push_mut(result, pair);
    }
    return result;
}

// ===== Assert (for test framework) =====

void torq_assert(TorqValue* condition, TorqValue* message) {
    if (!torq_is_truthy(condition)) {
        const char* msg = (message && message->type == TV_STR) ? message->string : "assertion failed";
        fprintf(stderr, "ASSERT FAILED: %s\n", msg);
        exit(1);
    }
}

void torq_assert_eq(TorqValue* a, TorqValue* b) {
    TorqValue* eq = torq_eq(a, b);
    if (!torq_is_truthy(eq)) {
        TorqValue* sa = torq_to_string(a);
        TorqValue* sb = torq_to_string(b);
        fprintf(stderr, "ASSERT_EQ FAILED: %s != %s\n", sa->string, sb->string);
        exit(1);
    }
}
