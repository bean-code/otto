<p align="center">
  <img src="docs/otto-banner.png" alt="Otto - autopilot for your engineering workflow" width="720" />
</p>

<p align="center">
  <a href="https://github.com/bean-code/otto/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/bean-code/otto/actions/workflows/ci.yml/badge.svg" /></a>
  <img alt="platforms" src="https://img.shields.io/badge/platforms-linux%20%7C%20macos%20%7C%20windows-6ea8fe" />
  <img alt="license" src="https://img.shields.io/badge/license-MIT-e8a33d" />
</p>

# Otto

Otto is a Jarvis-style personal automation assistant for a working engineer: a natural-language command bar - as
a desktop app or a CLI, whichever fits the moment - that turns a plain-English request into real, typed actions
against the tools you actually use, instead of another chat window you have to translate your intent into
manually. Say "show me my open PRs" and Otto doesn't describe how it would check; it checks, via GitHub's real
API, and hands back the actual list. That's the whole design premise: every capability is a typed `Skill` with a
real implementation behind it, Claude decides *which* one a request needs, and the result you get back is data
your dashboard or terminal can render, not just prose to read.

Today that means GitHub - list open PRs, check CI/check-run status, open a PR from a branch - plus a first pass
at the things around the edges of "get my engineering work done": voice in and out, a working-hours schedule so
Otto isn't always-on, and a small daily nudge about your own PR habits. Deployments and calendar management are
scaffolded and named in the roadmap below, not implemented yet - see the crate table for exactly what's real
versus staged.

## Highlights

- **GitHub, for real.** Open PRs, CI status, opening a PR from a branch - not a description of what it would do,
  an actual `octocrab` call against the GitHub API.
- **One agent loop, any provider.** The Claude round-trip and tool-calling loop are delegated to
  [`llmprism`](https://crates.io/crates/llmprism) instead of hand-rolled - swap providers or add middleware
  without touching `otto-core` or any skill.
- **A persona that isn't generic.** The system prompt carries who you are (see `otto-core::DEFAULT_PERSONA`), so
  replies land at the altitude of a peer conversation, not a first-time-user tutorial.
- **Voice in, voice back - fully local.** Push-to-talk mic capture, `whisper.cpp` transcription, OS-native
  text-to-speech for replies. Nothing audio-related leaves the machine; no cloud STT/TTS vendor in the loop.
- **A schedule, not an always-on process.** Tell Otto its working hours; it turns itself off outside them instead
  of sitting there listening around the clock.
- **A quiet nudge toward getting better.** A background digest looks at your own recent PR activity and
  surfaces one grounded, specific observation - not generic advice, not a leaderboard.
- **Cross-platform by construction.** CI builds, tests, and smoke-runs the whole workspace on Linux, macOS, and
  Windows on every push - see the badge above.
- **Set up from inside the app.** A native Settings panel (also reachable from the OS menu bar) writes your
  Claude/GitHub credentials straight to the keychain - no editing a config file or restarting to pick them up.
- **Real, queryable activity feed.** Every command, growth-digest note, and schedule event is persisted to a
  local SQLite database and readable back from an Activities tab in the app - built that way specifically so a
  future API layer over `otto-core` has real data to serve, not a log file to re-parse.
- **A model picker, not a config flag.** Switch between Claude models from a card grid in the app (each with a
  real tradeoff, not just a name) - the choice persists across restarts and takes effect on your very next
  command, no editing an env var.
- **Extensible on purpose.** A skill is a small Rust crate implementing one trait - no plugin runtime to trust,
  no manifest to write. [`docs/SKILLS.md`](docs/SKILLS.md) walks through building one end to end, and every
  registered skill shows up in the app's Skills tab automatically.

## How it's put together

A Cargo workspace of small, single-purpose crates rather than one monolith - each skill is independently
testable and the orchestration layer (`otto-core`) never needs to know a skill's internals, only its `Skill`
trait surface:

| Crate | What it is |
|---|---|
| `otto-core` | Orchestration: the `Skill` trait, `SkillRegistry` (dispatches tool calls by name, groups by skill for the Skills tab), `AgentLoop`, `CredentialStore` (OS keychain), `ActivityLog`/`SqliteStore` (local SQLite - queryable, not a flat log; also holds small persisted settings like the selected model), `ActiveWindow` (working-hours scheduling), `MODEL_CATALOG`, the growth-digest job. |
| `otto-skill-github` | **Fully working.** List open PRs, check CI/check-run status, open a PR from a branch. Built on [`octocrab`](https://docs.rs/octocrab). |
| `otto-skill-deploy` | Scaffolded (compiles, tools defined) but not yet implemented - intended for triggering/monitoring GitHub Actions workflow runs. |
| `otto-skill-calendar` | Scaffolded, not yet implemented - intended for Google Calendar (picked over Outlook as the starting provider). |
| `otto-skill-notify` | **Fully working.** Desktop notifications via [`notify-rust`](https://docs.rs/notify-rust). |
| `otto-voice` | **Fully working.** Local, offline voice I/O: mic capture (`cpal`) + speech-to-text (`whisper.cpp` via `whisper-rs`) for prompts, OS-native text-to-speech (`tts`) for talk-back. Nothing audio-related ever leaves the machine. |
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

**In `otto-ui`, the easiest way is Settings** - the gear icon in the toolbar, or "Otto > Settings..." in the
native menu bar. Paste a key in and hit Save; it's written straight to the keychain and picked up immediately
(the app rebuilds its GitHub/Claude clients on save - no restart needed). The toolbar's status pills show at a
glance whether each credential is configured. `otto-ui` no longer requires the Claude key to exist before it'll
even open - launch it with nothing configured and it starts fine, `run_command` just replies with a clear
"add your Claude API key in Settings" instead of a working answer until you do.

For `otto-cli`, or to script it, set them with any keychain tool that writes to the `otto-assistant` service, or
from Rust:

```rust
use otto_core::{CredentialStore, KeyringStore};

let store = KeyringStore::new();
store.set("anthropic-api-key", "sk-ant-...").await?;
store.set("github-token", "ghp_...").await?;
```

**No keychain daemon available (headless Linux/WSL with no Secret Service running)?** A keychain miss falls back to
reading `OTTO_<KEY>` from the process environment (dashes become underscores), so for local dev you can instead
export them, or use the `.env` file described next.

### `.env` for local development

`otto-cli` and `otto-ui` both load a `.env` file (via [`dotenvy`](https://docs.rs/dotenvy), searched from the
current directory upward) as the very first thing they do, before anything reads an environment variable - so it
covers credentials *and* every other `OTTO_*` setting below (model, persona, schedule, voice, growth interval).
Copy the template and fill it in:

```sh
cp .env.example .env
```

`.env` is gitignored - it's local-only and never committed. `.env.example` is the committed reference for every
variable Otto reads and what each one defaults to; keep it in sync if you add a new one.

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
OS speech engine, and OS audio stack respectively - CI installs exactly this list on `ubuntu-latest` and nothing
extra on the other two platforms):

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

**What's actually in the window:**

- **A native OS menu bar** (`tauri::menu`, wired in `otto-ui`'s `.setup()`) - an "Otto" menu with Settings and
  Quit, and a standard Edit menu so cut/copy/paste and their OS keyboard shortcuts work in text fields. This is
  genuinely native chrome (`AppKit`/`GTK`/`Win32` menus, not anything drawn in the webview) - Settings there and
  the in-window gear button both open the same panel, just via different triggers (the menu item emits a
  `open-settings` event the frontend listens for). The window itself keeps its normal OS title bar and
  decorations - "native" means real OS menu/window chrome, not a custom-drawn titlebar replacing it.
- **Four tabs**: **Dashboard** (the PR/deployment/meeting cards, command bar, voice, and a time-of-day greeting -
  "Good morning/afternoon/evening" plus your OS account name, read via `get_greeting_name`), **Activities** (a
  single feed - commands, growth-digest notes, schedule events - via `list_activities`, see History &
  persistence above), **Models** (a card picker over `otto-core::MODEL_CATALOG`, not a dropdown - each model's a
  card with a tier badge and a one-line tradeoff; clicking one calls `set_model`, which persists the choice via
  `SqliteStore`'s settings table and takes effect on the next command, no restart), and **Skills** (every
  registered skill and the tools it exposes, via `list_skills` - see [`docs/SKILLS.md`](docs/SKILLS.md) if you
  want to add one).
- **A light, card-based visual language** modeled on a Google Material-style dashboard mockup in `docs/ux.webp` -
  white surfaces, soft shadows, a card grid for anything selectable (models, skills, the dashboard panels), and a
  greeting header with a brand-colored accent flourish, in place of the earlier dark theme.
- **Toolbar status pills** for each credential (Claude/GitHub) that flip from amber "not set" to green
  "configured" the moment Settings saves - `get_credential_status` never returns the values themselves, only
  whether each is set.
- **Toast notifications** for every command's outcome (success, failure, and the "finish setting up" nudge on
  first launch if a credential is missing) - in addition to, not instead of, the OS-level desktop notifications
  `notify_desktop` can send.

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

### Activities & persistence

Everything Otto does - a command (input, reply, and the structured `steps` data each skill returned), a
growth-digest note, a schedule shutdown - is written to one local SQLite database (`otto_core::SqliteStore`, via
[`rusqlite`](https://docs.rs/rusqlite)'s `bundled` feature, so there's no system `libsqlite3` dependency to
install) at `otto.sqlite3` in the OS data directory, and read back by `otto-ui`'s Activities tab through
`ActivityLog::list_recent`. This is deliberately not a flat log: the whole point of `list_recent` existing as a
real query (not just an appender) is that the Activities tab, and anything built on `otto-core` later - a REST
API serving the same engine headlessly, say - gets an actual `ORDER BY id DESC LIMIT ?`, not a growing file to
parse front to back on every request. `SqliteStore` also holds a small `settings` table for things that need to
survive a restart but aren't secrets - the selected model, today - so there's one local database file, not a
database plus a scattering of config files.

The `activities` table's schema is intentionally simple (`kind` + `title` + `detail` + a `steps_json` column,
empty for anything that isn't a command) - normalizing `steps` into its own table with a foreign key is the
natural next step once something needs to query *across* steps (e.g. "every time `github_open_pr` was called").

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
4. The turn is written to `otto.sqlite3` in the OS data directory (`~/.local/share/otto` on Linux) - a real table
   (`id`, `timestamp`, `kind`, `title`, `detail`, `steps_json`), not a flat log, so `otto-ui`'s Activities tab
   (and any future API server) can query it directly instead of re-parsing an ever-growing file.

None of this is provider-specific plumbing you'd have to rewrite for a different model or a different skill: step
1 comes from whatever's registered in `SkillRegistry`, step 2 is `llmprism`'s tool loop calling back into that
same registry, and step 3's shape (`reply` + structured `steps`) is what every skill's result looks like, not
something special-cased for GitHub.

## Branding

The logo and banner in `docs/` (`otto-banner.{png,svg}`, `otto-logo-mark.{png,svg}`) are the source design assets
- an artificial-horizon mark (amber "sky" over navy "ground", the way an autopilot's attitude indicator reads),
which is also what actually ships as the desktop app's icon: `crates/otto-ui/icons/icon.png` (used by Tauri's
`generate_context!()` as the runtime window icon on every platform) and `crates/otto-ui/icons/icon.ico`
(Windows-only - a separate multi-resolution 256/64/32/16 file `tauri-build` needs specifically to embed as the
`.exe`'s Windows resource icon) are both generated straight from `docs/otto-logo-mark.png`. If you update the
source mark, regenerate both rather than editing them by hand.

`docs/ux.webp` is the visual reference the current UI was built against - a light, card-grid dashboard mockup
with a personalized greeting header. The app's light theme, card patterns (dashboard panels, the model picker,
the skills browser), and greeting hero are a direct translation of that reference into Otto's own brand colors
(amber/navy/blue rather than the mockup's own palette) rather than a generic redesign.

## CI

`.github/workflows/ci.yml` builds, tests, and smoke-runs the whole workspace on Linux, macOS, and Windows on every
push/PR to `main` - see the badge at the top of this file for the current status. "Smoke-runs" means
`otto-cli --help` specifically - a real functional run needs live Claude/GitHub credentials the workflow doesn't
have, so this only proves the built binary actually starts on each platform (clap's `--help` exits before anything
needs a credential). `otto-ui` isn't launched in CI: unlike the CLI it has no credential-free short-circuit (it
builds its full `AgentLoop` before a window ever opens) and would also need a virtual display on Linux - both
build-verified via the main build step, just not launched.

## Status

`cargo check --workspace` is clean on Linux (with the apt packages above installed), and CI confirms the same
build + test + run sequence passes on macOS and Windows too. The one platform-specific bug CI caught that a
Linux-only check couldn't: Windows needs `crates/otto-ui/icons/icon.ico` specifically (`tauri-build` panics
without it - "required for generating a Windows Resource file"), which is a separate requirement from the
`icon.png` `generate_context!()` needs on every platform; see the Branding section above for where both come
from now.

`cargo test --workspace` passes too - including real (not just compiled) coverage of `SqliteStore` in
`crates/otto-core/src/store.rs`: recording and listing commands newest-first, respecting the `limit`, mixing
commands with growth-digest/schedule events in one feed, and a settings round-trip/overwrite - all against a
temp on-disk database, not mocks.

Known gaps, tracked rather than hidden: `otto-skill-deploy` and `otto-skill-calendar` are scaffolded but not
wired up, the dashboard frontend is plain HTML/JS rather than the originally-requested Dioxus (see the Desktop
app section for why and how to swap it in), the growth digest only looks at open PRs on one repo, and the
`otto-ui` desktop window itself isn't visually verified in this environment (no display server here - see the
Desktop app section for what's in it; `cargo build`/`cargo test` both pass, but only a real screen confirms it
looks and behaves as described). None of these are blocked on anything - they're the natural next slice of work.
