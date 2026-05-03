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
    let api_key = match env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Set ANTHROPIC_API_KEY to run this example.");
            eprintln!("The fast triage agent runs locally on Ollama (no key needed);");
            eprintln!("the deep investigation report uses the Anthropic API.");
            std::process::exit(1);
        }
    };

    let fast_model = env::var("INCIDENT_FAST_MODEL").unwrap_or_else(|_| "qwen3:8b".to_string());
    let deep_model =
        env::var("INCIDENT_DEEP_MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

    let fast_llm = LlmConfig::builder()
        .provider(Provider::Ollama)
        .base_url("http://localhost:11434")
        .model(fast_model)
        .num_ctx(4096)
        .build()?;

    let deep_llm = LlmConfig::builder()
        .provider(Provider::Anthropic)
        .base_url("https://api.anthropic.com")
        .model(deep_model)
        .api_key(&api_key)
        .max_tokens(1400)
        .build()?;

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
