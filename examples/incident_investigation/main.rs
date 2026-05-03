mod agents;
mod data;
mod state;

use agent_line::{Ctx, LlmConfig, Provider, Runner, Workflow};
use agents::{
    CorrelateTimeline, FindAnomalies, InvestigationReport, LoadEvidence, TriageNarrative,
};
use state::IncidentState;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Both models default to local Ollama so this example runs out of the
    // box. Override with env vars or swap to a remote provider in the
    // commented-out alternatives below.
    //
    //   ollama pull qwen3:8b
    //   ollama pull llama3.1:70b   # heavier model for the deep step
    //   cargo run --example incident_investigation
    //
    let fast_model = env::var("INCIDENT_FAST_MODEL").unwrap_or_else(|_| "qwen3:8b".to_string());
    let deep_model = env::var("INCIDENT_DEEP_MODEL").unwrap_or_else(|_| "llama3.1:70b".to_string());

    let fast_llm = LlmConfig::builder()
        .provider(Provider::Ollama)
        .base_url("http://localhost:11434")
        .model(fast_model)
        .num_ctx(4096)
        .build()?;

    let deep_llm = LlmConfig::builder()
        .provider(Provider::Ollama)
        .base_url("http://localhost:11434")
        .model(deep_model)
        .num_ctx(8192)
        .build()?;

    // --- Remote alternatives ---
    //
    // Swap deep_llm to OpenRouter (one API key works across many models):
    //
    //     let api_key = env::var("OPENROUTER_API_KEY")?;
    //     let deep_llm = LlmConfig::builder()
    //         .provider(Provider::OpenAi)
    //         .base_url("https://openrouter.ai/api")
    //         .model("anthropic/claude-sonnet-4.6")
    //         .api_key(&api_key)
    //         .max_tokens(1400)
    //         .build()?;
    //
    // Or to Anthropic directly:
    //
    //     let api_key = env::var("ANTHROPIC_API_KEY")?;
    //     let deep_llm = LlmConfig::builder()
    //         .provider(Provider::Anthropic)
    //         .base_url("https://api.anthropic.com")
    //         .model("claude-sonnet-4-20250514")
    //         .api_key(&api_key)
    //         .max_tokens(1400)
    //         .build()?;

    let mut ctx = Ctx::new();
    let workflow = Workflow::builder("incident-investigation")
        .register(LoadEvidence)
        .register(FindAnomalies)
        .register(CorrelateTimeline)
        .register(TriageNarrative::new(fast_llm))
        .register(InvestigationReport::new(deep_llm))
        .start_at("load_evidence")
        .then("find_anomalies")
        .then("correlate_timeline")
        .then("triage_narrative")
        .then("investigation_report")
        .build()?;

    let final_state = Runner::new(workflow)
        .with_tracing()
        .run(IncidentState::new("checkout 5xx spike"), &mut ctx)?;

    println!("\n=== Shared Context Log ===");
    for entry in ctx.logs() {
        println!("  {entry}");
    }

    println!("\n=== Fast Triage Note ===\n{}", final_state.triage_note);
    println!("\n=== Investigation Report ===\n{}", final_state.report);

    Ok(())
}
