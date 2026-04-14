# Validation Pipeline

The `template::validate_v2023_09` module (crate-private) implements a multi-pass validation
pipeline that runs after serde deserialization. It validates semantic constraints that serde
can't express. The module lives inside `template/` because validation is a template concern,
and is version-scoped to `v2023_09` because validation rules are specific to a spec revision.

## Entry Points

```rust
// Called by decode_job_template
pub(crate) fn validate_job_template(
    jt: &JobTemplate,
    ctx: &ValidationContext,
) -> Result<(), OpenJdError>

// Called by decode_environment_template
pub(crate) fn validate_environment_template(
    et: &EnvironmentTemplate,
    ctx: &ValidationContext,
) -> Result<(), OpenJdError>
```

Both functions compute `EffectiveLimits` and `EffectiveRules` from the context, run all
applicable passes, and return accumulated errors as `OpenJdError::ModelValidation`.

## Pass Architecture

Passes run sequentially. Each pass receives the template and the computed limits/rules,
and appends errors to a shared `ValidationErrors` collector. All passes run regardless
of earlier errors (no short-circuiting), so users see all problems at once.

| Pass | File | Purpose |
|------|------|---------|
| 2 | `limits.rs` | Enforce numeric limits (name lengths, counts); FEATURE_BUNDLE_1 raises many limits |
| 3 | `structure.rs` | Structural validation (uniqueness, required fields, dependencies) |
| 4 | `feature_bundle_1.rs` | Gate FEATURE_BUNDLE_1 features (simple actions, endOfLine) |
| 5 | `format_strings.rs` | Validate format string variable references; adapts scopes and expression complexity based on EXPR |
| 6 | `task_chunking.rs` | Gate TASK_CHUNKING features (ChunkInt parameters) |

Passes are numbered starting at 2 because Pass 0 (YAML/JSON parsing) and Pass 1
(version dispatch + serde deserialization) happen in the `parse` module.

## Pass 2: Limits Enforcement

Walks the template tree checking every name length, list count, and string length against
`EffectiveLimits`. Pure numeric checks with no extension branching.

Checks include:
- Job name length vs `max_job_name_len`
- Parameter count vs `max_param_count`
- Parameter name lengths vs `max_identifier_len`
- Step name lengths vs `max_step_name_len`
- Embedded file name/filename lengths
- Task parameter name lengths
- Environment name lengths

## Pass 3: Structural Validation

The largest pass. Validates template structure using `EffectiveRules`. Key checks:

**Template level:**
- At least one step required
- Job name non-empty, no control characters
- Extensions list non-empty if present
- Description length and control character validation

**Parameter definitions:**
- Non-empty if present
- No duplicate names (case-sensitive)
- Parameter type in `rules.allowed_job_param_types`
- Type-specific validation via `validate_definition(limits)`

**Environment uniqueness:**
- Names unique across ALL environments (job + all step environments)

**Step validation:**
- No duplicate step names
- Step name non-empty, no control characters
- Must have `script` or exactly one simple action field (mutually exclusive)
- Dependencies: no self-dependency, target must exist, no duplicates
- Host requirements: amounts/attributes validation, capability name patterns,
  reserved scope checks, standard capability value validation
- Parameter space: ≤16 task parameters, no duplicate names, type allowed,
  range validation per type, combination expression validation
- Script actions: command non-empty, length limits, `Task.File.*` references
  must match embedded file names
- Embedded files: no duplicate names, type must be `TEXT`, valid identifier names,
  data required, filename no path separators

**Cycle detection:**
- Kahn's algorithm on step dependency graph

**Combination expression validation:**
- Character allowlist, balanced parentheses, tokenization
- All referenced parameters must exist and appear exactly once
- All defined parameters must appear in the expression

## Pass 4: FEATURE_BUNDLE_1 Gating

Validates or rejects features gated behind `FEATURE_BUNDLE_1`:

- **SimpleAction fields** (bash, python, cmd, powershell, node): Rejected without extension;
  mutually exclusive with `script` when enabled
- **`endOfLine` on embedded files**: Rejected without extension; must be `LF`, `CRLF`, or
  `AUTO` when enabled

## Pass 5: Format String Validation

The most complex pass. Validates that all format string references resolve to defined
variables by building scope-appropriate symbol tables.

### Symbol Table Construction

Four scope levels, each building on the previous:

1. **Param symtab** — `Param.*` and `RawParam.*` from job parameter definitions.
   PATH types excluded from `Param.*` at template scope (host-only).
   `RawParam.*` for PATH types is STRING.

2. **Template scope** — For job name, host requirements, parameter space ranges.
   Uses param symtab without PATH parameters.

3. **Session scope** — For environment scripts/variables. Adds `Session.WorkingDirectory`,
   `Session.HasPathMappingRules`, `Session.PathMappingRulesFile`, `Env.File.*`.
   With EXPR: adds `Step.Name` in step environments.

4. **Task scope** — For step scripts. Adds `Task.Param.*`, `Task.RawParam.*`,
   `Task.File.*`. With EXPR: adds `Job.Name`, `Step.Name`, `Env.File.*` from
   step and job environments.

### Let Binding Validation

Let bindings are validated with these rules:
- Non-empty if present, ≤50 bindings
- Each binding has `=` separator
- Name: non-empty, starts with lowercase/underscore, alphanumeric+underscore
- No duplicate names
- No shadowing of enclosing scope names
- No self-references (checked via regex on non-string-literal portions)
- Expression parsed and type-checked; result type added to symtab for subsequent bindings
- On error, added as `unresolved(ANY)` to prevent cascading errors

### Function Libraries

Two libraries control available functions in expressions:
- **`default_lib`** — Template-scope expressions (no host functions)
- **`host_lib`** — `default_lib.with_host_context()` for task/session-scope expressions

## Pass 6: TASK_CHUNKING Gating

Validates or rejects features gated behind `TASK_CHUNKING`:

- `ChunkInt` task parameters rejected without extension
- With extension: `defaultTaskCount` ≥ 1, `targetRuntimeSeconds` ≥ 0
- Only one `ChunkInt` parameter per step
- `ChunkInt` parameter must not appear inside parentheses in the combination expression
  (must not be in an associative combination)

## Error Infrastructure

See [error-handling.md](error-handling.md) for details on `ValidationErrors`, `PathElement`,
and error formatting.

## Shared Helpers

### Regex Patterns

| Pattern | Purpose |
|---------|--------|
| `AMOUNT_CAP_RE` | Amount capability name: `[scope:]amount.name[.sub]` |
| `ATTR_CAP_RE` | Attribute capability name: `[scope:]attr.name[.sub]` |
| `ATTR_VALUE_RE` | Attribute value: `[A-Za-z_][A-Za-z0-9_-]*` |

### Constants

| Constant | Values |
|----------|--------|
| `STANDARD_AMOUNT_CAPABILITIES` | `worker.vcpu`, `worker.memory`, `worker.gpu`, `worker.gpu.memory`, `worker.disk.scratch` |
| `STANDARD_ATTRIBUTE_CAPABILITIES` | `worker.os.family`, `worker.cpu.arch` |
| `RESERVED_SCOPES` | `worker`, `job`, `step`, `task` |

### Utility Functions

- `has_control_chars(s)` — True if string contains control chars other than `\n`, `\r`, `\t`
- `check_capability_reserved_scope(name, standard, path, errors)` — Errors if non-standard
  capability uses a reserved scope
- `validate_env_var_name(name, path, errors)` — Non-empty, ≤256 chars, no leading digit,
  alphanumeric+underscore only
