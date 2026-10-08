# Vran 0.1 Loop grammar (verbatim)

- **Status:** Recorded for review. Conformance to Silc is not claimed.
- **Date:** 2026-10-08
- **Source:** private repository `thoughtpivot/vran-oss`, branch `main`, commit `699ec980` (29 September 2026). No parser or compiler shipped with that commit.
- **Credit:** Billy (Bilyal Mestanov) proposed the Loop grammar. The files below are the version of that proposal implemented in vran-oss. He committed them as `docs(spec): add Vran 0.1 language spec, plugin model, grammar, and construction examples`.
- **Related:** [ADR-019](ADR-019-vran-language-into-loop.md), [ADR-014](ADR-014-loop-subject.md), [ADR-015](ADR-015-silc-loop-and-vran.md)

This page copies the draft into Silc so vran-oss can be retired without losing the grammar. Two bodies of text are verbatim, and nothing has been edited inside them:

1. `spec/grammar.ebnf`, the complete grammar, in the first appendix.
2. The syntax sections of `spec/vran-0.1.md` listed in the second appendix: lexical structure, `use`, inputs, the register declaration, workflows, triggers, values and conditions, each step's syntax block, `stop` / `fail` / `skip`, `retry`, and the formatting rules that fix clause order.

Execution, diagnostics, plugins, rationale, the Plan IR note, and the construction examples are not copied here. [ADR-019](ADR-019-vran-language-into-loop.md) is the design disposition of those ideas. It is not a conformance result.

## To be confirmed by Dan

The middle column names a Silc form a reviewer might compare with the grammar. It is a pointer, not a finding that the construct is honored. Every status is **unconfirmed**.

| Grammar construct | Silc `loop` equivalent | Status |
| --- | --- | --- |
| `vran` header, `.vran` source file | Silc `.silc` program and `@version` | unconfirmed |
| `use` of a local module or `package@major` | no `loop::*` import form | unconfirmed |
| `input` | no loop input declaration | unconfirmed |
| `enum`, `type`, bounded `list` | `contract` and Silc types | unconfirmed |
| `register` (`of`, `key`, `sequence`, `stored in`, `readonly`, `person owns`, `derived`) | `resource` | unconfirmed |
| `workflow` name, optional `timezone`, one trigger, then steps | `loop` name and `loop::flow` | unconfirmed |
| `on manual` | `loop::manual` | unconfirmed |
| `on schedule every … at … in …` | `loop::schedule` `:cron` and `:tz` | unconfirmed |
| `on schedule cron "…" in …` | `loop::schedule` `:cron` and `:tz` | unconfirmed |
| `catch up` | `loop::schedule` `:catch_up` | unconfirmed |
| plugin trigger (`on` `alias.op`) and `once per` | no plugin trigger; `loop::on_mutation` is a resource trigger | unconfirmed |
| `read` | `loop::read` | unconfirmed |
| `ask` (`returns`, `from`, `cite`, `using`, `prompt`, `else`) | `loop::ask` | unconfirmed |
| `let` | `loop::let` | unconfirmed |
| `find` / `find one` | `loop::find` | unconfirmed |
| `check` | `loop::gate` | unconfirmed |
| `match` / `when` / `else` | `loop::branch`, `loop::when`, `loop::otherwise` | unconfirmed |
| `each` … `max` | `loop::each` | unconfirmed |
| `fallback` | no `loop::fallback` node | unconfirmed |
| `signoff` (`by`, `via`, `within`, `when declined`, `when timed_out`) | `loop::approve`, `loop::declined`, `loop::timed_out` | unconfirmed |
| `upsert` | `loop::write` | unconfirmed |
| `do` | `loop::notify` (in-app notice) | unconfirmed |
| `stop`, `fail`, `skip` | `loop::stop`, `loop::fail`, `loop::skip` | unconfirmed |
| `key` | `:key` on `loop::write` and `loop::notify` | unconfirmed |
| `retry` | `:retry` on `loop::read` and `loop::ask` | unconfirmed |
| `unchecked` | `:unchecked` on `loop::write` and `loop::notify` | unconfirmed |
| `unattended` | no catalog option | unconfirmed |
| `.vrani` `plugin` interface (`trigger`, `read`, `effect`, `storage`, `channel`, `reason`, `pure`) | no plugin interface; catalog is closed | unconfirmed |
| indentation, two spaces, hard keywords, `{path}` interpolation | Silc surface syntax and `{$binding.path}` in loop string options | unconfirmed |

## Appendix A — `spec/grammar.ebnf`

Copied unchanged from commit `699ec980`.

```ebnf
(* Vran 0.1 grammar.

   Notation: ISO-style EBNF. `=` defines a rule, `;` ends it, `|` separates
   alternatives, `[ x ]` is optional, `{ x }` repeats zero or more times,
   `( ... )` groups. Quoted words are keywords or punctuation. Words that are
   not hard keywords (spec section 3.4) are matched by spelling in the
   position shown and remain usable as names elsewhere.

   Tokens in UPPER CASE come from the lexer (spec section 3). NEWLINE, INDENT
   and DEDENT are produced from line structure; there are no braces for
   blocks. Inside ( ), [ ] and { } line breaks are ignored.

   The grammar is LL(1) except in one place: a `value` that starts with
   NAME "." NAME is a call when the next token is "(", and a path otherwise. *)


(* ---------- Files ---------- *)

source_file    = header { use_decl } { top_decl } ;
interface_file = header plugin_decl { interface_decl } ;

header   = "vran" NUMBER NEWLINE ;
use_decl = "use" ( STRING | PACKAGE "@" INTEGER [ "as" NAME ] ) NEWLINE ;

top_decl = [ doc ] ( input_decl | enum_decl | type_decl | register_decl | workflow_decl ) ;
doc      = DOC { DOC } ;


(* ---------- Declarations ---------- *)

input_decl  = "input" NAME type_ref [ "default" literal ] NEWLINE ;

enum_decl   = "enum" TYPE_NAME NEWLINE INDENT enum_member { enum_member } DEDENT ;
enum_member = [ doc ] NAME NEWLINE ;

type_decl   = "type" TYPE_NAME NEWLINE INDENT field { field } DEDENT ;
field       = [ doc ] NAME type_ref NEWLINE ;

type_ref    = base_type [ "?" ] ;
base_type   = PRIMITIVE
            | TYPE_NAME
            | NAME "." TYPE_NAME
            | "list" "[" type_ref "max" INTEGER "]" ;

register_decl = "register" NAME NEWLINE INDENT register_item { register_item } DEDENT ;
register_item = "of" TYPE_NAME NEWLINE
              | "key" NAME NEWLINE
              | "sequence" STRING NEWLINE
              | "stored" "in" op_ref args
              | "readonly" NEWLINE
              | "person" "owns" NAME { "," NAME } NEWLINE
              | "derived" NAME "=" value NEWLINE ;

workflow_decl = "workflow" NAME NEWLINE INDENT
                  [ "timezone" value NEWLINE ]
                  trigger
                  step { step }
                DEDENT ;


(* ---------- Triggers ---------- *)

trigger        = "on" trigger_source NEWLINE [ INDENT trigger_item { trigger_item } DEDENT ] ;
trigger_source = "manual"
               | "schedule" schedule
               | op_ref ;
schedule       = ( "every" recurrence "at" TIME | "cron" STRING ) "in" value ;
recurrence     = "day" | "weekday" | WEEKDAY | "month" "on" "day" INTEGER ;
trigger_item   = "catch" "up" DURATION NEWLINE
               | "once" "per" path NEWLINE
               | arg ;

args = NEWLINE [ INDENT arg { arg } DEDENT ] ;
arg  = NAME ( value | TYPE_NAME ) NEWLINE ;


(* ---------- Steps ---------- *)

block = NEWLINE INDENT step { step } DEDENT ;

step  = [ doc ] ( read_step | ask_step | let_step | find_step | check_step
                | match_step | each_step | fallback_step | signoff_step
                | upsert_step | do_step | stop_step | fail_step | skip_step ) ;

read_step     = "read" NAME "from" op_ref NEWLINE [ INDENT read_item { read_item } DEDENT ] ;
read_item     = arg | retry ;

ask_step      = "ask" NAME "returns" TYPE_NAME NEWLINE INDENT ask_item { ask_item } DEDENT ;
ask_item      = "from" path { "," path } NEWLINE
              | "cite" path { "," path } NEWLINE
              | "using" NAME NEWLINE
              | retry
              | "prompt" TEXT_BLOCK NEWLINE
              | else_block ;

let_step      = "let" NAME "=" value NEWLINE ;

find_step     = "find" [ "one" ] NAME "in" NAME NEWLINE [ INDENT find_item { find_item } DEDENT ] ;
find_item     = "where" condition NEWLINE
              | "order" "by" NAME [ "desc" ] NEWLINE
              | "max" INTEGER NEWLINE ;

check_step    = "check" NAME NEWLINE INDENT check_item { check_item } DEDENT ;
check_item    = "that" condition NEWLINE
              | "reason" STRING NEWLINE
              | else_block ;

match_step    = "match" path NEWLINE INDENT when_arm { when_arm } [ else_block ] DEDENT ;
when_arm      = "when" pattern block ;
pattern       = "missing"
              | compare_op operand
              | pattern_value { "," pattern_value } ;
pattern_value = literal | NAME ;

each_step     = "each" NAME "in" path "max" INTEGER block ;

fallback_step = "fallback" NAME NEWLINE INDENT
                  try_item try_item { try_item }
                  "accept" "when" condition NEWLINE
                  [ else_block ]
                DEDENT ;
try_item      = "try" "read" "from" op_ref args ;

signoff_step  = "signoff" NAME NEWLINE INDENT signoff_item { signoff_item } DEDENT ;
signoff_item  = "by" value NEWLINE
              | "via" op_ref NEWLINE
              | "message" value NEWLINE
              | "show" path { "," path } NEWLINE
              | "decide" TYPE_NAME NEWLINE
              | "within" DURATION NEWLINE
              | "when" ( "declined" | "timed_out" ) block ;

upsert_step   = "upsert" NAME [ "as" NAME ] NEWLINE INDENT upsert_item { upsert_item } DEDENT ;
upsert_item   = "where" NAME "is" value NEWLINE
              | "set" NAME value NEWLINE
              | key
              | unchecked ;

do_step       = "do" op_ref [ "as" NAME ] NEWLINE [ INDENT do_item { do_item } DEDENT ] ;
do_item       = arg | key | retry | unchecked | "unattended" STRING NEWLINE ;

stop_step     = "stop" [ STRING ] NEWLINE ;
fail_step     = "fail" STRING NEWLINE ;
skip_step     = "skip" STRING NEWLINE ;

else_block    = "else" block ;
key           = "key" STRING NEWLINE ;
retry         = "retry" INTEGER NEWLINE ;
unchecked     = "unchecked" STRING NEWLINE ;


(* ---------- Values ---------- *)

value        = literal | path | call | list_value | record_value ;
call         = NAME "." NAME "(" [ value { "," value } ] ")" ;
list_value   = "[" [ value { "," value } ] "]" ;
record_value = "{" NAME ":" value { "," NAME ":" value } "}" ;
path         = NAME { "." NAME } ;
op_ref       = NAME "." NAME ;

literal      = STRING | TEXT_BLOCK | NUMBER | INTEGER | PERCENT | DURATION
             | DATE | TIME | MONEY | "true" | "false" ;


(* ---------- Conditions ---------- *)

condition   = conjunction { "or" conjunction } ;
conjunction = negation { "and" negation } ;
negation    = [ "not" ] comparison ;
comparison  = "(" condition ")"
            | operand ( "is" ( "missing" | "present" | [ "not" ] operand )
                      | compare_op operand
                      | [ "not" ] "in" list_value ) ;
operand     = literal | path | list_value ;
compare_op  = "<" | "<=" | ">" | ">=" ;


(* ---------- Interface files (.vrani) ---------- *)

plugin_decl    = [ doc ] "plugin" PACKAGE SEMVER NEWLINE [ INDENT plugin_item { plugin_item } DEDENT ] ;
plugin_item    = "config" NAME type_ref NEWLINE
               | "secret" NAME NEWLINE
               | "host" ( STRING | path ) NEWLINE ;

interface_decl = [ doc ] ( enum_decl | type_decl | operation_decl | pure_decl ) ;

operation_decl = capability NAME NEWLINE [ INDENT op_item { op_item } DEDENT ] ;
capability     = "trigger" | "read" | "effect" | "storage" | "channel" | "reason" ;
op_item        = "returns" type_ref NEWLINE
               | "param" NAME type_ref NEWLINE
               | "identity" NAME NEWLINE
               | "shape" NAME NEWLINE
               | "idempotency" ( "native" | "none" ) NEWLINE
               | "irreversible" NEWLINE
               | "readonly" NEWLINE ;

pure_decl      = "pure" NAME [ "[" TYPE_NAME { "," TYPE_NAME } "]" ]
                 "(" [ pure_param { "," pure_param } ] ")" "returns" type_ref NEWLINE ;
pure_param     = NAME type_ref ;


(* ---------- Lexical tokens (informal; see spec section 3) ----------

   NAME        [a-z_][a-z0-9_]*   (all lowercase words, including contextual
               keywords; hard keywords are listed in spec section 3.4)
   TYPE_NAME   [A-Z][A-Za-z0-9]*
   PACKAGE     [a-z][a-z0-9-]* "/" [a-z][a-z0-9-]*   (only after "use" and "plugin")
   PRIMITIVE   a NAME spelled text | number | integer | boolean | date | datetime
               | duration | money | percent | file | citation | any | schema,
               recognized only in type position
   INTEGER     -?[0-9]+
   NUMBER      -?[0-9]+ "." [0-9]+
   SEMVER      [0-9]+ "." [0-9]+ "." [0-9]+
   PERCENT     [0-9]+ ( "." [0-9]+ )? "%"
   DURATION    [0-9]+ ( "m" | "h" | "d" | "w" )
   DATE        [0-9]{4} "-" [0-9]{2} "-" [0-9]{2}
   TIME        [0-9]{2} ":" [0-9]{2}
   MONEY       [A-Z]{3} " " -?[0-9]+ ( "." [0-9]{2} )?
   WEEKDAY     monday | tuesday | wednesday | thursday | friday | saturday | sunday
   STRING      '"' characters '"'  with \" \\ \n \{ escapes and {path} interpolation
   TEXT_BLOCK  '"""' NEWLINE lines '"""'  with the same interpolation
   DOC         "///" text to end of line, including the NEWLINE
   Comments    "#" to end of line, discarded
*)

```

## Appendix B — syntax rules from `spec/vran-0.1.md`

Copied unchanged from commit `699ec980`. Section numbers are the source document's. Static-rule, execution, event, and failure subsections of each step are omitted; the syntax block for that step is kept. Sections 10 through 12 and 14 through 15 of the source are not in this appendix.

## 3. Lexical structure

### 3.1 Source text

A source file is UTF-8 text with LF line endings. A checker MUST accept CRLF and MAY warn. A byte order mark is not allowed (V0006).

### 3.2 Lines and indentation

Vran is line-oriented. Each declaration item, trigger item, step and step clause occupies its own line. Blocks are opened by a line ending in a construct that takes a block, and closed by returning to the enclosing indentation.

- Indentation MUST use spaces. A tab anywhere in leading whitespace is an error (V0003).
- Each level is exactly two spaces deeper than its parent (V0004).
- The lexer produces NEWLINE, INDENT and DEDENT tokens the way Python does.
- Blank lines and comment-only lines produce no tokens.
- Inside `( )`, `[ ]` and `{ }`, line breaks and indentation are ignored. There is no other form of line continuation.

### 3.3 Comments and doc comments

- `#` starts a comment that runs to the end of the line. Comments are discarded.
- `///` starts a doc comment. Consecutive doc comment lines attach to the declaration, enum member, field or step that follows. Doc comments are kept: tools show them, and the interpreter records a step's doc comment in its events.

### 3.4 Names and keywords

- `NAME` is `[a-z_][a-z0-9_]*`. Workflows, steps, fields, enum members, inputs, registers and plugin aliases use names. By convention they are `snake_case`.
- `TYPE_NAME` is `[A-Z][A-Za-z0-9]*`. Types and enums use type names.

**Hard keywords** cannot be used as names anywhere:

```
vran use as input enum type register workflow on
read ask let find check match each fallback signoff upsert do stop fail skip
when else in is not and or missing present true false
event inputs today now attempt
```

All other words that appear in quotes in the grammar are **contextual**, including the primitive type names and `max`. They are keywords only in the position the grammar shows them. For example, `text` is a type in `title text`, and a parameter name in `text "New RFI"`. `max` bounds a list in `list[text max 10]`, and is a function in `math.max(a, b)`. `from` introduces a read's operation in `read spec from files.get`, and is a parameter name in `date.days_between(from date, to date)`.

A plugin parameter MUST NOT be named `key`, `retry`, `unchecked` or `unattended` (see [`plugins.md`](plugins.md), section 5).

### 3.5 Literals

| Literal | Form | Examples | Type |
|---------|------|----------|------|
| String | `"..."` with interpolation (section 8.5) | `"RFI {rfi.number}"` | `text` |
| Text block | `"""`, a newline, lines, then `"""` on its own line | prompts, email bodies | `text` |
| Integer | optional `-`, digits | `7`, `-3` | `integer` |
| Number | optional `-`, digits, `.`, digits | `2.5` | `number` |
| Percent | number or integer followed by `%` | `80%`, `12.5%` | `percent` |
| Duration | integer followed by `m`, `h`, `d` or `w` | `30m`, `4h`, `3d`, `2w` | `duration` |
| Date | `YYYY-MM-DD` | `2026-10-25` | `date` |
| Time of day | `HH:MM`, 24-hour | `08:00` | used only in schedules |
| Money | ISO 4217 code, one space, amount | `USD 12500.00` | `money` |
| Boolean | `true`, `false` | | `boolean` |

- **Strings.** The escapes are `\"`, `\\`, `\n` and `\{`. Any other backslash sequence is an error (V0006). A `}` that doesn't close an interpolation is an ordinary character, so a regular expression like `"SUB-[0-9]\{3,}"` needs only the opening brace escaped.
- **Text blocks.** The common leading indentation of the content lines is removed, and so is the final newline. Text blocks interpolate like strings.
- **Durations** have a single unit. Months and years are not durations, because their length varies.
- **Money** is exact. An interpreter MUST store amounts as integer minor units and MUST NOT convert them to floating point. Arithmetic on money comes from plugins (for example `math.sum_money`).
- **Datetimes** have no literal form. They come from `now`, events and plugin results.

### 4.1 Kinds of file

- A **source file** (`.vran`) holds `use` lines and declarations: inputs, enums, types, registers and workflows.
- An **interface file** (`.vrani`) describes a plugin: its operations, pure functions and types. Interface files are specified in [`plugins.md`](plugins.md).

Every file starts with a header naming the language version:

```vran
vran 0.1
```

A missing or malformed header is V0001. A version the checker does not support is V0002.

### 4.2 `use`

```ebnf
use_decl = "use" ( STRING | PACKAGE "@" INTEGER [ "as" NAME ] ) NEWLINE ;
```

- `use "./project.vran"` imports a **local module**: another source file, by path relative to the importing file. Its inputs, enums, types and registers become visible under their own names. Its workflows are not imported. Its own `use` lines are not re-exported.
- `use vran/email@1` imports a **package** (a plugin) at major version 1. Its operations, functions and types are referenced through the alias: `email.send`, `email.Message`. The default alias is the last path segment, with `-` replaced by `_`.
- `use acme/gc-portal@0 as gc_portal` sets the alias explicitly.

All `use` lines come before the first declaration. Rules:

| Rule | Code |
|------|------|
| A package without a major version (`use vran/email`) | V0104 |
| Two imports with the same alias, or a name defined twice | V0105 |
| A cycle between local modules | V0106 |
| A package missing from `vran.lock`, or whose interface hash doesn't match | V0107 |
| A `use` major version that differs from the locked version | V0108 |

Imports are per file. A workflow that calls `date.add_days` MUST `use vran/date@1` itself, even if a module it imports also uses `vran/date`.

### 6.1 Inputs

```vran
input project_code text
input rfi_response_days integer default 7
```

Inputs are deployment configuration. Their values are fixed when a deployment is created. Every run records them and uses the same values to the end (section 11.2). They are read as `inputs.<name>`.

### 6.2 Registers

A register is typed, keyed, shared state: the RFI log, the submittal register, the master document list. Registers are declared in source files, usually a shared project module, and every workflow that imports the module sees the same rows.

```vran
register rfi_register
  of Rfi
  key number
  sequence "RFI-{inputs.project_code}-{0000}"
  stored in sharepoint.list
    list "RFI Log"
  person owns status, response
  derived days_overdue = date.days_between(due_date, today)
```

| Item | Meaning | Rules |
|------|---------|-------|
| `of T` | Row type | Required. `T` is a record type. |
| `key f` | Key field | Required. A non-optional `text`, `integer` or enum field of `T` (V0326). Unique within the register. |
| `sequence "…"` | Key generator | Optional. On insert, the interpreter assigns the key. `{0000}` is a counter, zero-padded to the number of zeros; counters increase and are never reused. `{inputs.x}` interpolates inputs; nothing else may be interpolated. |
| `stored in op` | Storage backend | Required. A `storage` operation from a plugin, with its parameters as indented arguments. |
| `readonly` | No writes | Workflows may not `upsert` into it (V0321). Required when the storage operation is declared `readonly` (V0325). |
| `person owns f, …` | Field ownership | Fields people maintain. A workflow may set them when inserting a row, never when updating one (V0320). |
| `derived f = value` | Computed field | Evaluated on every read from row fields, `inputs` and `today`, using pure functions. Not settable (V0323). |

### 6.3 Workflows

```ebnf
workflow_decl = "workflow" NAME NEWLINE INDENT
                  [ "timezone" value NEWLINE ]
                  trigger
                  step { step }
                DEDENT ;
```

A workflow has an optional `timezone`, exactly one trigger, and at least one step. The time zone is an IANA name, and determines `today` (section 11.3). A schedule trigger's own `in` zone takes precedence.

## 7. Triggers

### 7.1 Schedules

```vran
on schedule every weekday at 08:00 in inputs.timezone
on schedule every friday at 07:00 in "Europe/London"
on schedule every month on day 20 at 07:00 in inputs.timezone
  catch up 4d
on schedule cron "0 7 * * 1-5" in "America/New_York"
```

- **Recurrences:**
  - `day`: every day.
  - `weekday`: Monday to Friday.
  - A named weekday: that day each week.
  - `month on day N`: day `N` of each month, where `N` is 1 to 28 (V0407).
  - `cron`: a standard five-field expression.
- **Time zone.** Required. Local times follow the zone's daylight-saving rules. A local time that doesn't exist runs at the next valid instant; a time that occurs twice runs once, at the first occurrence.
- **Overlap.** If a run started by an earlier firing of the same schedule is still active (running or waiting), the new firing is skipped and logged.
- **Catch-up.** `catch up D` runs a firing that was missed while the interpreter was down, if it was missed less than `D` ago. Only the most recent missed firing runs. Without `catch up`, missed firings are logged and skipped. `D` is at most 30 days (V0406).
- **Event.** `event` has the fields `scheduled_for datetime`, `fired_at datetime` and `late boolean`.

### 7.2 Manual

`on manual` starts a run when a person asks for one through tooling. `event` has `started_by text` and `started_at datetime`.

### 7.3 Plugin triggers

```vran
on form.submitted
  form "rfi-intake"
  shape RfiForm
```

- The operation MUST be a `trigger` operation of an imported plugin (V0203). The indented lines are its parameters.
- `event` has the operation's `returns` type.
- **Deduplication.** Each trigger operation names an identity field. At most one run starts per identity value per workflow, whatever the plugin delivers. `once per path` replaces the identity with another path into `event`.
- **Shape.** A parameter of type `schema` takes a record type name, as in `shape RfiForm`. The operation's `shape` clause says which field of `event` gets that type (see [`plugins.md`](plugins.md), section 4.3). The plugin validates each payload against the type. An invalid payload starts no run and is logged as R016.

### 8.1 Values

```ebnf
value = literal | path | call | list_value | record_value ;
```

- A **call** invokes a `pure` function of an imported plugin: `date.add_days(today, inputs.rfi_response_days)`. Arguments are values, and calls may nest. Calling anything that is not a pure function is V0204.
- A **list value**, `[a, b, c]`, has the common element type and a bound equal to its length.
- A **record value**, `{ number: current.number, revision: current.revision }`, is anonymous. It is allowed only where the expected type is `any`, such as the `data` parameter of `files.render_template` (V0201).

There are no operators. Arithmetic, text manipulation and aggregation are pure functions: `math.increment(x)`, `text.concat(a, b)`, `table.count(rows)`.

### 8.4 Conditions

```ebnf
comparison = "(" condition ")"
           | operand ( "is" ( "missing" | "present" | [ "not" ] operand )
                     | compare_op operand
                     | [ "not" ] "in" list_value ) ;
```

| Form | Meaning | Operand types |
|------|---------|---------------|
| `a is b`, `a is not b` | equality | any matching types |
| `a < b`, `<=`, `>`, `>=` | ordering | integer, number, percent, money (same currency), duration, date, datetime |
| `a in [x, y]`, `a not in [...]` | membership | element type matches `a` |
| `a is missing`, `a is present` | presence | optional values |

- Conditions combine with `and`, `or` and `not`. Mixing `and` and `or` in one condition without parentheses is V0011.
- Comparing money in different currencies fails the step (R010).

**Missing values.** A comparison other than `is missing` or `is present` whose operand is missing is *unknown*. `not unknown` is unknown. In `and`, one false operand makes the result false, and otherwise any unknown operand makes it unknown. In `or`, one true operand makes the result true, and otherwise any unknown operand makes it unknown. What unknown means depends on the step:

| Where | Unknown means |
|-------|---------------|
| `check … that` | The check fails (fail-closed) |
| `where` in `find` | The row does not match |
| `accept when` in `fallback` | The attempt is not accepted |

### 8.5 Interpolation

Strings and text blocks interpolate `{path}`: `"RFI {rfi.number} due {rfi.due_date}"`.

- Only paths may be interpolated, not calls. Bind a computed value with `let` first.
- Values render as follows:
  - Text as-is.
  - Numbers in plain decimal.
  - Percent with `%`.
  - Money with its currency code.
  - Dates as `YYYY-MM-DD`.
  - Datetimes in ISO 8601 with offset, in the workflow's time zone.
  - Enums by member name.
  - Lists as comma-separated elements.
- A missing value renders as the empty string.
- Records and files can't be interpolated (V0201).
- `\{` produces a literal brace.

### 9.1 `read`

**Syntax.**
```vran
read sov from gc_portal.schedule_of_values
  contract inputs.contract_id
  period draw_period
  retry 3
```
### 9.2 `ask`

**Syntax.**
```vran
ask review returns SubmittalReview
  from package_text.text, spec_text.text, current.description
  cite spec_text.text
  using careful
  retry 2
  prompt """
    Review this submittal against its specification section.
    Cite the clause for every requirement you rely on.
    """
  else
    fail "The model could not produce a valid review"
```
### 9.3 `let`

**Syntax.** `let due_date = date.add_days(today, inputs.rfi_response_days)`
### 9.4 `find`

**Syntax.**
```vran
find overdue in rfi_register
  where status is open and days_overdue >= 1
  order by days_overdue desc
  max 500

find one owner in discipline_owners
  where discipline is routed_discipline
```
### 9.5 `check`

**Syntax.**
```vran
check waiver_ready
  that waiver is present
  that waiver.status is notarized
  reason "No notarized lien waiver on file for {draw_period}"
  else
    do email.send
      to inputs.approver
      subject "Pay application {draw_period} is blocked"
      body "The lien waiver is missing or not notarized."
      key "{inputs.contract_id}:{draw_period}:waiver-missing"
```
### 9.6 `match`

**Syntax.**
```vran
match rfi.days_overdue
  when >= 14
    ...
  when >= 7
    ...
  else
    ...

match pm_review.decision
  when revise_and_resubmit, rejected
    ...
  else
    ...

match event.fields.discipline
  when missing
    ...
  else
    ...
```
### 9.7 `each`

**Syntax.**
```vran
each rfi in overdue max 500
  ...
```
### 9.8 `fallback`

**Syntax.**
```vran
fallback package_text
  try read from pdf.text
    file package
  try read from pdf.ocr
    file package
  accept when attempt.characters >= 500
  else
    fail "Could not read text from the submittal package"
```
### 9.9 `signoff`

**Syntax.**
```vran
signoff pm_review
  by inputs.pm
  via teams.approval
  message "{current.number}: the model recommends {review.recommendation}."
  show review.summary, review.reasons, review.citations
  decide SubmittalDecision
  within 3d
  when declined
    stop "{inputs.pm} is handling {current.number} outside the workflow"
  when timed_out
    fail "No PM decision within 3 days"
```
### 9.10 `upsert`

**Syntax.**
```vran
upsert rfi_register as rfi
  set title event.fields.title
  set status open
  set raised_on today
  key "{event.id}:rfi"

upsert submittal_register
  where number is current.number
  set status with_sub
  key "{current.number}:{current.revision}:with-sub"
```
### 9.11 `do`

**Syntax.**
```vran
do gc_portal.submit_pay_application as filing
  draft draft.id
  key "{inputs.contract_id}:{draw_period}:submit"
  retry 3
```

### 9.12 `stop`, `fail`, `skip`

| Step | Effect | Where allowed |
|------|--------|---------------|
| `stop` | Ends the run. Without a reason the outcome is `done`; with a reason it is `stopped`. | Anywhere |
| `fail "reason"` | Ends the run as `failed`. Inside `each`, it ends the item as `failed`. | Anywhere |
| `skip "reason"` | Ends the current item as `skipped`. | Only inside `each` (V0336) |

A step after `stop`, `fail` or `skip` in the same block is unreachable (V0337).

### 9.13 The `retry` clause

`retry N` appears on `read`, `ask` and `do`. It repeats the same operation after a failure: an error for `read` and `do`, an invalid answer for `ask`. `N` is 0 to 10. The backoff is implementation-defined, but MUST be bounded: the total wait for one step is at most 15 minutes. For effects without native idempotency, only failures the plugin reports as definite are retried (section 11.4).

## 13. Formatting

There is one canonical layout. `vran fmt` produces it, and formatting is idempotent.

- The header comes first, then a blank line, then `use` lines: local modules first, then packages, each group sorted alphabetically. Then a blank line.
- One blank line between top-level declarations.
- One blank line between the trigger and the first step. Between steps, the formatter keeps at most one blank line, as written, so related steps such as a run of `let` lines can stay together. No blank lines between the clauses of a step, or in `type`, `enum` and `register` bodies.
- Two spaces per indentation level.
- **Clause order** within a step:

  | Step | Order |
  |------|-------|
  | Trigger | arguments in interface order, `once per`, `catch up` |
  | `read`, `do` | arguments in interface order, then `key`, `retry`, `unchecked`, `unattended` |
  | `ask` | `from`, `cite`, `using`, `retry`, `prompt`, `else` |
  | `find` | `where`, `order by`, `max` |
  | `check` | `that` lines, `reason`, `else` |
  | `signoff` | `by`, `via`, `message`, `show`, `decide`, `within`, `when declined`, `when timed_out` |
  | `upsert` | `where`, `set` lines, `key`, `unchecked` |
  | `register` | `of`, `key`, `sequence`, `stored in`, `readonly`, `person owns`, `derived` |

- In list values and record values: a single space after each comma, none inside brackets, and one space after each `:` in records.
- Text blocks keep their content as written, indented one level deeper than the clause.
