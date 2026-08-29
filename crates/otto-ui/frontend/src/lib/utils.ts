import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Tauri command rejections can be either a plain string or a JSON-ish
// object, depending on how the Rust side's `Result::Err` serializes -
// normalize both into readable text for toasts.
export function errorMessage(err: unknown): string {
  return typeof err === "string" ? err : JSON.stringify(err)
}
