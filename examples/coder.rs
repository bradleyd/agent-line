use agent_line::{tools, Agent, Ctx, Outcome, Runner, StepResult, Workflow};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Task {
    description: String,
    file_path: String,
    code: String,
    test_output: String,
    attempts: u32,
    max_attempts: u32,
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

struct Planner;
impl Agent<Task> for Planner {
    fn name(&self) -> &'static str {
        "planner"
    }
    fn run(&mut self, mut state: Task, ctx: &mut Ctx) -> StepResult<Task> {
        state.code = tools::read_file(&state.file_path).unwrap_or_default();

        let plan = ctx
            .llm()
            .system(
                "You are a senior developer. Create a brief implementation plan. \
                 List the specific changes needed. Be concise. \
                 Do not include doc comments or doc tests.",
            )
            .user(format!(
                "Task: {}\n\nFile: {}\n\nCurrent code:\n{}",
                state.description,
                state.file_path,
                if state.code.is_empty() {
                    "(new file)".to_string()
                } else {
                    state.code.clone()
                }
            ))
            .send()?;

        ctx.set("plan", &plan);
        ctx.log(format!("planner: created plan for {}", state.file_path));
        Ok((state, Outcome::Continue))
    }
}

struct Coder;
impl Agent<Task> for Coder {
    fn name(&self) -> &'static str {
        "coder"
    }
    fn run(&mut self, mut state: Task, ctx: &mut Ctx) -> StepResult<Task> {
        let plan = ctx.get("plan").unwrap_or("no plan found").to_string();

        let response = ctx
            .llm()
            .system(
                "You are a developer. Write the code based on the plan. \
                 Return ONLY the complete file contents, no explanation. \
                 Do not include doc comments or doc tests. \
                 Do not wrap the output in markdown code fences.",
            )
            .user(format!(
                "Plan:\n{plan}\n\nFile: {}\n\nCurrent code:\n{}",
                state.file_path, state.code
            ))
            .send()?;

        state.code = tools::strip_code_fences(&response);
        tools::write_file(&state.file_path, &state.code)?;
        ctx.log("coder: wrote code to file");
        Ok((state, Outcome::Continue))
    }
}

struct TestRunner;
impl Agent<Task> for TestRunner {
    fn name(&self) -> &'static str {
        "test_runner"
    }
    fn run(&mut self, mut state: Task, ctx: &mut Ctx) -> StepResult<Task> {
        let manifest = ctx.get("manifest_path").unwrap_or("Cargo.toml").to_string();
        let result = tools::run_cmd(&format!("cargo test --manifest-path {manifest} --lib"))?;

        if result.success {
            ctx.log("tests: all passed");
            Ok((state, Outcome::Done))
        } else {
            state.test_output = result.stderr;
            state.attempts += 1;
            ctx.log(format!("tests: failed (attempt {})", state.attempts));

            if state.attempts >= state.max_attempts {
                Ok((
                    state,
                    Outcome::Fail("max fix attempts reached, tests still failing".into()),
                ))
            } else {
                Ok((state, Outcome::Next("fixer")))
            }
        }
    }
}

struct Fixer;
impl Agent<Task> for Fixer {
    fn name(&self) -> &'static str {
        "fixer"
    }
    fn run(&mut self, mut state: Task, ctx: &mut Ctx) -> StepResult<Task> {
        let response = ctx
            .llm()
            .system(
                "You are a debugger. Fix the code based on the test failures. \
                 Return ONLY the complete fixed file contents, no explanation. \
                 Do not include doc comments or doc tests. \
                 Do not wrap the output in markdown code fences.",
            )
            .user(format!(
                "Test errors:\n{}\n\nCurrent code:\n{}",
                state.test_output, state.code
            ))
            .send()?;

        state.code = tools::strip_code_fences(&response);
        tools::write_file(&state.file_path, &state.code)?;
        ctx.log("fixer: wrote fix to file");
        Ok((state, Outcome::Next("test_runner")))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn scaffold_project(dir: &std::path::Path) {
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"scratch\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(src.join("lib.rs"), "").unwrap();
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

fn main() {
    let tmp = std::env::temp_dir().join("agent-line-coder");
    scaffold_project(&tmp);

    let lib_path = tmp.join("src/lib.rs").display().to_string();
    let manifest = tmp.join("Cargo.toml").display().to_string();

    let mut ctx = Ctx::new();
    ctx.set("manifest_path", &manifest);

    let wf = Workflow::builder("coding-agent")
        .register(Planner)
        .register(Coder)
        .register(TestRunner)
        .register(Fixer)
        .start_at("planner")
        .then("coder")
        .then("test_runner")
        .build()
        .unwrap();

    let mut runner = Runner::new(wf);

    let result = runner.run(
        Task {
            description: "Add a function called `reverse_string` that reverses a string and add unit tests".into(),
            file_path: lib_path,
            code: String::new(),
            test_output: String::new(),
            attempts: 0,
            max_attempts: 3,
        },
        &mut ctx,
    );

    println!("=== Result ===");
    match result {
        Ok(task) => {
            println!("  Success after {} fix attempts", task.attempts);
            println!("  Final code:\n{}", task.code);
        }
        Err(e) => println!("  Failed: {e}"),
    }

    println!("\n=== Log ===");
    for entry in ctx.logs() {
        println!("  {entry}");
    }
}
