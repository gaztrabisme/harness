//! openai-completions wire format (used by oMLX locally). One-shot JSON request,
//! one-shot JSON response — the simple side of the abstraction.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Message, Provider, Request, Response, Role, StopReason, ToolCall};

/// Which server dialect this provider speaks. Both are openai-completions on the
/// wire — the only divergence is how the reasoning control is encoded, so this
/// enum is consulted *only* in `request_to_body`. Response parsing is identical
/// (both return `reasoning_content` / `tool_calls` / `finish_reason`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
	/// Local oMLX/Qwen: `chat_template_kwargs.enable_thinking` + flat `thinking_budget`.
	Omlx,
	/// DeepSeek V4 (research/30): a `thinking` object `{type, reasoning_effort}`.
	DeepSeek,
}

/// The oMLX API key, resolved at call time: `OMLX_API_KEY` env if set, else
/// `auth.api_key` in `~/.omlx/settings.json` (the same place the omlx skill's
/// scripts read it, so a configured machine needs no env), else empty — the
/// local server then rejects the call with a clear 401 instead of us guessing.
/// Pub because the trajectory recorder must redact whatever value was used.
pub fn omlx_key() -> String {
	if let Ok(k) = std::env::var("OMLX_API_KEY") {
		return k;
	}
	let Some(home) = std::env::var_os("HOME") else { return String::new() };
	let path = std::path::Path::new(&home).join(".omlx/settings.json");
	std::fs::read_to_string(path)
		.ok()
		.and_then(|s| serde_json::from_str::<Value>(&s).ok())
		.and_then(|v| v["auth"]["api_key"].as_str().map(str::to_owned))
		.unwrap_or_default()
}

pub struct OpenAiProvider {
	base_url: String,
	api_key: String,
	name: String,
	dialect: Dialect,
	client: reqwest::Client,
}

impl OpenAiProvider {
	pub fn new(
		base_url: impl Into<String>,
		api_key: impl Into<String>,
		name: impl Into<String>,
		dialect: Dialect,
	) -> Self {
		Self {
			base_url: base_url.into(),
			api_key: api_key.into(),
			name: name.into(),
			dialect,
			client: reqwest::Client::new(),
		}
	}

	/// The local oMLX server (see ~/.claude global context).
	pub fn omlx() -> Self {
		Self::new("http://127.0.0.1:8000/v1", omlx_key(), "omlx", Dialect::Omlx)
	}

	/// DeepSeek V4 (research/30). Env-only credential — the pre-publish baked
	/// fallback was removed when the repo went public-bound (its own comment
	/// carried the trigger: "ROTATE ... if this repo ever goes public").
	pub fn deepseek() -> Result<Self> {
		let key = std::env::var("DEEPSEEK_API_KEY")
			.context("DEEPSEEK_API_KEY unset — the review worker needs it (export it or skip `agent review`)")?;
		Ok(Self::new("https://api.deepseek.com", key, "deepseek", Dialect::DeepSeek))
	}

	fn message_to_wire(m: &Message) -> Value {
		match m.role {
			Role::System => json!({ "role": "system", "content": m.content }),
			Role::User => json!({ "role": "user", "content": m.content }),
			Role::Assistant => {
				if m.tool_calls.is_empty() {
					json!({ "role": "assistant", "content": m.content })
				} else {
					let calls: Vec<Value> = m
						.tool_calls
						.iter()
						.map(|c| {
							json!({
								"id": c.id,
								"type": "function",
								"function": { "name": c.name, "arguments": c.arguments },
							})
						})
						.collect();
					json!({ "role": "assistant", "content": m.content, "tool_calls": calls })
				}
			}
			Role::Tool => json!({
				"role": "tool",
				"tool_call_id": m.tool_call_id.as_deref().unwrap_or(""),
				"content": m.content,
			}),
		}
	}

	/// Build the openai-completions request body. Extracted from `complete` so the
	/// wire shape — notably the reasoning control, which is the only per-dialect
	/// divergence — is unit-testable without a live server.
	fn request_to_body(req: &Request, dialect: Dialect) -> Value {
		let messages: Vec<Value> = req.messages.iter().map(Self::message_to_wire).collect();
		let mut body = json!({
			"model": req.model,
			"messages": messages,
			"max_tokens": req.max_tokens,
			"temperature": req.temperature,
		});
		// Reasoning control. The two dialects encode it differently; `think_budget`
		// is the normalized input (None → off, Some(n) → on with soft target `n`).
		match dialect {
			// oMLX/Qwen (research/26 §9.1): `enable_thinking` via `chat_template_kwargs`
			// (per-request inference control) plus, when budgeted, a FLAT top-level
			// `thinking_budget:n` — the verified placement (Probe 3/4). `n` is a soft
			// target; `max_tokens` is the hard backstop. Needs the server's per-model
			// `thinking_budget_enabled` + `reasoning_parser` flags on.
			Dialect::Omlx => {
				body["chat_template_kwargs"] = json!({ "enable_thinking": req.think_budget.is_some() });
				if let Some(n) = req.think_budget {
					body["thinking_budget"] = json!(n);
				}
			}
			// DeepSeek V4 (research/30): a `thinking` object. No numeric budget exists —
			// the knob is `reasoning_effort` ∈ {high, max}. We map any modest budget to
			// "high" and a large one (≥4096) to "max"; `max_tokens` still bounds CoT +
			// answer together. Off → `{type:"disabled"}`.
			Dialect::DeepSeek => {
				body["thinking"] = match req.think_budget {
					None => json!({ "type": "disabled" }),
					Some(n) => json!({
						"type": "enabled",
						"reasoning_effort": if n >= 4096 { "max" } else { "high" },
					}),
				};
			}
		}
		if !req.tools.is_empty() {
			let tools: Vec<Value> = req
				.tools
				.iter()
				.map(|t| {
					json!({
						"type": "function",
						"function": {
							"name": t.name,
							"description": t.description,
							"parameters": t.parameters,
						},
					})
				})
				.collect();
			body["tools"] = json!(tools);
			body["tool_choice"] = json!("auto");
		}
		body
	}
}

#[async_trait::async_trait]
impl Provider for OpenAiProvider {
	fn name(&self) -> &str {
		&self.name
	}

	async fn complete(&self, req: &Request) -> Result<Response> {
		let body = Self::request_to_body(req, self.dialect);

		let resp = self
			.client
			.post(format!("{}/chat/completions", self.base_url))
			.bearer_auth(&self.api_key)
			.json(&body)
			.send()
			.await
			.with_context(|| format!("{} request failed", self.name))?;

		let status = resp.status();
		let text = resp.text().await.with_context(|| format!("reading {} body", self.name))?;
		if !status.is_success() {
			bail!("{} returned {status}: {text}", self.name);
		}

		let parsed: OaResponse = serde_json::from_str(&text)
			.with_context(|| format!("parsing {} response: {text}", self.name))?;
		let choice = parsed
			.choices
			.into_iter()
			.next()
			.with_context(|| format!("no choices in {} response", self.name))?;

		let tool_calls: Vec<ToolCall> = choice
			.message
			.tool_calls
			.into_iter()
			.map(|c| ToolCall { id: c.id, name: c.function.name, arguments: c.function.arguments })
			.collect();

		let stop_reason = match choice.finish_reason.as_deref() {
			Some("tool_calls") => StopReason::ToolCalls,
			Some("length") => StopReason::Length,
			_ if !tool_calls.is_empty() => StopReason::ToolCalls,
			_ => StopReason::End,
		};

		Ok(Response {
			text: choice.message.content.unwrap_or_default(),
			reasoning: choice.message.reasoning_content.unwrap_or_default(),
			tool_calls,
			stop_reason,
		})
	}
}

// ── wire types (parse only what we use) ──────────────────────────────────────

#[derive(Deserialize)]
struct OaResponse {
	choices: Vec<OaChoice>,
}

#[derive(Deserialize)]
struct OaChoice {
	message: OaMessage,
	finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OaMessage {
	#[serde(default)]
	content: Option<String>,
	/// Set when the served model has a reasoning parser on (oMLX dashboard): the
	/// chain-of-thought, split out of `content`. Absent otherwise.
	#[serde(default)]
	reasoning_content: Option<String>,
	#[serde(default)]
	tool_calls: Vec<OaToolCall>,
}

#[derive(Deserialize)]
struct OaToolCall {
	id: String,
	function: OaFunction,
}

#[derive(Deserialize)]
struct OaFunction {
	name: String,
	arguments: String,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Message, Request, ToolDef};

	/// Live smoke test against local oMLX — proves the openai-completions path
	/// end-to-end (Phase 0 criterion #1 substrate). Ignored by default so the
	/// suite passes when oMLX is down; run with `cargo test -- --ignored`.
	#[tokio::test]
	#[ignore]
	async fn omlx_emits_tool_call() {
		let p = OpenAiProvider::omlx();
		let req = Request {
			model: "Qwen3.6-35B-A3B-oQ8-fp16-mtp".into(),
			messages: vec![Message::user("Read the file /tmp/notes.txt. Use the tool.")],
			tools: vec![ToolDef {
				name: "read_file".into(),
				description: "Read a file from disk".into(),
				parameters: serde_json::json!({
					"type": "object",
					"properties": { "path": { "type": "string" } },
					"required": ["path"],
				}),
			}],
			max_tokens: 256,
			temperature: 0.3,
			think_budget: None,
		};
		let r = p.complete(&req).await.expect("oMLX call");
		assert_eq!(r.stop_reason, StopReason::ToolCalls);
		assert_eq!(r.tool_calls.len(), 1);
		assert_eq!(r.tool_calls[0].name, "read_file");
	}

	fn req(think_budget: Option<u32>) -> Request {
		Request {
			model: "m".into(),
			messages: vec![Message::user("hi")],
			tools: vec![],
			max_tokens: 64,
			temperature: 0.3,
			think_budget,
		}
	}

	// oMLX dialect: the bounded-thinking control reaches the wire as
	// `chat_template_kwargs.enable_thinking` (the on/off flag) plus, when a budget is
	// set, a FLAT top-level `thinking_budget` (research/26 §9.1 verified path). With
	// no budget the flat key is absent entirely.
	#[test]
	fn body_carries_thinking_budget() {
		let off = OpenAiProvider::request_to_body(&req(None), Dialect::Omlx);
		assert_eq!(off["chat_template_kwargs"]["enable_thinking"], serde_json::json!(false));
		assert!(off.get("thinking_budget").is_none(), "no budget key when think is off");
		assert!(off.get("thinking").is_none(), "no deepseek thinking object on the oMLX path");

		let on = OpenAiProvider::request_to_body(&req(Some(2048)), Dialect::Omlx);
		assert_eq!(on["chat_template_kwargs"]["enable_thinking"], serde_json::json!(true));
		assert_eq!(on["thinking_budget"], serde_json::json!(2048));
	}

	// DeepSeek dialect: reasoning is a `thinking` object, never the oMLX keys.
	// None → disabled; Some(n) → enabled with reasoning_effort high (modest n) or
	// max (n ≥ 4096). research/30.
	#[test]
	fn body_carries_deepseek_thinking() {
		let off = OpenAiProvider::request_to_body(&req(None), Dialect::DeepSeek);
		assert_eq!(off["thinking"], serde_json::json!({ "type": "disabled" }));
		assert!(off.get("chat_template_kwargs").is_none(), "no oMLX keys on the deepseek path");
		assert!(off.get("thinking_budget").is_none(), "no flat budget on the deepseek path");

		let modest = OpenAiProvider::request_to_body(&req(Some(2048)), Dialect::DeepSeek);
		assert_eq!(
			modest["thinking"],
			serde_json::json!({ "type": "enabled", "reasoning_effort": "high" }),
		);

		let big = OpenAiProvider::request_to_body(&req(Some(8192)), Dialect::DeepSeek);
		assert_eq!(big["thinking"]["reasoning_effort"], serde_json::json!("max"));
	}

	/// Live smoke against DeepSeek V4 — proves the deepseek dialect end-to-end
	/// (request shape accepted, a tool call comes back). Reads `DEEPSEEK_API_KEY`;
	/// skips cleanly (returns Ok) if unset so CI without the secret stays green.
	/// Run with `DEEPSEEK_API_KEY=… cargo test -p provider -- --ignored deepseek`.
	#[tokio::test]
	#[ignore]
	async fn deepseek_emits_tool_call() {
		if std::env::var("DEEPSEEK_API_KEY").is_err() {
			eprintln!("DEEPSEEK_API_KEY unset — skipping live deepseek smoke");
			return;
		}
		let p = OpenAiProvider::deepseek().expect("deepseek provider");
		let req = Request {
			model: "deepseek-v4-pro".into(),
			messages: vec![Message::user("Read the file /tmp/notes.txt. Use the tool.")],
			tools: vec![ToolDef {
				name: "read_file".into(),
				description: "Read a file from disk".into(),
				parameters: serde_json::json!({
					"type": "object",
					"properties": { "path": { "type": "string" } },
					"required": ["path"],
				}),
			}],
			max_tokens: 256,
			temperature: 0.3,
			think_budget: None,
		};
		let r = p.complete(&req).await.expect("deepseek call");
		assert_eq!(r.stop_reason, StopReason::ToolCalls);
		assert_eq!(r.tool_calls[0].name, "read_file");
	}
}
