use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};

#[derive(Clone, Debug)]
struct Doc {
    text: String,
    revision: u32,
}

struct Writer;
impl Agent<Doc> for Writer {
    fn name(&self) -> &'static str {
        "writer"
    }
    fn run(&mut self, mut state: Doc, ctx: &mut Ctx) -> StepResult<Doc> {
        state.revision += 1;
        ctx.log(format!("writer: producing revision {}", state.revision));

        // Simulate: first draft has typos, second is clean.
        if state.revision == 1 {
            state.text = "Hello wrold! This is a dcument.".to_string();
        } else {
            state.text = "Hello world! This is a document.".to_string();
        }

        Ok((state, Outcome::Continue))
    }
}

struct Validator;
impl Agent<Doc> for Validator {
    fn name(&self) -> &'static str {
        "validator"
    }
    fn run(&mut self, state: Doc, ctx: &mut Ctx) -> StepResult<Doc> {
        let mut errors = Vec::new();

        if state.text.contains("wrold") {
            errors.push("typo: 'wrold' should be 'world'");
        }
        if state.text.contains("dcument") {
            errors.push("typo: 'dcument' should be 'document'");
        }

        if errors.is_empty() {
            ctx.log("validator: all checks passed");
            Ok((state, Outcome::Done))
        } else {
            for e in &errors {
                ctx.log(format!("validator: {e}"));
            }
            Ok((state, Outcome::Next("fixer")))
        }
    }
}

struct Fixer;
impl Agent<Doc> for Fixer {
    fn name(&self) -> &'static str {
        "fixer"
    }
    fn run(&mut self, mut state: Doc, ctx: &mut Ctx) -> StepResult<Doc> {
        // Collect logs first, then clear. Reading logs() borrows &self,
        // but we need &mut self to clear — so we clone into a Vec first.
        let entries: Vec<String> = ctx.logs().to_vec();
        ctx.clear_logs();

        for entry in &entries {
            if entry.contains("wrold") {
                state.text = state.text.replace("wrold", "world");
                ctx.log("fixer: corrected 'wrold' -> 'world'");
            }
            if entry.contains("dcument") {
                state.text = state.text.replace("dcument", "document");
                ctx.log("fixer: corrected 'dcument' -> 'document'");
            }
        }

        Ok((state, Outcome::Next("validator")))
    }
}

fn main() {
    let mut ctx = Ctx::new();

    let mut runner = Runner::new(
        Workflow::builder("edit-loop")
            .register(Writer)
            .register(Validator)
            .register(Fixer)
            .start_at("writer")
            .then("validator")
            .build()
            .unwrap(),
    );

    for round in 1..=3 {
        println!("=== Round {round} ===");

        let doc = Doc {
            text: String::new(),
            revision: 0,
        };

        match runner.run(doc, &mut ctx) {
            Ok(doc) => {
                println!("  Final text: {:?}", doc.text);
                println!("  Revisions:  {}", doc.revision);
            }
            Err(e) => println!("  Error: {e}"),
        }

        println!("  Log:");
        for entry in ctx.logs() {
            println!("    {entry}");
        }
        ctx.clear_logs();
        println!();
    }
}
