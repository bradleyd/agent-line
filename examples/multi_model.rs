// Multi-model workflow: use a local Ollama model to pick topics, then a
// stronger remote model to write articles. The workflow uses one shared Ctx;
// each LLM-powered agent receives the model config it needs.
//
// Run with:
//   ollama pull qwen3:8b   # one-time, if not already pulled
//   ANTHROPIC_API_KEY=sk-ant-... cargo run --example multi_model
//
use agent_line::{Agent, Ctx, LlmConfig, Outcome, Provider, Runner, StepResult, Workflow};
use std::env;

#[derive(Clone)]
struct State {
    theme: String,
    topics: Vec<String>,
    articles: Vec<String>,
}

struct TopicPicker {
    llm: LlmConfig,
}

impl TopicPicker {
    fn new(llm: LlmConfig) -> Self {
        Self { llm }
    }
}

impl Agent<State> for TopicPicker {
    fn name(&self) -> &'static str {
        "topic_picker"
    }

    fn run(&mut self, mut state: State, _ctx: &mut Ctx) -> StepResult<State> {
        let response = self
            .llm
            .request()
            .system(
                "You are a newsletter curator. Pick 3 topics for the given theme. \
                 Return one per line, no numbering, no preamble.",
            )
            .user(format!("Theme: {}", state.theme))
            .send()?;

        state.topics = response
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .take(3)
            .collect();

        Ok((state, Outcome::Done))
    }
}

struct ArticleWriter {
    llm: LlmConfig,
}

impl ArticleWriter {
    fn new(llm: LlmConfig) -> Self {
        Self { llm }
    }
}

impl Agent<State> for ArticleWriter {
    fn name(&self) -> &'static str {
        "article_writer"
    }

    fn run(&mut self, mut state: State, _ctx: &mut Ctx) -> StepResult<State> {
        for topic in &state.topics {
            let article = self
                .llm
                .request()
                .system("You are a careful writer. Write a short 2-paragraph article on the topic.")
                .user(topic)
                .send()?;
            state.articles.push(article);
        }

        Ok((state, Outcome::Done))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = match env::var("ANTHROPIC_API_KEY") {
        Ok(k) => k,
        Err(_) => {
            eprintln!("Set ANTHROPIC_API_KEY to run this example.");
            eprintln!("The cheap model runs locally on Ollama (no key needed);");
            eprintln!("the strong model uses the Anthropic API.");
            std::process::exit(1);
        }
    };

    let cheap = LlmConfig::builder()
        .provider(Provider::Ollama)
        .base_url("http://localhost:11434")
        .model("qwen3:8b")
        .num_ctx(4096)
        .build()?;

    let strong = LlmConfig::builder()
        .provider(Provider::Anthropic)
        .base_url("https://api.anthropic.com")
        .model("claude-sonnet-4-20250514")
        .api_key(&api_key)
        .max_tokens(1200)
        .build()?;

    let mut ctx = Ctx::new();

    let topic_wf = Workflow::builder("topics")
        .register(TopicPicker::new(cheap))
        .build()?;

    let article_wf = Workflow::builder("articles")
        .register(ArticleWriter::new(strong))
        .build()?;

    let initial = State {
        theme: "rust learning resources for embedded developers".into(),
        topics: vec![],
        articles: vec![],
    };

    let after_topics = Runner::new(topic_wf)
        .run(initial, &mut ctx)
        .expect("topic phase failed");

    println!("=== Topics (qwen3:8b, local) ===");
    for t in &after_topics.topics {
        println!("  - {t}");
    }

    let final_state = Runner::new(article_wf)
        .run(after_topics, &mut ctx)
        .expect("article phase failed");

    println!("\n=== Articles (claude-sonnet, remote) ===");
    for (i, a) in final_state.articles.iter().enumerate() {
        let preview: String = a.chars().take(160).collect();
        println!("  [{}]: {preview}...", i + 1);
    }

    Ok(())
}
