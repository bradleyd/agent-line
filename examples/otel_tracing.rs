use agent_line::{Agent, Ctx, ErrorEvent, Outcome, Runner, StepEvent, StepResult, Workflow};
use opentelemetry::trace::{Span, Status, TraceContextExt, Tracer};
use opentelemetry::{Context, KeyValue, global};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;

#[derive(Clone, Debug)]
struct DraftState {
    topic: String,
    notes: String,
    draft: String,
}

impl DraftState {
    fn new(topic: &str) -> Self {
        Self {
            topic: topic.to_string(),
            notes: String::new(),
            draft: String::new(),
        }
    }
}

struct Research;
impl Agent<DraftState> for Research {
    fn name(&self) -> &'static str {
        "research"
    }

    fn run(&mut self, mut state: DraftState, ctx: &mut Ctx) -> StepResult<DraftState> {
        ctx.log("researching topic");
        state.notes = format!(
            "Key points about {}: keep examples short, concrete, and practical.",
            state.topic
        );
        Ok((state, Outcome::Continue))
    }
}

struct Write;
impl Agent<DraftState> for Write {
    fn name(&self) -> &'static str {
        "write"
    }

    fn run(&mut self, mut state: DraftState, ctx: &mut Ctx) -> StepResult<DraftState> {
        ctx.log("writing first draft");
        state.draft = format!(
            "Topic: {}\n\n{}\n\nDraft: Build one step at a time and keep feedback loops tight.",
            state.topic, state.notes
        );
        Ok((state, Outcome::Continue))
    }
}

struct Finalize;
impl Agent<DraftState> for Finalize {
    fn name(&self) -> &'static str {
        "finalize"
    }

    fn run(&mut self, state: DraftState, ctx: &mut Ctx) -> StepResult<DraftState> {
        ctx.log("finalizing output");
        Ok((state, Outcome::Done))
    }
}

fn init_tracer() -> impl FnOnce() {
    let exporter = opentelemetry_stdout::SpanExporter::default();
    let resource = Resource::builder()
        .with_attributes([KeyValue::new("service.name", "agent-line-example")])
        .build();
    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_simple_exporter(exporter)
        .build();
    global::set_tracer_provider(provider.clone());
    move || {
        let _ = provider.shutdown();
    }
}

fn record_step(workflow: &str, parent: &Context, event: &StepEvent) {
    let tracer = global::tracer("agent-line.examples.otel");
    let mut span = tracer.start_with_context("agent.step", parent);
    span.set_attribute(KeyValue::new("workflow.name", workflow.to_string()));
    span.set_attribute(KeyValue::new("agent.name", event.agent.to_string()));
    span.set_attribute(KeyValue::new("step.number", event.step_number as i64));
    span.set_attribute(KeyValue::new("step.retries", event.retries as i64));
    span.set_attribute(KeyValue::new(
        "step.duration_ms",
        event.duration.as_millis() as i64,
    ));
    span.set_attribute(KeyValue::new(
        "step.outcome",
        format!("{:?}", event.outcome),
    ));
    span.end();
}

fn record_error(workflow: &str, parent: &Context, event: &ErrorEvent) {
    let tracer = global::tracer("agent-line.examples.otel");
    let mut span = tracer.start_with_context("agent.step.error", parent);
    span.set_status(Status::error(event.error.to_string()));
    span.set_attribute(KeyValue::new("workflow.name", workflow.to_string()));
    span.set_attribute(KeyValue::new("agent.name", event.agent.to_string()));
    span.set_attribute(KeyValue::new("step.number", event.step_number as i64));
    span.set_attribute(KeyValue::new("error.message", event.error.to_string()));
    span.end();
}

fn main() {
    let shutdown = init_tracer();
    let tracer = global::tracer("agent-line.examples.otel");
    let workflow_name = "draft-workflow";

    let mut ctx = Ctx::new();
    let wf = Workflow::builder(workflow_name)
        .register(Research)
        .register(Write)
        .register(Finalize)
        .start_at("research")
        .then("write")
        .then("finalize")
        .build()
        .expect("workflow should be valid");

    let mut workflow_span = tracer.start("workflow.run");
    workflow_span.set_attribute(KeyValue::new("workflow.name", workflow_name.to_string()));
    let parent = Context::new().with_remote_span_context(workflow_span.span_context().clone());
    let parent_for_step = parent.clone();
    let parent_for_error = parent.clone();

    let mut runner = Runner::new(wf)
        .on_step(move |event| record_step(workflow_name, &parent_for_step, event))
        .on_error(move |event| record_error(workflow_name, &parent_for_error, event));

    let initial = DraftState::new("OpenTelemetry tracing with agent-line hooks");
    match runner.run(initial, &mut ctx) {
        Ok(final_state) => {
            workflow_span.set_status(Status::Ok);
            println!("=== Final Draft ===\n{}", final_state.draft);
        }
        Err(err) => {
            workflow_span.set_status(Status::error(err.to_string()));
            eprintln!("workflow failed: {err}");
        }
    }

    workflow_span.end();
    shutdown();
}
