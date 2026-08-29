# Building a skill

A skill is how Otto gains a new capability - GitHub, deployments, notifications, and (once wired up) calendars
are all skills. There's no plugin runtime, no dynamic loading, no sandboxing to reason about: a skill
is a normal Rust crate in the workspace that implements one trait. This is a deliberate choice, not a stopgap -
compile-time extensibility is simpler, faster, and more trustworthy than a runtime plugin system, and nothing
about the design below rules one out later if it's ever actually needed.

If you just want to see a finished example, read `crates/otto-skills/notify/src/lib.rs` - it's the shortest real
skill in the workspace and every step below walks through it.

## The mental model

The agent loop (`otto-core::AgentLoop`) never knows what a skill *is*. It knows three things:

1. A skill has a list of **tools** - named, described, JSON-schema-typed operations the model can choose to call.
2. Calling a tool returns a **`SkillOutcome`**: a `summary` (plain text fed back to Claude) and `data` (structured
   JSON the UI renders).
3. Skills are registered into a **`SkillRegistry`**, which is what actually gets tool calls dispatched to them.

Claude never sees your Rust types - only the tool's name, description, and JSON schema. Write those as if you're
briefing a competent engineer who's never seen your code: the description is the only thing standing between the
model calling your tool correctly and not calling it at all.

## The `Skill` trait

```rust
#[async_trait]
pub trait Skill: Send + Sync {
    /// A short, stable id - used for logging and for grouping in the
    /// Skills tab. Lowercase, no spaces: "github", "notify", "deploy".
    fn id(&self) -> &'static str;

    /// Every tool this skill exposes to the agent loop.
    fn tools(&self) -> Vec<ToolSpec>;

    /// Handle one call. `tool_name` is always one of the names you
    /// returned from `tools()` - route on it if you expose more than one.
    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome>;
}
```

`ToolSpec::new(name, description, input_schema)` takes a raw `serde_json::json!` value for the schema - standard
JSON Schema, the same shape Anthropic's Messages API expects for a tool definition (see `otto-skill-github` for a
schema with several fields and a required subset).

## Building one, step by step

### 1. Scaffold the crate

```sh
mkdir -p crates/otto-skills/<name>/src
```

`crates/otto-skills/<name>/Cargo.toml`:

```toml
[package]
name = "otto-skill-<name>"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
otto-core = { workspace = true }
tokio = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
anyhow = { workspace = true }
```

Add it to the workspace: `crates/otto-skills/<name>` in the root `Cargo.toml`'s `members`, and
`otto-skill-<name> = { path = "crates/otto-skills/<name>" }` in `[workspace.dependencies]`.

### 2. Implement the trait

```rust
use async_trait::async_trait;
use otto_core::{CoreError, CoreResult, Skill, SkillOutcome, ToolSpec};
use serde_json::{json, Value};

pub struct MySkill;

impl MySkill {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Skill for MySkill {
    fn id(&self) -> &'static str { "myskill" }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![ToolSpec::new(
            "my_tool_name",
            "One sentence Claude will read to decide whether this is the right tool for the user's request.",
            json!({
                "type": "object",
                "properties": {
                    "some_field": { "type": "string", "description": "What this argument means." }
                },
                "required": ["some_field"],
                "additionalProperties": false
            }),
        )]
    }

    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome> {
        if tool_name != "my_tool_name" {
            return Err(CoreError::SkillNotFound(tool_name.to_string()));
        }

        let some_field = input
            .get("some_field")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::InvalidInput {
                tool: tool_name.into(),
                message: "missing required field 'some_field'".into(),
            })?;

        // ... do the actual work ...

        Ok(SkillOutcome::new(
            format!("Did the thing with {some_field}"), // fed back to Claude as plain text
            json!({ "some_field": some_field }),          // structured data the UI can render
        ))
    }
}
```

Conventions worth keeping, all visible in `otto-skill-notify` and `otto-skill-github`:

- **Tool names are globally unique** across every skill - `SkillRegistry::register` panics at startup if two
  skills claim the same name, on purpose (a silent collision would be far worse than a loud one).
- **`additionalProperties: false` and an explicit `required` array** on every schema - it's the difference between
  Claude sending well-formed arguments and you writing defensive parsing code for malformed ones.
- **Validate input, don't `unwrap`** - a missing/malformed field should return `CoreError::InvalidInput`, not
  panic. The model sees the error message and can retry with corrected arguments; a panic just kills the request.
- **`summary` is prose, `data` is structure** - `summary` becomes the literal `tool_result` content Claude reads
  next, so keep it short and information-dense. `data` is never shown to Claude directly; it's there for the UI
  (see how `otto-ui`'s dashboard renders `github_list_open_prs`'s `data.pull_requests`).
- **Errors go through `CoreError`**, not `anyhow` - `CoreError::SkillFailed { skill, message }` for "the operation
  itself failed" (an API call errored, a file couldn't be read), `CoreError::InvalidInput` for "the model's
  arguments don't make sense". Both get surfaced to Claude as a tool error it can react to.

### 3. Register it

In `otto-cli/src/main.rs` and `otto-ui/src/lib.rs` (`build_runtime`), alongside the existing skills:

```rust
registry.register(Arc::new(MySkill::new()));
```

That's the entire integration surface. No manifest, no discovery mechanism - registering it is what makes its
tools visible to the agent loop, and it shows up in the desktop app's Skills tab automatically (via
`SkillRegistry::skills()` - grouped by `id()`, nothing extra to wire up there).

### 4. Test it

Skills are ordinary structs - test `call()` directly without needing a live LLM:

```rust
#[tokio::test]
async fn returns_expected_outcome() {
    let skill = MySkill::new();
    let outcome = skill.call("my_tool_name", json!({ "some_field": "value" })).await.unwrap();
    assert_eq!(outcome.data["some_field"], "value");
}
```

For a skill that calls an external API (like `otto-skill-github`'s use of `octocrab`), keep the HTTP client
construction lazy (see `GithubSkill`'s `OnceCell`-based client, built from a `CredentialStore` on first use) so
tests that only exercise input validation don't need real credentials or network access.

## Credentials

If your skill needs a secret, take an `Arc<dyn CredentialStore>` in its constructor (see `GithubSkill::new`) -
never read an API key from an env var directly inside a skill. That keeps every credential going through the one
keychain-backed path (`otto-core::KeyringStore`), which is also what makes them settable from `otto-ui`'s
Settings panel without your skill needing to know anything about that UI.

## What you get for free

Once a skill is registered, nothing else in the system needs to change to pick it up:

- The agent loop exposes its tools to Claude automatically (`AgentLoop` builds the tool list from
  `SkillRegistry::tool_specs()` on every call).
- Every call your skill handles is recorded to the Activities feed (`ActivityLog::record_command`), with your
  `SkillOutcome`'s `data` preserved in `steps` - readable back later, including by a future API layer over
  `otto-core`.
- It shows up in the desktop app's Skills tab, and its tools are individually visible there.

That's the whole point of the trait boundary: `otto-core` and `otto-ui` were both written against `Skill`, never
against `GithubSkill` or `NotifySkill` specifically.
