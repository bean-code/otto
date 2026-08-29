import { invoke } from "@tauri-apps/api/core";

import type {
  ActivityEntry,
  AgentTurn,
  CredentialStatus,
  ModelInfo,
  ProviderInfo,
  SelectedModel,
  SkillInfo,
} from "@/lib/types";

// The only module allowed to call the raw Tauri `invoke` - every backend
// command gets a typed wrapper here so the rest of the app never guesses at
// argument/return shapes. Names and signatures match
// `crates/otto-ui/src/lib.rs`'s `#[tauri::command]` functions exactly.

export function runCommand(input: string): Promise<AgentTurn> {
  return invoke<AgentTurn>("run_command", { input });
}

export function listActivities(limit = 100): Promise<ActivityEntry[]> {
  return invoke<ActivityEntry[]>("list_activities", { limit });
}

export function listProviders(): Promise<ProviderInfo[]> {
  return invoke<ProviderInfo[]>("list_providers");
}

export function listModels(): Promise<ModelInfo[]> {
  return invoke<ModelInfo[]>("list_models");
}

export function getSelectedModel(): Promise<SelectedModel> {
  return invoke<SelectedModel>("get_selected_model");
}

export function setSelectedModel(provider: string, model: string): Promise<void> {
  return invoke<void>("set_selected_model", { provider, model });
}

export function listSkills(): Promise<SkillInfo[]> {
  return invoke<SkillInfo[]>("list_skills");
}

export function getGreetingName(): Promise<string | null> {
  return invoke<string | null>("get_greeting_name");
}

export function getCredentialStatus(): Promise<CredentialStatus[]> {
  return invoke<CredentialStatus[]>("get_credential_status");
}

export function setCredential(key: string, value: string): Promise<void> {
  return invoke<void>("set_credential", { key, value });
}

export function startRecording(): Promise<void> {
  return invoke<void>("start_recording");
}

export function stopRecording(): Promise<string> {
  return invoke<string>("stop_recording");
}

export function speak(text: string): Promise<void> {
  return invoke<void>("speak", { text });
}
