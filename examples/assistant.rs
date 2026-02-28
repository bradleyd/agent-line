use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct BriefingState {
    weather: String,
    calendar: String,
    emails: String,
    summary: String,
}

impl BriefingState {
    fn new() -> Self {
        Self {
            weather: String::new(),
            calendar: String::new(),
            emails: String::new(),
            summary: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

struct FetchWeather;
impl Agent<BriefingState> for FetchWeather {
    fn name(&self) -> &'static str {
        "fetch_weather"
    }
    fn run(&mut self, mut state: BriefingState, ctx: &mut Ctx) -> StepResult<BriefingState> {
        ctx.log("fetching weather data");

        // Stub: in a real app, call a weather API via tools::http
        state.weather = "72F and sunny, high of 78F. Light breeze from the southwest.".into();

        Ok((state, Outcome::Continue))
    }
}

struct FetchCalendar;
impl Agent<BriefingState> for FetchCalendar {
    fn name(&self) -> &'static str {
        "fetch_calendar"
    }
    fn run(&mut self, mut state: BriefingState, ctx: &mut Ctx) -> StepResult<BriefingState> {
        ctx.log("fetching calendar events");

        state.calendar = "\
            9:00 AM - Standup with the team\n\
            11:00 AM - Design review\n\
            1:00 PM - Lunch with Sarah\n\
            3:00 PM - Sprint planning"
            .into();

        Ok((state, Outcome::Continue))
    }
}

struct FetchEmail;
impl Agent<BriefingState> for FetchEmail {
    fn name(&self) -> &'static str {
        "fetch_email"
    }
    fn run(&mut self, mut state: BriefingState, ctx: &mut Ctx) -> StepResult<BriefingState> {
        ctx.log("fetching email summaries");

        state.emails = "\
            - AWS billing alert: $42.17 for March (within budget)\n\
            - PR #187 approved by Jake, ready to merge\n\
            - Newsletter from Rust Weekly: edition #412"
            .into();

        Ok((state, Outcome::Continue))
    }
}

struct Summarize;
impl Agent<BriefingState> for Summarize {
    fn name(&self) -> &'static str {
        "summarize"
    }
    fn run(&mut self, mut state: BriefingState, ctx: &mut Ctx) -> StepResult<BriefingState> {
        ctx.log("generating daily briefing via LLM");

        let prompt = format!(
            "Weather:\n{}\n\nCalendar:\n{}\n\nEmails:\n{}",
            state.weather, state.calendar, state.emails
        );

        let response = ctx
            .llm()
            .system(
                "You are a personal assistant. Produce a concise daily briefing \
                 from the provided weather, calendar, and email data. \
                 Keep it under 200 words. Use plain text, no markdown.",
            )
            .user(prompt)
            .send()?;

        state.summary = response;
        Ok((state, Outcome::Done))
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let mut ctx = Ctx::new();

    let wf = Workflow::builder("daily-briefing")
        .register(FetchWeather)
        .register(FetchCalendar)
        .register(FetchEmail)
        .register(Summarize)
        .start_at("fetch_weather")
        .then("fetch_calendar")
        .then("fetch_email")
        .then("summarize")
        .build()
        .unwrap();

    let mut runner = Runner::new(wf).with_tracing();

    let mut iteration = 0;
    loop {
        iteration += 1;
        println!("=== Briefing #{iteration} ===\n");

        match runner.run(BriefingState::new(), &mut ctx) {
            Ok(state) => {
                println!("{}\n", state.summary);
            }
            Err(e) => {
                eprintln!("Briefing failed: {e}\n");
            }
        }

        ctx.clear_logs();

        println!("(sleeping 30s before next briefing...)\n");
        thread::sleep(Duration::from_secs(30));
    }
}
