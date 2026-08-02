//! Normalized provider abstraction — the Phase 0 thing under test.
//!
//! One `Request`/`Response` model and one `Provider` trait span two
//! structurally different wire formats (openai-completions + anthropic-messages).
//! Each wire format's quirks live inside its own impl; the trait boundary stays
//! format-agnostic. If this model needs a redesign to fit the second format,
//! that is a sizing finding (Phase 0 kill-trigger).

use anyhow::Result;

pub mod anthropic;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use openai::{Dialect, OpenAiProvider, omlx_key};

/// Role of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
	System,
	User,
	Assistant,
	Tool,
}

/// A single tool invocation requested by the model. `arguments` is the raw
/// JSON-object string exactly as the model emitted it (not pre-parsed — the
/// caller decides how strict to be).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
	pub id: String,
	pub name: String,
	pub arguments: String,
}

/// One conversation message in the normalized model.
#[derive(Debug, Clone)]
pub struct Message {
	pub role: Role,
	/// Text content. Empty for an assistant turn that is purely tool calls.
	pub content: String,
	/// Tool calls emitted by an assistant turn.
	pub tool_calls: Vec<ToolCall>,
	/// Set on a `Tool`-role message: which call this is the result of.
	pub tool_call_id: Option<String>,
}

impl Message {
	pub fn system(content: impl Into<String>) -> Self {
		Self { role: Role::System, content: content.into(), tool_calls: Vec::new(), tool_call_id: None }
	}
	pub fn user(content: impl Into<String>) -> Self {
		Self { role: Role::User, content: content.into(), tool_calls: Vec::new(), tool_call_id: None }
	}
	pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
		Self { role: Role::Assistant, content: content.into(), tool_calls, tool_call_id: None }
	}
	/// A tool result fed back to the model.
	pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
		Self {
			role: Role::Tool,
			content: content.into(),
			tool_calls: Vec::new(),
			tool_call_id: Some(call_id.into()),
		}
	}
}

/// A tool the model may call. `parameters` is a JSON-Schema object.
#[derive(Debug, Clone)]
pub struct ToolDef {
	pub name: String,
	pub description: String,
	pub parameters: serde_json::Value,
}

/// A completion request in the normalized model.
#[derive(Debug, Clone)]
pub struct Request {
	pub model: String,
	pub messages: Vec<Message>,
	pub tools: Vec<ToolDef>,
	pub max_tokens: u32,
	pub temperature: f32,
	/// Server-side reasoning control as a **bounded budget** (research/26).
	/// `None` → reasoning OFF (`enable_thinking:false`). `Some(n)` → reasoning ON
	/// with a *soft* target of ~`n` reasoning tokens (the preset ladder is
	/// low/med/high = 1024/2048/4096). The budget is a target, not a hard cap —
	/// verified against oMLX: the model can run slightly over or under, there is no
	/// exhaustion signal, and `max_tokens` remains the hard backstop. Bounded
	/// think-ON is loop-safe (the earlier think-ON-unsafe verdict was a
	/// parser-off + no-budget artifact, retracted in research/26 §9.1). Requires the
	/// server's per-model `thinking_budget_enabled` + `reasoning_parser` flags on;
	/// when off, `Some(n)` degrades to plain think-ON. The wire mechanism is
	/// provider-specific and stays in each impl.
	pub think_budget: Option<u32>,
}

/// Why the model stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
	/// Model wants to call tools — the loop must dispatch them and continue.
	ToolCalls,
	/// Natural end of turn.
	End,
	/// Hit the token cap.
	Length,
}

/// An assembled completion. Each provider parses its own wire format (one-shot
/// JSON for openai, accumulated SSE deltas for anthropic) down to this.
#[derive(Debug, Clone)]
pub struct Response {
	/// The model's answer — the `content` field. With a server-side reasoning
	/// parser on, this is the *post-reasoning* answer (concise), not the
	/// deliberation.
	pub text: String,
	/// Server-separated chain-of-thought (`reasoning_content`), when the provider
	/// emits it. Captured for the trajectory (audit/telemetry) but NOT re-fed —
	/// it is the bulky deliberation, and re-feeding it bloats the window (the
	/// research/21 finding, now solved cleanly by a field boundary instead of
	/// tag-stripping). Empty when the provider doesn't separate reasoning.
	pub reasoning: String,
	pub tool_calls: Vec<ToolCall>,
	pub stop_reason: StopReason,
}

/// The seam under test. Two wire formats, one trait.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
	async fn complete(&self, req: &Request) -> Result<Response>;
	/// Human-readable id for logs.
	fn name(&self) -> &str;
}
