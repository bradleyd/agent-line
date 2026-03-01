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
    let mut agent = AddOne;
    let (state, _) = agent.run(State { n: 1 }, &mut ctx).unwrap();
    println!("n = {}", state.n); // n = 2
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

`Ctx` includes a built-in LLM client that supports Ollama, OpenAI-compatible APIs (OpenRouter, etc.), and the Anthropic API.

```rust
let response = ctx.llm()
    .system("You are a helpful assistant.")
    .user("Summarize this text: ...")
    .send()?;
```

### Configuration

Set via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_LINE_PROVIDER` | `ollama` | LLM provider: `ollama`, `openai`, or `anthropic` |
| `AGENT_LINE_LLM_URL` | `http://localhost:11434` | LLM API base URL |
| `AGENT_LINE_MODEL` | `llama3.1:8b` | Model name |
| `AGENT_LINE_NUM_CTX` | `4096` | Context window size |
| `AGENT_LINE_API_KEY` | (none) | API key (required for remote providers) |
| `AGENT_LINE_DEBUG` | (unset) | Set to any value to log config at startup and LLM requests/responses to stderr |

### Provider examples

**Ollama (default, no API key needed):**
```sh
export AGENT_LINE_MODEL=llama3.1:8b
```

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
| coder | `cargo run --example coder` | Code generation with test loop (needs Ollama) |
| assistant | `cargo run --example assistant` | Personal assistant pipeline with tracing (needs Ollama) |
| parallel | `cargo run --example parallel` | Threaded fan-out/fan-in with researcher/writer/editor pipeline |

## TODO

- [ ] Rename `find_files` to `glob` or add proper glob pattern support

## Dependencies

- [ureq](https://crates.io/crates/ureq) - Sync HTTP client
- [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) - JSON serialization
