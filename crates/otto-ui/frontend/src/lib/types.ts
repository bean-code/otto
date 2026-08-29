// Mirrors the `Serialize` structs on the Rust side (`otto-core`'s `agent.rs`,
// `store.rs`, `providers.rs`, `models.rs`, `registry.rs`, `skill.rs`, and the
// locally-defined types in `otto-ui/src/lib.rs`). Keep field names/shapes in
// exact sync with those - this is the frontend's only source of truth for
// what the backend sends.

export interface AgentStep {
  tool_name: string;
  summary: string;
  data: unknown;
}

export interface AgentTurn {
  reply: string;
  steps: AgentStep[];
  provider: string;
  model: string;
}

export type ActivityKind = "command" | "growth_digest" | "schedule";

export interface ActivityEntry {
  id: number;
  timestamp: string;
  kind: ActivityKind;
  title: string;
  detail: string;
  steps: AgentStep[];
  provider: string;
  model: string;
}

export interface ProviderInfo {
  id: string;
  name: string;
  needs_api_key: boolean;
}

export type ModelTier = "flagship" | "balanced" | "fast" | "frontier";

export interface ModelInfo {
  provider: string;
  id: string;
  name: string;
  tagline: string;
  tier: ModelTier;
}

export interface SelectedModel {
  provider: string;
  model: string;
}

export interface CredentialStatus {
  key: string;
  label: string;
  configured: boolean;
}

export interface ToolSpec {
  name: string;
  description: string;
  input_schema: unknown;
}

export interface SkillInfo {
  id: string;
  tools: ToolSpec[];
}

// Shape of a GitHub PR, as it appears in `AgentStep.data.pull_requests` for
// a `github_list_open_prs` step. Not a Rust-side Serialize struct (it's
// dynamic `serde_json::Value`) - this is the frontend's own contract with
// the github skill's tool output shape.
export interface PullRequest {
  number: number;
  title?: string;
  head?: string;
  base?: string;
  author?: string;
  draft?: boolean;
}

export interface GithubPrStepData {
  pull_requests?: PullRequest[];
  repo?: string;
}

// Mirrors `otto_core::profiles::Profile` and `otto-ui/src/lib.rs`'s
// `Preferences`/`DeviceCodePayload`.

export interface Profile {
  id: string;
  display_name: string;
  avatar_url: string | null;
  github_login: string | null;
  created_at: string;
}

export interface Preferences {
  persona: string;
  talk_back_default: boolean;
  notifications_enabled: boolean;
}

export interface DeviceCodePayload {
  user_code: string;
  verification_uri: string;
}
