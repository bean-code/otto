import { describe, expect, it } from "vitest";

import { cn, errorMessage } from "@/lib/utils";

describe("cn", () => {
  it("merges class names and resolves Tailwind conflicts (last wins)", () => {
    expect(cn("px-2", "px-4")).toBe("px-4");
  });

  it("drops falsy values", () => {
    const isActive = false;
    expect(cn("a", isActive && "b", undefined, null, "c")).toBe("a c");
  });
});

describe("errorMessage", () => {
  it("passes a plain string rejection through as-is", () => {
    expect(errorMessage("no 'repo' given")).toBe("no 'repo' given");
  });

  it("stringifies a non-string rejection (Tauri's object-shaped errors)", () => {
    expect(errorMessage({ message: "boom" })).toBe('{"message":"boom"}');
  });
});
