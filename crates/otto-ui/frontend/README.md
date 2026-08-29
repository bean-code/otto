# otto-ui frontend

Vite + React + TypeScript, styled with [Tailwind CSS v4](https://tailwindcss.com) and
[shadcn/ui](https://ui.shadcn.com) (Radix primitives). Unlike the very first version of this
frontend, Node is now a **required build-time dependency** - the source is real `.tsx`/`.ts`, so
`frontend/dist/` can no longer be hand-maintained or committed; it's generated build output.

## Setup

```sh
pnpm install
```

## Developing

```sh
pnpm tauri dev
```

Runs the Vite dev server (with HMR) and the Tauri window together - this is the primary dev
workflow. `cargo run -p otto-ui` also still works, but only against a previously-built `dist/`
(no HMR); run `pnpm build` first if you use it.

## Building

```sh
pnpm tauri build
```

Builds the frontend and the platform installer together. `pnpm build` alone just produces
`dist/` (type-checks with `tsc -b`, then bundles with Vite) without invoking Tauri's bundler.

## Adding shadcn components

```sh
pnpm dlx shadcn@latest add <component>
```

Writes into `src/components/ui/`. The project's shadcn config (`components.json`) already points
at the `@/*` path alias and `src/index.css`.

## Design tokens

`src/index.css` holds Otto's brand tokens (Tailwind v4's CSS-first `@theme` block - colors,
shadows, custom animation utilities) alongside shadcn's own theme slots (`--primary`,
`--background`, etc.), which are mapped onto the same brand colors so shadcn's generated
primitives stay visually consistent with the rest of the app.

## Icons

Two icon sets, deliberately kept separate:

- `src/components/icons.tsx` - Otto's own hand-authored outline SVGs (24x24, `currentColor`
  stroke), used for the app's own branding/UI (toolbar, cards, tabs).
- `lucide-react` - pulled in by shadcn's generated primitives for their own internals (dialog
  close button, etc.) - not used directly by Otto's own components.

## Talking to the Rust backend

`src/lib/tauri.ts` is the **only** file that calls the raw Tauri `invoke()` - every
`#[tauri::command]` in `crates/otto-ui/src/lib.rs` has a typed wrapper there. `src/lib/types.ts`
mirrors the Rust `Serialize` struct shapes. If you add or change a Tauri command, update both
files to match - nothing else in the frontend should call `invoke` directly.
