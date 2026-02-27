use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct TopicState {
    query: String,
    topics: Vec<String>,
    selected: Vec<String>,
}

#[derive(Clone, Debug)]
struct ArticleState {
    topic: String,
    draft: String,
    revision: u32,
}

// ---------------------------------------------------------------------------
// Phase 1 agents: find and pick topics
// ---------------------------------------------------------------------------

struct TopicSearcher;
impl Agent<TopicState> for TopicSearcher {
    fn name(&self) -> &'static str {
        "topic_searcher"
    }
    fn run(&mut self, mut state: TopicState, ctx: &mut Ctx) -> StepResult<TopicState> {
        ctx.log(format!("searching for: {}", state.query));

        // Stub: pretend we did a web search
        state.topics = vec![
            "Rust in embedded systems".into(),
            "Why plumbers love side projects".into(),
            "Welding meets software: CNC pipelines".into(),
            "HVAC technicians automating schedules".into(),
            "Electricians using Raspberry Pi on the job".into(),
        ];

        ctx.log(format!("found {} topics", state.topics.len()));
        Ok((state, Outcome::Continue))
    }
}

struct TopicPicker;
impl Agent<TopicState> for TopicPicker {
    fn name(&self) -> &'static str {
        "topic_picker"
    }
    fn run(&mut self, mut state: TopicState, ctx: &mut Ctx) -> StepResult<TopicState> {
        let response = ctx.llm()
            .system("You are a newsletter curator. Pick exactly 3 topics. Return one per line, nothing else.")
            .user(format!("Choose from:\n{}", state.topics.join("\n")))
            .send()?;

        // brittle, but should pick out the 3
        state.selected = response
            .lines()
            .map(|l| l.trim())
            .map(|l| {
                l.trim_start_matches(|c: char| c.is_numeric() || c == '.' || c == '-' || c == ' ')
            })
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        for topic in &state.selected {
            ctx.log(format!("selected: {topic}"));
        }

        Ok((state, Outcome::Done))
    }
}

// ---------------------------------------------------------------------------
// Phase 2 agents: write, validate, fix one article
// ---------------------------------------------------------------------------

struct ArticleWriter;
impl Agent<ArticleState> for ArticleWriter {
    fn name(&self) -> &'static str {
        "article_writer"
    }
    fn run(&mut self, mut state: ArticleState, ctx: &mut Ctx) -> StepResult<ArticleState> {
        state.revision += 1;
        ctx.log(format!(
            "writing draft {} for: {}",
            state.revision, state.topic
        ));

        // Stub: first draft is sloppy, second is clean
        if state.revision == 1 {
            state.draft = format!(
                "# {}\n\nThis is a artcle about {}. It has lots of good infomation.",
                state.topic, state.topic
            );
        } else {
            state.draft = format!(
                "# {}\n\nThis is an article about {}. It has lots of good information.",
                state.topic, state.topic
            );
        }

        Ok((state, Outcome::Continue))
    }
}

struct ArticleValidator;
impl Agent<ArticleState> for ArticleValidator {
    fn name(&self) -> &'static str {
        "article_validator"
    }
    fn run(&mut self, state: ArticleState, ctx: &mut Ctx) -> StepResult<ArticleState> {
        // use store k/v to pull in validation rules and pass them to the llm
        let response = ctx
            .llm()
            .system("You are a strict editor. List any errors. Say PASS if none.")
            .user(&state.draft)
            .send()?;

        // ctx,store get rules or previous work?
        if response.contains("PASS") {
            Ok((state, Outcome::Done))
        } else {
            ctx.set("errors", &response);
            Ok((state, Outcome::Next("article_fixer")))
        }
    }
}

struct ArticleFixer;
impl Agent<ArticleState> for ArticleFixer {
    fn name(&self) -> &'static str {
        "article_fixer"
    }
    fn run(&mut self, mut state: ArticleState, ctx: &mut Ctx) -> StepResult<ArticleState> {
        let errors = ctx.get("errors").unwrap_or("no errors found").to_string();
        let response = ctx
            .llm()
            .system("You are a writer. Rewrite the article fixing only the listed errors.")
            .user(format!("Errors:\n{errors}\n\nArticle:\n{}", state.draft))
            .send()?;

        state.draft = response;
        Ok((state, Outcome::Next("article_validator")))
    }
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

fn main() {
    let mut ctx = Ctx::new();

    // could populate ctx.store with some writing rules or could read in a markdown skill and pass
    // that into the agent.
    // Phase 1: find topics
    let topic_wf = Workflow::builder("find-topics")
        .register(TopicSearcher)
        .register(TopicPicker)
        .start_at("topic_searcher")
        .then("topic_picker")
        .build()
        .unwrap();

    let mut topic_runner = Runner::new(topic_wf);
    let topics = topic_runner
        .run(
            TopicState {
                query: "bluecollar engineering newsletter".into(),
                topics: vec![],
                selected: vec![],
            },
            &mut ctx,
        )
        .unwrap();

    println!("=== Topics ===");
    for entry in ctx.logs() {
        println!("  {entry}");
    }
    ctx.clear_logs();
    println!();

    // Phase 2: write one article per topic
    let article_wf = Workflow::builder("write-article")
        .register(ArticleWriter)
        .register(ArticleValidator)
        .register(ArticleFixer)
        .start_at("article_writer")
        .then("article_validator")
        .build()
        .unwrap();

    let mut article_runner = Runner::new(article_wf);
    let mut finished_articles: Vec<String> = Vec::new();

    for (i, topic) in topics.selected.iter().enumerate() {
        println!("=== Article {} ===", i + 1);

        let result = article_runner
            .run(
                ArticleState {
                    topic: topic.clone(),
                    draft: String::new(),
                    revision: 0,
                },
                &mut ctx,
            )
            .unwrap();

        finished_articles.push(result.draft.clone());
        println!("  Revisions: {}", result.revision);

        for entry in ctx.logs() {
            println!("  {entry}");
        }
        ctx.clear_logs();
        println!();
    }

    // Phase 3: "store" the articles
    println!("=== Stored ===");
    for (i, article) in finished_articles.iter().enumerate() {
        let preview: String = article.chars().take(60).collect();
        println!("  article_{}.md: {preview}...", i + 1);
    }
}
