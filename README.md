# agent-line

A batteries-included Rust library for building agent workflows. Sync-only, opinionated, and designed for people getting started with agent patterns.

Define agents, wire them into workflows, and let the runner execute them. Agents communicate through shared context and control flow with outcomes like `Continue`, `Next`, `Retry`, and `Done`.

## Quick Start

```rust
use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};

#[derive(Clone)]
struct State { n: i32 }

struct AddOne;
impl Agent<State> for AddOne {
    fn name(&self) -> &'static str { "add_one" }
    fn run(&mut self, state: State, _ctx: &mut Ctx) -> StepResult<State> {
        Ok((State { n: state.n + 1 }, Outcome::Done))
    }
}

fn main() {
    let mut ctx = Ctx::new();
    let wf = Workflow::builder("demo")
        .register(AddOne)
        .build()
        .unwrap();

    let result = Runner::new(wf).run(State { n: 0 }, &mut ctx).unwrap();
    println!("n = {}", result.n); // n = 1
}
```

## Workflows

Agents are registered into a workflow, then wired together with `start_at` and `then`. The workflow validates everything at build time.

```rust
let wf = Workflow::builder("my-workflow")
    .register(StepA)
    .register(StepB)
    .register(StepC)
    .start_at("step_a")
    .then("step_b")
    .then("step_c")
    .build()
    .unwrap();

let mut runner = Runner::new(wf);
let result = runner.run(initial_state, &mut ctx);
```

Agents can also route dynamically by returning `Outcome::Next("agent_name")` instead of `Outcome::Continue`.

## Context (Ctx)

`Ctx` is shared mutable state passed to every agent. It provides a key-value store and an event log.

```rust
let mut ctx = Ctx::new();

// Key-value store
ctx.set("draft", "Hello world");
let val = ctx.get("draft"); // Some("Hello world")
ctx.remove("draft");

// Event log
ctx.log("validator: found 2 errors");
for entry in ctx.logs() {
    println!("{entry}");
}
ctx.clear_logs();

// Reset everything
ctx.clear();
```

`Ctx` persists across multiple `runner.run()` calls, so the log and store accumulate across runs.

## LLM Integration

Agents that need an LLM hold their own `LlmConfig` and call `LlmConfig::request()` to start a chat request. Supports Ollama, OpenAI-compatible APIs (OpenRouter, etc.), and the Anthropic API.

```rust
use agent_line::{Agent, Ctx, LlmConfig, Outcome, StepResult};

#[derive(Clone)]
struct State {
    text: String,
    summary: String,
}

struct Summarize {
    llm: LlmConfig,
}

impl Summarize {
    fn new(llm: LlmConfig) -> Self { Self { llm } }
}

impl Agent<State> for Summarize {
    fn name(&self) -> &'static str { "summarize" }
    fn run(&mut self, mut state: State, _ctx: &mut Ctx) -> StepResult<State> {
        state.summary = self.llm.request()
            .system("Summarize the input in one sentence.")
            .user(&state.text)
            .send()?;
        Ok((state, Outcome::Done))
    }
}
```

In `main`, build a config and inject it into the agent:

```rust
let llm = LlmConfig::from_env();   // reads AGENT_LINE_* env vars

let wf = Workflow::builder("summarize")
    .register(Summarize::new(llm))
    .build()?;
```

### Configuration

`LlmConfig::from_env()` reads:

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_LINE_PROVIDER` | `ollama` | LLM provider: `ollama`, `openai`, or `anthropic` |
| `AGENT_LINE_LLM_URL` | `http://localhost:11434` | LLM API base URL |
| `AGENT_LINE_MODEL` | `llama3.1:8b` | Model name |
| `AGENT_LINE_NUM_CTX` | `4096` | Ollama context window size (`options.num_ctx`) |
| `AGENT_LINE_MAX_TOKENS` | value of `AGENT_LINE_NUM_CTX` | OpenAI/Anthropic `max_tokens` cap on the response |
| `AGENT_LINE_API_KEY` | (none) | API key (required for remote providers) |
| `AGENT_LINE_DEBUG` | (unset) | Set to any value to log the resolved config and LLM requests/responses to stderr |

For explicit configuration without environment variables, use `LlmConfig::builder()` instead.

### Provider examples

**Ollama (default, no API key needed):**
```sh
export AGENT_LINE_MODEL=llama3.1:8b
```

Requests to Ollama send `"think": false` so thinking-capable models (Qwen 3, etc.) skip the `<think>...</think>` reasoning block before the response. This is the default for latency reasons; thinking can otherwise add minutes per request. Models without thinking support ignore the field.

**OpenRouter:**
```sh
export AGENT_LINE_PROVIDER=openai
export AGENT_LINE_LLM_URL=https://openrouter.ai/api
export AGENT_LINE_MODEL=amazon/nova-lite-v1
export AGENT_LINE_API_KEY=sk-or-...
```

**Anthropic:**
```sh
export AGENT_LINE_PROVIDER=anthropic
export AGENT_LINE_LLM_URL=https://api.anthropic.com
export AGENT_LINE_MODEL=claude-sonnet-4-20250514
export AGENT_LINE_API_KEY=sk-ant-...
```

### Multiple models per workflow

Give each agent its own `LlmConfig`. A cheap local model handles routine extraction; a stronger remote model handles the harder reasoning step:

```rust
use agent_line::{Agent, Ctx, LlmConfig, Outcome, Provider, Runner, StepResult, Workflow};

#[derive(Clone)]
struct Draft {
    body: String,
    notes: String,
    review: String,
}

struct Researcher { llm: LlmConfig }

impl Researcher {
    fn new(llm: LlmConfig) -> Self { Self { llm } }
}

impl Agent<Draft> for Researcher {
    fn name(&self) -> &'static str { "researcher" }
    fn run(&mut self, mut draft: Draft, _ctx: &mut Ctx) -> StepResult<Draft> {
        draft.notes = self.llm.request()
            .system("Extract the three key claims from the draft, one per line.")
            .user(&draft.body)
            .send()?;
        Ok((draft, Outcome::Continue))
    }
}

struct Reviewer { llm: LlmConfig }

impl Reviewer {
    fn new(llm: LlmConfig) -> Self { Self { llm } }
}

impl Agent<Draft> for Reviewer {
    fn name(&self) -> &'static str { "reviewer" }
    fn run(&mut self, mut draft: Draft, _ctx: &mut Ctx) -> StepResult<Draft> {
        draft.review = self.llm.request()
            .system("Critique the draft against its own claims. Be specific.")
            .user(format!("Claims:\n{}\n\nDraft:\n{}", draft.notes, draft.body))
            .send()?;
        Ok((draft, Outcome::Done))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cheap = LlmConfig::builder()
        .provider(Provider::Ollama)
        .base_url("http://localhost:11434")
        .model("qwen3:8b")
        .build()?;

    let strong = LlmConfig::builder()
        .provider(Provider::Anthropic)
        .base_url("https://api.anthropic.com")
        .model("claude-sonnet-4-20250514")
        .api_key(std::env::var("ANTHROPIC_API_KEY")?)
        .max_tokens(1200)
        .build()?;

    let mut ctx = Ctx::new();
    let wf = Workflow::builder("review")
        .register(Researcher::new(cheap))
        .register(Reviewer::new(strong))
        .start_at("researcher")
        .then("reviewer")
        .build()?;

    Runner::new(wf).run(
        Draft {
            body: "Rust ownership lets you pass a value or borrow it...".into(),
            notes: String::new(),
            review: String::new(),
        },
        &mut ctx,
    )?;
    Ok(())
}
```

Required `LlmConfig` fields: `provider`, `base_url`, `model`. Optional: `api_key`, `num_ctx` for Ollama requests, and `max_tokens` for OpenAI-compatible and Anthropic requests. `LlmConfig::build()` returns an error if a required field is missing.

See `examples/multi_model.rs` for a small pipeline and `examples/incident_investigation/` for a multi-file incident correlation example.

## Outcomes

Agents return an `Outcome` to control what happens next:

| Outcome | Behavior |
|---------|----------|
| `Continue` | Follow the default next step set by `.then()` |
| `Done` | Workflow complete, return the final state |
| `Next("name")` | Jump to a specific agent by name |
| `Retry(hint)` | Re-run the current agent (counted against `max_retries`) |
| `Wait(duration)` | Sleep, then re-run the current agent |
| `Fail(msg)` | Stop the workflow with an error |

## Tools

Standalone utility functions for common agent tasks. Import with `use agent_line::tools;`.

### File operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `read_file` | `(path: &str) -> Result<String, StepError>` | Read file contents |
| `write_file` | `(path: &str, content: &str) -> Result<(), StepError>` | Write to file (creates parent dirs) |
| `append_file` | `(path: &str, content: &str) -> Result<(), StepError>` | Append to file (creates if missing) |
| `file_exists` | `(path: &str) -> bool` | Check if a file exists |
| `delete_file` | `(path: &str) -> Result<(), StepError>` | Delete a file |
| `create_dir` | `(path: &str) -> Result<(), StepError>` | Create directory (and parents) |
| `list_dir` | `(path: &str) -> Result<Vec<String>, StepError>` | List directory entries |
| `find_files` | `(path: &str, pattern: &str) -> Result<Vec<String>, StepError>` | Recursively find files by pattern |

### Command execution

| Function | Signature | Description |
|----------|-----------|-------------|
| `run_cmd` | `(cmd: &str) -> Result<CmdOutput, StepError>` | Run a shell command |
| `run_cmd_in_dir` | `(dir: &str, cmd: &str) -> Result<CmdOutput, StepError>` | Run a shell command in a specific directory |

`CmdOutput` has `success: bool`, `stdout: String`, and `stderr: String`.

### HTTP

| Function | Signature | Description |
|----------|-----------|-------------|
| `http_get` | `(url: &str) -> Result<String, StepError>` | GET request, returns body as string |
| `http_post` | `(url: &str, body: &str) -> Result<String, StepError>` | POST with string body |
| `http_post_json` | `(url: &str, body: &Value) -> Result<String, StepError>` | POST with JSON body |

### Parsing

| Function | Signature | Description |
|----------|-----------|-------------|
| `strip_code_fences` | `(response: &str) -> String` | Remove markdown code fences from LLM output |
| `parse_lines` | `(response: &str) -> Vec<String>` | Split LLM response into lines, strip numbering/bullets |
| `extract_json` | `(response: &str) -> Result<String, StepError>` | Extract first JSON object or array from text |

## Error Handling

`StepError` has four variants designed around what the caller can do about them:

| Variant | Meaning | Action |
|---------|---------|--------|
| `Invalid(String)` | Bad input or logic error | Fix the code |
| `Transient(String)` | Network/rate limit failure | Retry might help |
| `Failed(String)` | Agent explicitly failed | Handle or propagate |
| `Other(String)` | Everything else | Inspect the message |

`From` impls exist for `ureq::Error` (maps to `Transient`) and `std::io::Error` (maps to `Other`), so you can use `?` in tool calls.

## Runner Configuration

```rust
let mut runner = Runner::new(wf)
    .with_max_steps(10_000)   // default, prevents infinite loops
    .with_max_retries(3);     // default, per-agent consecutive retry limit
```

## Hooks

Runner supports closure-based hooks for observability. Closures are `FnMut`, so you can use stateful callbacks (counters, accumulators, etc.).

```rust
let mut runner = Runner::new(wf)
    .on_step(|e| {
        println!(
            "[step {}] {} -> {:?} ({:.3}s)",
            e.step_number, e.agent, e.outcome, e.duration.as_secs_f64()
        );
    })
    .on_error(|e| {
        eprintln!("[error] {} at step {}: {}", e.agent, e.step_number, e.error);
    });
```

Or use the built-in tracing shorthand, which prints step transitions and errors to stderr:

```rust
let mut runner = Runner::new(wf).with_tracing();
```

Output looks like:

```
[step 1] fetch_weather -> Continue (0.001s)
[step 2] fetch_calendar -> Continue (0.000s)
[step 3] fetch_email -> Continue (0.000s)
[step 4] summarize -> Done (2.340s)
```

### OpenTelemetry (OTEL) integration

You can export each agent step as OTEL spans by wiring hooks to your own tracer:

```rust
use agent_line::{Runner, Workflow};
use opentelemetry::{global, Context, KeyValue};
use opentelemetry::trace::{Span, TraceContextExt, Tracer};

let mut workflow_span = global::tracer("agent-line").start("workflow.run");
let parent = Context::new().with_remote_span_context(workflow_span.span_context().clone());
let parent_for_step = parent.clone();
let parent_for_error = parent.clone();

let mut runner = Runner::new(wf)
    .on_step(move |e| {
        let tracer = global::tracer("agent-line");
        let mut span = tracer.start_with_context("agent.step", &parent_for_step);
        span.set_attribute(KeyValue::new("agent.name", e.agent.to_string()));
        span.set_attribute(KeyValue::new("step.number", e.step_number as i64));
        span.set_attribute(KeyValue::new("step.retries", e.retries as i64));
        span.set_attribute(KeyValue::new("step.outcome", format!("{:?}", e.outcome)));
        span.set_attribute(KeyValue::new("step.duration_ms", e.duration.as_millis() as i64));
        span.end();
    })
    .on_error(move |e| {
        let tracer = global::tracer("agent-line");
        let mut span = tracer.start_with_context("agent.step.error", &parent_for_error);
        span.set_attribute(KeyValue::new("agent.name", e.agent.to_string()));
        span.set_attribute(KeyValue::new("step.number", e.step_number as i64));
        span.set_attribute(KeyValue::new("error.message", e.error.to_string()));
        span.end();
    });

let _ = runner.run(initial_state, &mut ctx);
workflow_span.end();
```

Full runnable example:

```sh
cargo run --example otel_tracing
```

### Why tracing is hook-based

`agent-line` intentionally does not hardcode an observability backend in the core runner. That design is the most flexible for a library because users can:

- Send events to OTEL, `tracing`, metrics, logs, or custom sinks without adapter friction.
- Avoid extra global initialization and dependency weight when tracing is not needed.
- Keep runtime behavior predictable in embedded, CLI, service, and test environments.

The built-in `with_tracing()` helper remains for quick local debugging, while hooks cover production observability needs.

### Hook event types

`StepEvent` is passed to `on_step` after each successful agent step:

| Field | Type | Description |
|-------|------|-------------|
| `agent` | `&str` | Name of the agent that ran |
| `outcome` | `&Outcome` | The outcome the agent returned |
| `duration` | `Duration` | Wall-clock time for the step |
| `step_number` | `usize` | Sequential step counter (starts at 1) |
| `retries` | `usize` | Consecutive retry count for the current agent |

`ErrorEvent` is passed to `on_error` when an agent errors or a limit is exceeded:

| Field | Type | Description |
|-------|------|-------------|
| `agent` | `&str` | Name of the agent that errored |
| `error` | `&StepError` | The error that occurred |
| `step_number` | `usize` | Step number where the error happened |

## Examples

| Example | Run | Description |
|---------|-----|-------------|
| hello_world | `cargo run --example hello_world` | Single agent, no workflow |
| workflow | `cargo run --example workflow` | Linear workflow with chained agents |
| edit_loop | `cargo run --example edit_loop` | Validate/fix loop with retry |
| newsletter | `cargo run --example newsletter` | Multi-phase LLM workflow (needs Ollama) |
| multi_model | `cargo run --example multi_model` | Pipeline with different models per agent: cheap step uses local Ollama (`qwen3:8b`), strong step uses Anthropic (needs `ANTHROPIC_API_KEY`) |
| incident_investigation | `cargo run --example incident_investigation` | Multi-file incident correlation workflow with a fast small Ollama model for triage and a heavier Ollama model for the report. `main.rs` shows commented-out OpenRouter and Anthropic alternatives |
| coder | `cargo run --example coder` | Code generation with test loop (needs Ollama) |
| assistant | `cargo run --example assistant` | Personal assistant pipeline with tracing (needs Ollama) |
| otel_tracing | `cargo run --example otel_tracing` | OTEL span export from `on_step`/`on_error` hooks |
| parallel | `cargo run --example parallel` | Threaded fan-out/fan-in with researcher/writer/editor pipeline |

## TODO

- [ ] Rename `find_files` to `glob` or add proper glob pattern support
- [ ] Better LLM error output. Today a non-2xx response surfaces as `transient: llm request failed: http status: 404` with no body. Read the response body and surface the underlying message (e.g. Ollama's "model X not found") so users can act on it.
- [ ] Expose Ollama thinking mode as an opt-in. The library currently hardcodes `"think": false` for the Ollama provider so thinking models (Qwen 3, etc.) skip the `<think>` block by default. Add a way to re-enable it (likely a method on `LlmConfigBuilder`) for users who want the quality bump on hard reasoning tasks and can wait.

## Dependencies

- [ureq](https://crates.io/crates/ureq) - Sync HTTP client
- [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) - JSON serialization
