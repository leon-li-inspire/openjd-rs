# openjd-expr Crate Quality Evaluation Report

**Date:** 2026-04-13
**Evaluator:** AI-assisted review
**Crate version:** 0.1.0
**Scope:** Specifications, implementation source, and tests

---

## Executive Summary

The `openjd-expr` crate is a well-engineered Rust implementation of the OpenJD Expression
Language. It has 2,819 tests across 34 test binaries, all passing with zero warnings.
The specifications are comprehensive and mostly aligned with the implementation, though
several areas of spec drift were identified. One confirmed bug was found (symbol table
conflict detection), along with two confirmed spec violations (URI case sensitivity,
operation limit error types) and one performance issue (format string re-parsing).

**Overall assessment:** High quality, with targeted improvements needed.

---

## 1. Build and Test Results

- **Build:** Compiles cleanly with `cargo build --package openjd-expr` — zero errors, zero warnings.
- **Tests:** 2,819 tests across 34 test binaries — all pass.
  - 119 unit tests (in-crate `#[cfg(test)]` modules)
  - 2,696 integration tests (32 files in `tests/`)
  - 4 doc-tests

---

## 2. Specification Review

### 2.1 Specification Documents

The `specs/expr/` directory contains 11 well-structured documents:

| Document | Size | Quality |
|----------|------|---------|
| README.md | Overview | ✅ Good — clear index with normative references |
| architecture.md | 7.6KB | ✅ Good — accurate module layout, dependency graph, public API |
| parser.md | 5.4KB | ✅ Good — faithful description of parsing pipeline |
| type-system.md | 5.3KB | ✅ Good — complete type system description |
| values.md | 6.2KB | ⚠️ Stale — enum definition missing cached size fields |
| symbol-table.md | 3.4KB | ✅ Good — minor naming discrepancy |
| evaluator.md | 10.9KB | ✅ Good — thorough evaluator description |
| function-library.md | 8.4KB | ⚠️ Stale — API signatures differ from implementation |
| format-string.md | 4.1KB | ⚠️ Stale — claims pre-parsing that doesn't happen |
| range-expr.md | 4.9KB | ⚠️ Conflict — shows descending ranges as valid but code rejects them |
| path-mapping.md | 5.2KB | ✅ Good — minor return type difference |
| error-formatting.md | 3.8KB | ✅ Good — accurate description |

### 2.2 Spec-Implementation Discrepancies

#### Critical

| # | Area | Issue |
|---|------|-------|
| S1 | format-string.md | Spec says expressions are "pre-parsed `ParsedExpression` objects, avoiding re-parsing on each resolution call" but the implementation stores expressions as strings and re-parses on every `resolve` call. |
| S2 | range-expr.md | Spec shows `"10-1"` as valid (producing `[10,9,...,1]`) but code rejects descending ranges without a negative step. Code comment says "invalid per spec" — contradicts the spec document. |
| S3 | function-library.md | Spec says `library.call(name, &arg_types, &args, eval_context)` but implementation is `library.call(name, &args, ctx)` — arg types are derived internally. |

#### Medium

| # | Area | Issue |
|---|------|-------|
| S4 | values.md | Enum definition missing `usize` cached memory size fields on `ListString`, `ListPath`, `ListList`, and `ExprType` field on `ListList`. |
| S5 | values.md | Spec says empty list defaults to `ListInt`; implementation defaults to `ListString` for null/unknown hints. |
| S6 | values.md | Spec says `make_list` "validates max 2 nesting levels" but implementation does no depth validation. |
| S7 | values.md | Spec doesn't document `hint_type` parameter on `make_list` or nested list promotion rules. |
| S8 | values.md | Spec lists `RANGE_EXPR → STRING` and `RANGE_EXPR → LIST[INT]` target type coercions not present in `coerce()`. |
| S9 | function-library.md | `EvalContext` trait methods return `Result<(), ExpressionError>` but spec shows them returning `()`. |
| S10 | function-library.md | `EvalContext` has `get_or_compile_regex()` method not mentioned in spec. |
| S11 | type-system.md | Spec defines `LIST_INT`, `LIST_FLOAT`, `LIST_STRING`, `LIST_PATH`, `LIST_BOOL`, `LIST_LIST_INT`, `EMPTY_LIST` constants not present in implementation. |

#### Low

| # | Area | Issue |
|---|------|-------|
| S12 | type-system.md | Spec uses `ExprType::NULLTYPE`, implementation uses `ExprType::NULL`. |
| S13 | symbol-table.md | Spec says `SymbolEntry`, code uses `SymbolTableEntry`. |
| S14 | values.md | Spec references `Float64::from_str()` constructor; actual is `Float64::with_str()`. |
| S15 | format-string.md | `copy_used_symtab_values`, `resolve_typed`, `resolve_typed_with_format`, `FormatStringValidationError` not documented. |
| S16 | evaluator.md | Regex cache not mentioned in spec. |

---

## 3. Implementation Review

### 3.1 Confirmed Bugs

#### BUG-1: Symbol Table Conflict Detection is Asymmetric

**Severity:** Medium
**Location:** `symbol_table.rs`, `set()` method

When a table entry exists at a path (e.g., `A.B` is a table containing `A.B.C`), setting
a scalar value at `A.B` silently overwrites the table, destroying all entries beneath it.
The reverse direction (setting `A.B.C` when `A.B` is a scalar) correctly returns an error.

**Reproduction:**
```rust
let mut symtab = SymbolTable::new();
symtab.set("A.B.C", ExprValue::Int(1)).unwrap();
symtab.set("A.B", ExprValue::Int(2));  // Silently succeeds — should error
// A.B.C is now lost
```

**Expected:** `set("A.B", ...)` should return an error when `A.B` is already a table.

#### BUG-2: Operation Limit Error Type Inconsistency

**Severity:** Medium
**Location:** `eval/evaluator.rs`, two `count_op` implementations

The evaluator has two `count_op` methods that produce different error types:

- **Private method** (called from `eval_call`, `eval_compare`, `eval_ifexp`, `eval_subscript`,
  `eval_attribute`, list comprehension): produces `ExpressionErrorKind::Other` with a
  formatted message including counts.
- **EvalContext trait impl** (called from function implementations): produces
  `ExpressionErrorKind::OperationLimitExceeded`.

Code matching on `ExpressionErrorKind::OperationLimitExceeded` will miss operation limit
errors from the evaluator's own control flow. The error messages also differ.

**Fix:** Unify both to use `ExpressionErrorKind::OperationLimitExceeded`. Include the
counts in the error kind's `Display` impl if desired.

#### BUG-3: URI Path Mapping Case Sensitivity Violation

**Severity:** Medium
**Location:** `path_mapping.rs`, `apply_uri()` method

The `apply_uri` method uses `path.starts_with(&self.source_path)` which is case-sensitive.
Per RFC 3986 and the path-mapping.md spec, URI scheme and authority components should be
compared case-insensitively. For example, `S3://Bucket/key` should match a rule with
`source_path: "s3://bucket/key"`, but it won't.

The filesystem path matching (`apply_filesystem`) correctly handles case sensitivity for
Windows paths, but the URI path matching does not.

### 3.2 Performance Issue

#### PERF-1: Format String Re-Parses Expressions on Every Resolve

**Severity:** Medium
**Location:** `format_string.rs`, `eval_segment()` method

The `Segment::Expression` variant stores the expression as a `String`. Every call to
`resolve`, `resolve_string`, or `resolve_typed` calls `ParsedExpression::new(expr)` which
invokes the ruff Python parser. For format strings evaluated repeatedly (e.g., once per
task in a job), this is unnecessary overhead.

**Fix:** Store `ParsedExpression` in the `Segment` enum at `FormatString::new()` time,
matching what the spec describes.

### 3.3 Code Quality Assessment

#### Naming and Consistency

- **Good:** Method names follow consistent conventions (`eval_name`, `eval_attribute`, etc.)
- **Good:** Error constructors are descriptive (`ExpressionError::undefined_variable`, etc.)
- **Good:** Builder pattern with `#[must_use]` on all builder methods
- **Minor:** `SymbolTableEntry` vs spec's `SymbolEntry` — pick one and update the other

#### Error Messages

Error messages are consistently high quality:
- Caret-annotated source display (3-line format: message, expression, caret)
- Smart caret positioning (at operator for BinOp, at attribute name for Attribute, etc.)
- "Did you mean?" suggestions via Levenshtein distance for unknown functions/variables
- Available-types hints for property access on wrong types
- Full error message assertion in tests (per AGENTS.md requirement)

#### Performance

- **Good:** `LazyLock` caching of the default function library
- **Good:** Typed list variants for memory efficiency (97% savings for `list[bool]`)
- **Good:** O(log n) indexing and containment for `RangeExpr` via binary search
- **Good:** Regex cache in the evaluator for repeated pattern compilation
- **Concern:** `ast::Expr` cloning for error context — clones entire AST subtrees
- **Concern:** `build_dotted_name` allocates `Vec<&str>` on every attribute access
- **Concern:** List comprehension creates new `SymbolTable` + `Evaluator` per iteration

#### Algorithmic Complexity

No O(N²) or worse algorithms were found. Key operations:
- Expression parsing: O(N) via ruff parser
- Symbol table lookup: O(K) where K is path depth (typically 2-3)
- Function dispatch: O(S) where S is number of overloads (typically <10)
- List operations: O(N) for most, O(N log N) for sort
- RangeExpr indexing: O(log R) where R is number of ranges
- Memory tracking: O(1) per operation

#### Rust Best Practices

- **Good:** `#[non_exhaustive]` on `ExpressionErrorKind` for forward compatibility
- **Good:** `thiserror` for error derivation
- **Good:** `fn` pointers (not closures) in `FunctionEntry` for `Clone + Send + Sync`
- **Good:** `EvalContext` trait boundary prevents function impls from calling evaluator methods
- **Good:** `saturating_sub` for memory release to prevent underflow
- **Minor:** Some `#[allow(dead_code)]` in test helpers that should be cleaned up

---

## 4. Test Review

### 4.1 Coverage and Organization

| Aspect | Assessment |
|--------|-----------|
| Total tests | 2,819 — comprehensive |
| Test files | 34 — well-organized by feature area |
| Happy path coverage | Excellent — all operators, functions, types covered |
| Edge case coverage | Excellent — overflow, empty collections, Unicode, boundary values |
| Error message assertions | Strong — most error tests assert full 3-line caret messages |
| RFC example tests | Present (26 tests) — validates spec compliance |

### 4.2 Test Quality Issues

| # | File | Issue | Severity |
|---|------|-------|----------|
| T1 | test_strings.rs | `eval_fails` helper defined with `#[allow(dead_code)]` — never used | Low |
| T2 | test_strings.rs | Some `split`/`rsplit` tests only assert `is_list()` without checking contents | Medium |
| T3 | test_types.rs | 7 `p_err()` calls only check `is_err()` without asserting error messages | Medium |
| T4 | test_rfc_examples.rs | Some assertions use `contains()` instead of exact value comparison | Low |
| T5 | format_string.rs | 8 test functions outside `mod tests` block (at module scope) | Low |
| T6 | path_mapping.rs | No tests for actual path mapping application logic — only serde tests | High |

### 4.3 Exploratory Test Results

27 exploratory edge-case tests were written and executed:

- **26 passed** — confirming correct behavior for: integer overflow, float edge cases,
  division by zero, empty collections, Unicode string length, truthiness semantics,
  chained comparisons, negative indexing, slicing, power operator, null handling,
  keyword-as-attribute, format strings, ternary expressions.
- **1 failed** — `symbol_table_table_then_value_conflict` (BUG-1 above).

---

## 5. Recommendations

### Priority 1 — Bug Fixes

1. **Fix symbol table conflict detection** (BUG-1): Add a check in `set()` that returns
   an error when the target key already maps to a `SymbolTableEntry::Table`.

2. **Unify operation limit error types** (BUG-2): Change the private `count_op` to use
   `ExpressionErrorKind::OperationLimitExceeded` instead of `ExpressionErrorKind::Other`.

3. **Fix URI case sensitivity** (BUG-3): In `apply_uri`, split the URI into
   scheme+authority and path components, compare scheme+authority case-insensitively
   and path case-sensitively.

### Priority 2 — Performance

4. **Pre-parse format string expressions** (PERF-1): Change `Segment::Expression` to
   store `ParsedExpression` instead of `String`. Parse at `FormatString::new()` time.

### Priority 3 — Spec Updates

5. **Update values.md**: Add cached size fields to enum definition, document `hint_type`
   parameter, document nested list promotion rules, fix `Float64::from_str` reference.

6. **Update function-library.md**: Fix `call()` signature, update `EvalContext` return
   types, document `get_or_compile_regex`.

7. **Update format-string.md**: Either fix the "pre-parsed" claim to match reality, or
   implement pre-parsing (recommendation 4) and keep the spec.

8. **Resolve range-expr.md conflict**: Either update the spec to say descending ranges
   require a negative step, or update the code to accept `"10-1"` as valid. The code
   comment says "invalid per spec" so the spec document may be aspirational.

9. **Update evaluator.md**: Document regex cache, document `eval_attribute` operation
   counting, document unresolved handling in BoolOp.

### Priority 4 — Test Improvements

10. **Add path mapping application tests**: The `path_mapping.rs` test module only tests
    serde — add tests for `apply()`, `apply_rules()`, Windows case-insensitive matching,
    URI matching, and trailing slash preservation.

11. **Strengthen weak assertions**: Fix `split`/`rsplit` tests in test_strings.rs to
    assert exact list contents. Fix `p_err()` calls in test_types.rs to assert error
    messages. Fix `contains()` assertions in test_rfc_examples.rs.

12. **Clean up test organization**: Remove dead `eval_fails` helper. Move 8 tests in
    format_string.rs into the `mod tests` block.

### Priority 5 — Code Quality

13. **Extract `eval_listcomp`**: The list comprehension handling is ~80 lines inline in
    `evaluate_inner`. Extract to a separate method for readability and testability.

14. **Reduce AST cloning for error context**: Consider using `Cow` or span references
    instead of cloning entire AST subtrees for error context attachment.

15. **Add missing type constants**: Implement `ExprType::LIST_INT`, `LIST_FLOAT`,
    `LIST_STRING`, `LIST_PATH`, `LIST_BOOL`, `LIST_LIST_INT`, `EMPTY_LIST` as documented
    in the spec, for API ergonomics.

---

## 6. Summary Scorecard

| Dimension | Score | Notes |
|-----------|-------|-------|
| Spec completeness | 7/10 | Good coverage but several stale sections |
| Spec accuracy | 6/10 | Multiple discrepancies with implementation |
| Implementation correctness | 8/10 | 1 confirmed bug, 2 spec violations |
| Implementation performance | 8/10 | Good overall, format string re-parsing is the main issue |
| Error message quality | 10/10 | Excellent caret formatting, suggestions, context |
| Test coverage | 9/10 | 2,819 tests, comprehensive edge cases, missing path mapping tests |
| Test organization | 8/10 | Well-structured, minor cleanup needed |
| Rust best practices | 9/10 | Good use of type system, traits, error handling |
| Code readability | 8/10 | Clear naming, some large methods could be extracted |
| API ergonomics | 9/10 | Builder pattern, convenience functions, macro support |
