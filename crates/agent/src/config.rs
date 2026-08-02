//! Provider registry — pick the backend for a landing `run` from config, not code.
//!
//! The swap seam is `trait Provider`; this module is the *selection* layer that
//! sits in front of it. A new endpoint of a wire format the harness already speaks
//! (oMLX/openai, deepseek, anthropic) is **config-only** — add a `[section]` to
//! `providers.toml`, zero recompile. A genuinely new wire format is the only thing
//! that needs a new `Provider` impl. That boundary is the whole point.
//!
//! ## Resolution order (`select`)
//! 1. name  = `HARNESS_PROVIDER`            (default `omlx`)
//! 2. model = `HARNESS_MODEL`               (overrides the registry's model)
//! 3. registry = built-in defaults, with file entries layered ON TOP
//!    (file path = `HARNESS_PROVIDERS_CONFIG`, else `<root>/providers.toml`).
//! 4. resolve the name against the registry; unknown name → loud error.
//!
//! With **no file present**, behaviour is identical to the old hardcoded
//! `provider_and_model` (omlx default, `HARNESS_PROVIDER=deepseek` → deepseek-v4-pro)
//! — see the `no_file_*` tests. That regression-safety is deliberate.
//!
//! ## Secrets
//! A config entry names an env var (`key_env`) — it NEVER holds an inline key.
//! The built-in oMLX default uses the local, non-secret key via `OpenAiProvider::omlx()`,
//! so the key stays out of any file even in the zero-config case.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use provider::{AnthropicProvider, Dialect, OpenAiProvider, Provider};

/// A provider entry parsed from `providers.toml`. All fields are required; the
/// parser errors loudly if any are missing (no silent partial config).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDef {
	pub base_url: String,
	/// Name of the env var holding the API key — never the key itself.
	pub key_env: String,
	pub model: String,
	/// Wire dialect: `omlx` | `deepseek` | `anthropic`.
	pub dialect: String,
}

/// Pick `(provider, model)` for a landing run from env + optional config file.
/// `root` is the repo root (where `providers.toml` is looked for by default).
pub fn select(root: &Path) -> Result<(Box<dyn Provider>, String)> {
	let name = env_nonempty("HARNESS_PROVIDER").unwrap_or_else(|| "omlx".to_string());
	let model_override = env_nonempty("HARNESS_MODEL");

	let registry = load_registry(root)?;
	resolve(&name, model_override, &registry)
}

/// Resolve a *specific* provider name against the registry, ignoring
/// `HARNESS_PROVIDER`/`HARNESS_MODEL` — for roles that pick their own backend
/// instead of inheriting the landing run's (e.g. `agent draft` defaults to the
/// strong cloud backend regardless of what `run` is configured to use).
pub fn select_named(root: &Path, name: &str) -> Result<(Box<dyn Provider>, String)> {
	let registry = load_registry(root)?;
	resolve(name, None, &registry)
}

/// The pure decision: resolve a provider name against a (possibly empty) registry.
/// File entries win over built-ins; built-ins cover `omlx`/`deepseek` so the
/// zero-config path needs no file. Split out from `select` so it's testable
/// without mutating process env.
fn resolve(
	name: &str,
	model_override: Option<String>,
	registry: &BTreeMap<String, ProviderDef>,
) -> Result<(Box<dyn Provider>, String)> {
	if let Some(def) = registry.get(name) {
		return build_from_def(name, def, model_override);
	}
	// Built-in defaults (no file, or a name the file doesn't define).
	match name {
		"omlx" => Ok((
			Box::new(OpenAiProvider::omlx()),
			model_override.unwrap_or_else(|| crate::MODEL.to_string()),
		)),
		"deepseek" => Ok((
			Box::new(OpenAiProvider::deepseek()?),
			model_override.unwrap_or_else(|| "deepseek-v4-pro".to_string()),
		)),
		other => bail!(
			"unknown provider {other:?} — set HARNESS_PROVIDER to a built-in (omlx, deepseek) \
			 or define [{other}] in providers.toml"
		),
	}
}

/// Construct a provider from a file entry. The key is read from the named env var
/// at this point (never earlier, never from the file).
fn build_from_def(
	name: &str,
	def: &ProviderDef,
	model_override: Option<String>,
) -> Result<(Box<dyn Provider>, String)> {
	let model = model_override.unwrap_or_else(|| def.model.clone());
	let provider: Box<dyn Provider> = match def.dialect.as_str() {
		"omlx" => Box::new(OpenAiProvider::new(&def.base_url, key(def)?, name, Dialect::Omlx)),
		"deepseek" => {
			Box::new(OpenAiProvider::new(&def.base_url, key(def)?, name, Dialect::DeepSeek))
		}
		"anthropic" => Box::new(AnthropicProvider::new(&def.base_url, key(def)?)),
		other => bail!(
			"provider [{name}]: unknown dialect {other:?} (known: omlx, deepseek, anthropic)"
		),
	};
	Ok((provider, model))
}

fn key(def: &ProviderDef) -> Result<String> {
	std::env::var(&def.key_env)
		.with_context(|| format!("env var {} (key_env) is not set", def.key_env))
}

/// Load the registry: built-in defaults plus any file entries layered on top.
/// File path = `HARNESS_PROVIDERS_CONFIG` if set, else `<root>/providers.toml`.
/// Built-ins (omlx/deepseek) are constructed in `resolve`, not stored here — the
/// map only holds *file* entries, which override the built-ins by name.
fn load_registry(root: &Path) -> Result<BTreeMap<String, ProviderDef>> {
	let path = match env_nonempty("HARNESS_PROVIDERS_CONFIG") {
		Some(p) => std::path::PathBuf::from(p),
		None => root.join("providers.toml"),
	};
	if !path.exists() {
		return Ok(BTreeMap::new());
	}
	let text = std::fs::read_to_string(&path)
		.with_context(|| format!("reading provider config {}", path.display()))?;
	parse_providers(&text).with_context(|| format!("parsing {}", path.display()))
}

fn env_nonempty(var: &str) -> Option<String> {
	std::env::var(var).ok().filter(|s| !s.is_empty())
}

#[derive(Default)]
struct PartialDef {
	base_url: Option<String>,
	key_env: Option<String>,
	model: Option<String>,
	dialect: Option<String>,
}

impl PartialDef {
	fn finish(self, name: &str) -> Result<ProviderDef> {
		Ok(ProviderDef {
			base_url: self.base_url.with_context(|| format!("[{name}]: missing base_url"))?,
			key_env: self.key_env.with_context(|| format!("[{name}]: missing key_env"))?,
			model: self.model.with_context(|| format!("[{name}]: missing model"))?,
			dialect: self.dialect.with_context(|| format!("[{name}]: missing dialect"))?,
		})
	}
}

/// Strict, zero-dep TOML subset: `[section]` headers and `key = "value"` pairs.
/// Blank lines and `#` comments are skipped; ANYTHING else is a loud error (we do
/// not silently tolerate malformed config — a typo'd key should fail the run, not
/// quietly drop a setting). Only the four known keys and double-quoted values are
/// accepted. This is intentionally not a general TOML parser.
fn parse_providers(text: &str) -> Result<BTreeMap<String, ProviderDef>> {
	let mut map = BTreeMap::new();
	let mut current: Option<(String, PartialDef)> = None;

	for (idx, raw) in text.lines().enumerate() {
		let ln = idx + 1;
		let line = raw.trim();
		if line.is_empty() || line.starts_with('#') {
			continue;
		}

		if let Some(inner) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
			// Flush the previous section before opening a new one.
			if let Some((nm, pd)) = current.take() {
				map.insert(nm.clone(), pd.finish(&nm)?);
			}
			let nm = inner.trim().to_string();
			if nm.is_empty() {
				bail!("line {ln}: empty section name `[]`");
			}
			if map.contains_key(&nm) {
				bail!("line {ln}: duplicate section [{nm}]");
			}
			current = Some((nm, PartialDef::default()));
			continue;
		}

		let (k, v) = line
			.split_once('=')
			.with_context(|| format!("line {ln}: expected `key = \"value\"` or `[section]`, got: {line}"))?;
		let key = k.trim();
		let val = parse_quoted(v.trim())
			.with_context(|| format!("line {ln}: value for `{key}` must be double-quoted"))?;
		let cur = current
			.as_mut()
			.with_context(|| format!("line {ln}: key `{key}` appears before any [section]"))?;
		match key {
			"base_url" => cur.1.base_url = Some(val),
			"key_env" => cur.1.key_env = Some(val),
			"model" => cur.1.model = Some(val),
			"dialect" => cur.1.dialect = Some(val),
			other => bail!("line {ln}: unknown key `{other}` (known: base_url, key_env, model, dialect)"),
		}
	}

	if let Some((nm, pd)) = current.take() {
		map.insert(nm.clone(), pd.finish(&nm)?);
	}
	Ok(map)
}

fn parse_quoted(s: &str) -> Result<String> {
	let inner = s
		.strip_prefix('"')
		.and_then(|x| x.strip_suffix('"'))
		.context("expected a double-quoted string")?;
	Ok(inner.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn def(base: &str, env: &str, model: &str, dialect: &str) -> ProviderDef {
		ProviderDef {
			base_url: base.into(),
			key_env: env.into(),
			model: model.into(),
			dialect: dialect.into(),
		}
	}

	#[test]
	fn parses_two_sections_with_comments_and_blanks() {
		let text = r#"
# the local default
[omlx]
base_url = "http://127.0.0.1:8000/v1"
key_env = "OMLX_KEY"
model = "Qwen3.6"
dialect = "omlx"

# cloud
[deepseek]
base_url = "https://api.deepseek.com"
key_env = "DEEPSEEK_API_KEY"
model = "deepseek-v4-pro"
dialect = "deepseek"
"#;
		let m = parse_providers(text).unwrap();
		assert_eq!(m.len(), 2);
		assert_eq!(m["omlx"], def("http://127.0.0.1:8000/v1", "OMLX_KEY", "Qwen3.6", "omlx"));
		assert_eq!(
			m["deepseek"],
			def("https://api.deepseek.com", "DEEPSEEK_API_KEY", "deepseek-v4-pro", "deepseek")
		);
	}

	#[test]
	fn missing_field_errors() {
		let text = "[x]\nbase_url = \"u\"\nkey_env = \"E\"\nmodel = \"m\"\n"; // no dialect
		let err = parse_providers(text).unwrap_err().to_string();
		assert!(err.contains("missing dialect"), "got: {err}");
	}

	#[test]
	fn unquoted_value_errors() {
		let text = "[x]\nbase_url = http://nope\n";
		let err = parse_providers(text).unwrap_err().to_string();
		assert!(err.contains("double-quoted"), "got: {err}");
	}

	#[test]
	fn unknown_key_errors() {
		let text = "[x]\nbsae_url = \"typo\"\n";
		let err = parse_providers(text).unwrap_err().to_string();
		assert!(err.contains("unknown key"), "got: {err}");
	}

	#[test]
	fn key_before_section_errors() {
		let text = "base_url = \"u\"\n";
		let err = parse_providers(text).unwrap_err().to_string();
		assert!(err.contains("before any [section]"), "got: {err}");
	}

	#[test]
	fn duplicate_section_errors() {
		let text = "[x]\nbase_url=\"u\"\nkey_env=\"E\"\nmodel=\"m\"\ndialect=\"omlx\"\n[x]\n";
		let err = parse_providers(text).unwrap_err().to_string();
		assert!(err.contains("duplicate section"), "got: {err}");
	}

	#[test]
	fn empty_text_is_empty_registry() {
		assert!(parse_providers("").unwrap().is_empty());
		assert!(parse_providers("\n\n# only a comment\n").unwrap().is_empty());
	}

	#[test]
	fn no_file_omlx_is_default() {
		let empty = BTreeMap::new();
		let (p, model) = resolve("omlx", None, &empty).unwrap();
		assert_eq!(p.name(), "omlx");
		assert_eq!(model, crate::MODEL);
	}

	#[test]
	fn no_file_model_override_applies() {
		let empty = BTreeMap::new();
		let (_p, model) = resolve("omlx", Some("custom-model".into()), &empty).unwrap();
		assert_eq!(model, "custom-model");
	}

	#[test]
	fn unknown_provider_errors() {
		let empty = BTreeMap::new();
		let err = resolve("nope", None, &empty).map(|_| ()).unwrap_err().to_string();
		assert!(err.contains("unknown provider"), "got: {err}");
	}

	#[test]
	fn file_entry_unknown_dialect_errors() {
		let mut reg = BTreeMap::new();
		reg.insert("x".to_string(), def("http://u", "OMLX_KEY", "m", "klingon"));
		// An unknown dialect bails before the key env var is ever read.
		let err = resolve("x", None, &reg).map(|_| ()).unwrap_err().to_string();
		assert!(err.contains("unknown dialect"), "got: {err}");
	}
}
