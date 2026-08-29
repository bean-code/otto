import { invoke } from "@tauri-apps/api/core";

import type {
  ActivityEntry,
  AgentTurn,
  CredentialStatus,
  ModelInfo,
  Preferences,
  Profile,
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

/**
 * Runs the Google Calendar OAuth2/PKCE connect flow: opens the consent
 * screen in the user's default browser and waits for it to complete.
 * Requires a Google Calendar Client ID to already be saved via
 * `setCredential`.
 */
export function connectGoogleCalendar(): Promise<void> {
  return invoke<void>("connect_google_calendar_cmd");
}

export function listProfiles(): Promise<Profile[]> {
  return invoke<Profile[]>("list_profiles");
}

/** `null` before any sign-in - the frontend shows the sign-in gate then. */
export function getCurrentProfile(): Promise<Profile | null> {
  return invoke<Profile | null>("get_current_profile");
}

/**
 * Runs the GitHub Device Flow end to end: resolves once the user has
 * entered the code and Otto has fetched their identity. Listen for the
 * `"device-code"` Tauri event (fired as soon as the code is ready) to show
 * it to the user while this promise is still pending.
 */
export function startGithubDeviceFlow(): Promise<Profile> {
  return invoke<Profile>("start_github_device_flow");
}

export function signInWithPat(token: string): Promise<Profile> {
  return invoke<Profile>("sign_in_with_pat", { token });
}

export function switchProfile(profileId: string): Promise<void> {
  return invoke<void>("switch_profile", { profileId });
}

/** Returns to the sign-in gate without deleting the profile. */
export function signOut(): Promise<void> {
  return invoke<void>("sign_out");
}

export function getPreferences(): Promise<Preferences> {
  return invoke<Preferences>("get_preferences");
}

export function setPreferences(preferences: Preferences): Promise<void> {
  return invoke<void>("set_preferences", { preferences });
}
