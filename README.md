# Otto

Otto is a Jarvis-style personal automation assistant: a natural-language command bar (desktop app or CLI) that
dispatches to typed integrations - GitHub PRs today, deployments and calendar management to follow - via the
Claude API.

## How it's put together

A Cargo workspace of small crates:

| Crate | What it is |
|---|---|
| `otto-core` | Orchestration: the `Skill` trait, `SkillRegistry` (dispatches tool calls by name), `AgentLoop`, `CredentialStore` (OS keychain), `HistoryStore` (local JSONL log). |
| `otto-skill-github` | **Fully working.** List open PRs, check CI/check-run status, open a PR from a branch. Built on [`octocrab`](https://docs.rs/octocrab). |
| `otto-skill-deploy` | Scaffolded (compiles, tools defined) but not yet implemented - intended for triggering/monitoring GitHub Actions workflow runs. |
| `otto-skill-calendar` | Scaffolded, not yet implemented - intended for Google Calendar (picked over Outlook as the starting provider). |
| `otto-skill-notify` | **Fully working.** Desktop notifications via [`notify-rust`](https://docs.rs/notify-rust). |
| `otto-voice` | **Fully implemented, compiler-unverified** (see Status). Local, offline voice I/O: mic capture (`cpal`) + speech-to-text (`whisper.cpp` via `whisper-rs`) for prompts, OS-native text-to-speech (`tts`) for talk-back. Nothing audio-related ever leaves the machine. |
| `otto-ui` | The Tauri desktop app: a dashboard (PR / deployment / meeting panels), a command bar, push-to-talk voice input, and talk-back. |
| `otto-cli` | Thin CLI (`otto "show me my open PRs"`) driving the same `AgentLoop`. |

### The agent loop

`otto-core::AgentLoop` doesn't talk to Claude directly - it hands the request to
[`llmprism`](https://crates.io/crates/llmprism), a Rust LLM-provider abstraction (Anthropic, OpenAI, and others
behind one interface), with every registered skill's tools exposed as `llmprism::Tool`s. `llmprism` runs the actual
model <-> tool round-trips (Claude decides to call `github_list_open_prs`, `llmprism` runs it via `SkillRegistry`,
sends the result back, repeats up to a step cap); `AgentLoop` just records what happened for the UI/history and
returns the final reply. Swapping providers, or wrapping one in middleware (logging, a default system prompt,
response caching), is entirely an `llmprism::Registry` concern - nothing in `otto-core` or any skill needs to
change.

### Persona

The system prompt has two parts: fixed operational instructions (`otto-core::agent::SYSTEM_PROMPT` - what Otto is
and how to use tools), and a persona paragraph appended after it (`otto-core::DEFAULT_PERSONA`) that tells Otto who
it's talking to, so replies land at the right technical altitude and tone instead of a generic assistant voice.
Override it per-user with `OTTO_PERSONA` (both `otto-cli` and `otto-ui` read it the same way); an empty string
disables the persona addition entirely.

## Configuring credentials

Nothing is ever written to a plaintext config file. Credentials are read through `otto-core::KeyringStore`, which
stores/reads secrets in the OS keychain via the [`keyring`](https://docs.rs/keyring) crate, under the service name
`otto-assistant`. Two keys are used today:

- `anthropic-api-key` - your Claude API key.
- `github-token` - a GitHub personal access token (needs `repo` scope for private repos; `public_repo` is enough
  for public ones) with permission to read PRs/checks and open PRs on the repos you point it at.

Set them with any keychain tool that writes to the `otto-assistant` service, or from Rust:

```rust
use otto_core::{CredentialStore, KeyringStore};

let store = KeyringStore::new();
store.set("anthropic-api-key", "sk-ant-...").await?;
store.set("github-token", "ghp_...").await?;
```

**No keychain daemon available (headless Linux/WSL with no Secret Service running)?** A keychain miss falls back to
reading `OTTO_<KEY>` from the process environment (dashes become underscores), so for local dev you can instead
export:

```sh
export OTTO_ANTHROPIC_API_KEY=sk-ant-...
export OTTO_GITHUB_TOKEN=ghp_...
```

## Running it

### CLI

```sh
cargo run -p otto-cli -- "show me my open PRs"
cargo run -p otto-cli -- --repo owner/name "open a PR from my-branch"
```

`--repo` (or `OTTO_DEFAULT_REPO`) sets the default `owner/name` GitHub commands fall back to when a command doesn't
name one. `--model` (or `OTTO_MODEL`) overrides the Claude model; defaults to `claude-opus-5`.

### Desktop app (Tauri)

The Linux build needs several system dev packages first (already present on macOS/Windows via the OS webview,
OS speech engine, and OS audio stack respectively):

```sh
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf libxdo-dev libssl-dev build-essential pkg-config \
  libasound2-dev speech-dispatcher libspeechd-dev cmake
```

(The last line is for `otto-voice`: `libasound2-dev` for mic capture via `cpal`/ALSA, `speech-dispatcher` +
`libspeechd-dev` for talk-back via the `tts` crate, `cmake` to build `whisper.cpp` via `whisper-rs`.)

Then:

```sh
cargo run -p otto-ui
```

The dashboard's frontend (`crates/otto-ui/frontend/dist/index.html`) is hand-written HTML/CSS/JS - it calls the
Rust backend through `window.__TAURI__.core.invoke("run_command", { input })` (`app.withGlobalTauri` is on in
`tauri.conf.json`, so no JS bundler or npm package is needed). This is a deliberate simplification, not the final
shape: the original brief asked for a Rust-first frontend (Dioxus/Leptos) compiled through `trunk`, but `trunk`
failed to build from source in this environment (a `lightningcss` compile error unrelated to this project) and
neither `trunk` nor the `wasm32-unknown-unknown` target/`dioxus-cli` were preinstalled. Swapping in a real Dioxus
frontend is a matter of adding a crate under `crates/otto-ui/frontend/`, pointing `tauri.conf.json`'s
`build.frontendDist` at its `trunk build` output, and wiring `beforeDevCommand`/`beforeBuildCommand` to
`trunk serve`/`trunk build` - the Rust backend (commands, state, ACL) doesn't need to change.

### Voice: setup

Voice is optional - `otto-ui` starts fine without it, the mic button just reports "voice input isn't available"
if the model is missing. To turn it on:

1. Install the Linux system packages above (or nothing extra on macOS/Windows).
2. Download a whisper.cpp GGML model - `base.en` is a good balance of speed/accuracy for a command bar:
   ```sh
   mkdir -p ~/.local/share/otto/models
   curl -L -o ~/.local/share/otto/models/ggml-base.en.bin \
     https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin
   ```
   (`otto-voice::default_model_path()` looks there by default; set `OTTO_WHISPER_MODEL=/path/to/model.bin` to
   point elsewhere, or to a multilingual/larger model.)
3. Run `otto-ui`. Hold the mic button to record, release to transcribe and auto-run; check "Talk back" to have
   replies spoken aloud through the OS's native TTS voice.

`otto-voice` runs mic capture and TTS on one dedicated OS thread (both `cpal::Stream` and `tts::Tts` are `!Send`,
so they can't live in async task state) and talks to the rest of the app over a channel - see the module docs in
`crates/otto-voice/src/lib.rs`.

### Scheduling

Otto can turn itself off outside working hours. Set both:

```sh
export OTTO_ACTIVE_FROM=09:00    # HH:MM, 24h, local time
export OTTO_ACTIVE_UNTIL=18:00   # supports overnight windows too, e.g. 22:00 -> 06:00
```

If `otto-ui` is launched outside that window, it shows a "back during scheduled hours" notification and exits
immediately rather than sitting there with a dashboard you're not supposed to be using. If launched inside the
window, it runs normally and exits itself the moment the window closes. **Otto doesn't relaunch itself** - use an
OS-level scheduler (`crontab -e` / `launchd` / Task Scheduler) to start it at `OTTO_ACTIVE_FROM` each day; Otto's
own code only ever handles the "turn off" half. This split (OS starts it, Otto stops itself) avoids building a
second always-on daemon just to relaunch a GUI app.

### Growth digest

While running, `otto-ui` periodically (`OTTO_GROWTH_INTERVAL_HOURS`, default `24`) looks at your current open-PR
activity and asks Claude for one short, specific observation grounded in that data - then sends it as a desktop
notification if it found something worth saying (see `otto_core::run_growth_digest`). It's intentionally narrow
for now: only open PRs on the default repo, not a full cross-repo activity history - extending that is a matter of
growing `otto-skill-github`'s tool surface, not the digest logic itself.

## What "PR round-trip" looks like

Typing `show me my open PRs` in either the CLI or the command bar:

1. `AgentLoop::run` builds one `llmprism::Tool` per registered skill tool (`github_list_open_prs`,
   `github_pr_ci_status`, `github_open_pr`, `notify_desktop`, the `deploy_*`/`calendar_*` stubs) and sends the
   prompt + tool definitions to Claude.
2. Claude responds with a `github_list_open_prs` tool call; `llmprism`'s tool loop dispatches it through
   `SkillRegistry` to `GithubSkill`, which calls the GitHub REST API via `octocrab` and returns a summary + the
   structured PR list.
3. The result goes back to Claude, which writes a final reply; `AgentLoop` returns `{ reply, steps }` - `steps`
   carries the structured data (used by the UI to render the PR panel) alongside the summary.
4. The turn is appended to `history.jsonl` in the OS data directory (`~/.local/share/otto` on Linux).

## Status

`cargo check --workspace` is clean - every crate (`otto-core`, all four `otto-skill-*` crates, `otto-voice`,
`otto-ui`, `otto-cli`) compiles, on Linux with the apt packages above installed.

One gotcha if you ever regenerate or delete `crates/otto-ui/icons/icon.png`: Tauri's `generate_context!()` macro
requires a real RGBA PNG there unconditionally (not just when bundling) - it's the default window icon on every
platform, decoded and embedded at compile time, and panics with an "icon not found" / "icon is not RGBA" proc-macro
error otherwise. The checked-in icon is a placeholder (a solid accent-blue circle); swap it for real artwork
whenever you like, just keep it PNG + RGBA (8-bit, color type 6).
