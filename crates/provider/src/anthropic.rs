//! anthropic-messages wire format. The structurally *different* side of the
//! abstraction — this is what stresses the normalized model:
//!   - `system` is a top-level field, not a message;
//!   - tool calls are `tool_use` content blocks (input is a JSON object);
//!   - tool results are `tool_result` content blocks inside a *user* turn.
//!
//! Spike scope: non-streaming (`stream:false`). SSE delta-assembly is a wire
//! mechanic, not an abstraction stress, and the live leg is key-gated — so it's
//! the documented v1 upgrade, not a Phase 0 blocker. The point proven here is
//! that this very different message model maps onto the *same* `Provider` trait
//! and the *same* `Request`/`Response` with no fork.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Provider, Request, Response, Role, StopReason, ToolCall};

pub struct AnthropicProvider {
	base_url: String,
	api_key: String,
	name: String,
	client: reqwest::Client,
}

impl AnthropicProvider {
	pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
		Self {
			base_url: base_url.into(),
			api_key: api_key.into(),
			name: "anthropic".into(),
			client: reqwest::Client::new(),
		}
	}

	/// Reads `ANTHROPIC_API_KEY` from the environment. Errors (rather than
	/// panicking) so the loop can run oMLX-only when no key is present.
	pub fn from_env() -> Result<Self> {
		let key = std::env::var("ANTHROPIC_API_KEY")
			.context("ANTHROPIC_API_KEY not set — the anthropic leg is key-gated")?;
		Ok(Self::new("https://api.anthropic.com/v1", key))
	}

	/// Build (system, messages) from the normalized message list. System turns
	/// are hoisted into the top-level `system` string; tool results become
	/// `tool_result` blocks in user turns.
	fn split_messages(req: &Request) -> (String, Vec<Value>) {
		let mut system = String::new();
		let mut out: Vec<Value> = Vec::new();
		for m in &req.messages {
			match m.role {
				Role::System => {
					if !system.is_empty() {
						system.push_str("\n\n");
					}
					system.push_str(&m.content);
				}
				Role::User => {
					out.push(json!({ "role": "user", "content": [{ "type": "text", "text": m.content }] }));
				}
				Role::Assistant => {
					let mut blocks: Vec<Value> = Vec::new();
					if !m.content.is_empty() {
						blocks.push(json!({ "type": "text", "text": m.content }));
					}
					for c in &m.tool_calls {
						let input: Value = serde_json::from_str(&c.arguments).unwrap_or_else(|_| json!({}));
						blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": input }));
					}
					out.push(json!({ "role": "assistant", "content": blocks }));
				}
				Role::Tool => {
					let id = m.tool_call_id.as_deref().unwrap_or("");
					out.push(json!({
						"role": "user",
						"content": [{ "type": "tool_result", "tool_use_id": id, "content": m.content }],
					}));
				}
			}
		}
		(system, out)
	}
}

#[async_trait::async_trait]
impl Provider for AnthropicProvider {
	fn name(&self) -> &str {
		&self.name
	}

	async fn complete(&self, req: &Request) -> Result<Response> {
		let (system, messages) = Self::split_messages(req);
		let mut body = json!({
			"model": req.model,
			"max_tokens": req.max_tokens,
			"temperature": req.temperature,
			"messages": messages,
			"stream": false,
		});
		if !system.is_empty() {
			body["system"] = json!(system);
		}
		if !req.tools.is_empty() {
			let tools: Vec<Value> = req
				.tools
				.iter()
				.map(|t| {
					json!({
						"name": t.name,
						"description": t.description,
						"input_schema": t.parameters,
					})
				})
				.collect();
			body["tools"] = json!(tools);
		}

		let resp = self
			.client
			.post(format!("{}/messages", self.base_url))
			.header("x-api-key", &self.api_key)
			.header("anthropic-version", "2023-06-01")
			.json(&body)
			.send()
			.await
			.context("anthropic request failed")?;

		let status = resp.status();
		let text = resp.text().await.context("reading anthropic body")?;
		if !status.is_success() {
			bail!("anthropic returned {status}: {text}");
		}

		let parsed: AnthResponse = serde_json::from_str(&text)
			.with_context(|| format!("parsing anthropic response: {text}"))?;

		let mut out_text = String::new();
		let mut tool_calls: Vec<ToolCall> = Vec::new();
		for block in parsed.content {
			match block {
				AnthBlock::Text { text } => out_text.push_str(&text),
				AnthBlock::ToolUse { id, name, input } => {
					tool_calls.push(ToolCall { id, name, arguments: input.to_string() });
				}
			}
		}

		let stop_reason = match parsed.stop_reason.as_deref() {
			Some("tool_use") => StopReason::ToolCalls,
			Some("max_tokens") => StopReason::Length,
			_ if !tool_calls.is_empty() => StopReason::ToolCalls,
			_ => StopReason::End,
		};

		// anthropic `thinking` blocks aren't parsed here yet — reasoning stays empty
		// until there's a consumer for it on this path.
		Ok(Response { text: out_text, reasoning: String::new(), tool_calls, stop_reason })
	}
}

// ── wire types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AnthResponse {
	content: Vec<AnthBlock>,
	stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthBlock {
	Text { text: String },
	ToolUse { id: String, name: String, input: Value },
}
