// Threaded fan-out / fan-in example.
//
// Spawns one thread per topic to write articles in parallel. Each thread gets
// its own Ctx and Runner -- no shared mutable state, no async runtime.
//
// Pipeline per thread: researcher -> writer -> editor -> (loop back to writer if needed) -> done
//
// All data flows through the state struct. Swap the stubs for ctx.llm() calls
// to use a real LLM.
//
// Run: cargo run --example parallel

use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};
use std::thread;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct ArticleState {
    topic: String,
    research: String,
    #[allow(dead_code)] // used by LLM calls in the commented-out real implementations
    guidelines: String,
    draft: String,
    feedback: String,
    revision: u32,
}

impl ArticleState {
    fn new(topic: String, guidelines: String) -> Self {
        Self {
            topic,
            research: String::new(),
            guidelines,
            draft: String::new(),
            feedback: String::new(),
            revision: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

/// Gathers background material on the topic.
/// In a real app this would use tools::http for web search, then ctx.llm()
/// to summarize the findings into research notes.
struct Researcher;
impl Agent<ArticleState> for Researcher {
    fn name(&self) -> &'static str {
        "researcher"
    }
    fn run(&mut self, mut state: ArticleState, ctx: &mut Ctx) -> StepResult<ArticleState> {
        ctx.log(format!("researching: {}", state.topic));

        // Stub: pretend we did a web search and summarized the results
        state.research = match state.topic.as_str() {
            t if t.contains("embedded") => {
                "Rust's ownership model prevents memory bugs common in C firmware. \
                 The embassy framework provides async on bare metal. \
                 Companies like Espressif ship official Rust support for ESP32."
                    .into()
            }
            t if t.contains("plumber") => {
                "Side projects help tradespeople automate billing and scheduling. \
                 Low-code tools like AppSheet let plumbers build apps without coding. \
                 One plumber built a leak-detection IoT sensor with a Raspberry Pi."
                    .into()
            }
            _ => "Raspberry Pi runs Node-RED for home automation wiring diagrams. \
                 Electricians use it to monitor panel loads in real time. \
                 The $35 price point makes it practical for small shops."
                .into(),
        };

        Ok((state, Outcome::Continue))
    }
}

/// Writes (or rewrites) the article draft.
///
/// On the first pass, the system prompt is "write from scratch." When the
/// editor has sent it back with feedback, the system prompt becomes "rewrite
/// incorporating this feedback" so the LLM understands it is revising, not
/// starting over.
struct Writer;
impl Agent<ArticleState> for Writer {
    fn name(&self) -> &'static str {
        "writer"
    }
    fn run(&mut self, mut state: ArticleState, ctx: &mut Ctx) -> StepResult<ArticleState> {
        state.revision += 1;

        let is_rewrite = !state.feedback.is_empty();

        if is_rewrite {
            ctx.log(format!(
                "rewriting draft {} for: {} (feedback: {})",
                state.revision, state.topic, state.feedback
            ));

            // Stub for rewrite pass
            // In a real app:
            // let response = ctx.llm()
            //     .system(&format!(
            //         "You are a writer. Rewrite this article incorporating the editor's feedback.\n\
            //          Guidelines: {}\n\
            //          Feedback: {}",
            //         state.guidelines, state.feedback
            //     ))
            //     .user(&state.draft)
            //     .send()?;
            // state.draft = response;

            state.draft = format!(
                "# {}\n\n\
                 Ever wonder how {} is changing the trades?\n\n\
                 {}",
                state.topic,
                state.topic.to_lowercase(),
                state.research,
            );
            state.feedback.clear();
        } else {
            ctx.log(format!(
                "writing draft {} for: {}",
                state.revision, state.topic
            ));

            // Stub for first draft
            // In a real app:
            // let response = ctx.llm()
            //     .system(&format!(
            //         "You are a writer. Write a short article based on the research notes.\n\
            //          Guidelines: {}",
            //         state.guidelines
            //     ))
            //     .user(&format!("Topic: {}\n\nResearch:\n{}", state.topic, state.research))
            //     .send()?;
            // state.draft = response;

            state.draft = format!(
                "# {}\n\n\
                 {} is a interesting topic that many people are talking about.\n\n\
                 {}",
                state.topic, state.topic, state.research,
            );
        }

        Ok((state, Outcome::Continue))
    }
}

/// Reviews the draft against the writing guidelines and the author's voice.
/// Approves or sends it back to the writer with specific feedback.
struct Editor;
impl Agent<ArticleState> for Editor {
    fn name(&self) -> &'static str {
        "editor"
    }
    fn run(&mut self, mut state: ArticleState, ctx: &mut Ctx) -> StepResult<ArticleState> {
        ctx.log(format!("reviewing rev {}: {}", state.revision, state.topic));

        // Stub: check the draft against guidelines
        // In a real app:
        // let response = ctx.llm()
        //     .system(&format!(
        //         "You are an editor. Review this article against the guidelines.\n\
        //          Guidelines: {}\n\n\
        //          If the article passes, respond with exactly: APPROVED\n\
        //          Otherwise list the specific changes needed.",
        //         state.guidelines
        //     ))
        //     .user(&state.draft)
        //     .send()?;
        //
        // if response.contains("APPROVED") { ... } else { state.feedback = response; }

        // Stub logic: first draft always needs work, second passes
        if state.revision < 2 {
            state.feedback =
                "opening is bland, needs a hook. 'a interesting' should be 'an interesting'".into();
            ctx.log(format!("needs revision: {}", state.feedback));
            Ok((state, Outcome::Next("writer")))
        } else {
            ctx.log(format!("approved: {}", state.topic));
            Ok((state, Outcome::Done))
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let topics = vec![
        "Rust in embedded systems".to_string(),
        "Why plumbers love side projects".to_string(),
        "Electricians using Raspberry Pi on the job".to_string(),
    ];

    // Shared guidelines for all writers -- the author's voice and style rules
    let guidelines = "\
        Write in first person. \
        Do not use emdashes. \
        Add a touch of humor. \
        Keep it under 300 words."
        .to_string();

    println!("=== Fan-out: {} threads ===\n", topics.len());

    // Fan-out: spawn one thread per topic
    let handles: Vec<_> = topics
        .into_iter()
        .enumerate()
        .map(|(i, topic)| {
            let guidelines = guidelines.clone();

            thread::spawn(move || {
                // Each thread gets its own Ctx and Runner -- no shared mutable state
                let mut ctx = Ctx::new();

                let wf = Workflow::builder("write-article")
                    .register(Researcher)
                    .register(Writer)
                    .register(Editor)
                    .start_at("researcher")
                    .then("writer")
                    .then("editor")
                    .build()
                    .unwrap();

                let mut runner = Runner::new(wf).with_max_retries(5);

                let result = runner.run(ArticleState::new(topic.clone(), guidelines), &mut ctx);

                // Print the log from this thread's pipeline
                for entry in ctx.logs() {
                    println!("  [thread {}] {}", i, entry);
                }

                result
            })
        })
        .collect();

    // Fan-in: join all threads, collect results
    let mut finished = Vec::new();
    for handle in handles {
        match handle.join().unwrap() {
            Ok(state) => finished.push(state),
            Err(e) => eprintln!("thread failed: {e}"),
        }
    }

    // Show results
    println!("\n=== Fan-in: {} articles ===\n", finished.len());
    for (i, article) in finished.iter().enumerate() {
        let preview: String = article.draft.chars().take(72).collect();
        println!(
            "  {}. {} (rev {})\n     {preview}...\n",
            i + 1,
            article.topic,
            article.revision
        );
    }
}
