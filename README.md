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

`Ctx` includes a built-in LLM client that targets Ollama by default. No API key needed for local usage.

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
| `AGENT_LINE_LLM_URL` | `http://localhost:11434` | LLM API base URL |
| `AGENT_LINE_MODEL` | `llama3.1:8b` | Model name |
| `AGENT_LINE_NUM_CTX` | `4096` | Context window size |
| `AGENT_LINE_API_KEY` | (none) | API key (optional, for remote providers) |
| `AGENT_LINE_DEBUG` | (unset) | Set to any value to log LLM requests/responses to stderr |

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
| `list_dir` | `(path: &str) -> Result<Vec<String>, StepError>` | List directory entries |
| `find_files` | `(path: &str, pattern: &str) -> Result<Vec<String>, StepError>` | Recursively find files by pattern |

### Command execution

| Function | Signature | Description |
|----------|-----------|-------------|
| `run_cmd` | `(cmd: &str) -> Result<CmdOutput, StepError>` | Run a shell command |

`CmdOutput` has `success: bool`, `stdout: String`, and `stderr: String`.

### HTTP

| Function | Signature | Description |
|----------|-----------|-------------|
| `http_get` | `(url: &str) -> Result<String, StepError>` | GET request, returns body as string |

### Parsing

| Function | Signature | Description |
|----------|-----------|-------------|
| `strip_code_fences` | `(response: &str) -> String` | Remove markdown code fences from LLM output |

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

## Examples

| Example | Run | Description |
|---------|-----|-------------|
| hello_world | `cargo run --example hello_world` | Single agent, no workflow |
| workflow | `cargo run --example workflow` | Linear workflow with chained agents |
| edit_loop | `cargo run --example edit_loop` | Validate/fix loop with retry |
| newsletter | `cargo run --example newsletter` | Multi-phase LLM workflow (needs Ollama) |
| coder | `cargo run --example coder` | Code generation with test loop (needs Ollama) |

## TODO

- [ ] Rename `find_files` to `glob` or add proper glob pattern support
- [ ] Runner hooks/callbacks for observability (`on_step`, `on_error`)
- [ ] Built-in tracing beyond `AGENT_LINE_DEBUG`
- [ ] Parallel agent execution (fan-out/fan-in with threads)
- [ ] More LLM providers (OpenAI, Anthropic) without proxy
- [ ] `http_post` tool
- [ ] Response parsing helpers (structured output from LLMs)

## Dependencies

- [ureq](https://crates.io/crates/ureq) - Sync HTTP client
- [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) - JSON serialization
