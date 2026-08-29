# otto-ui frontend

Vite + React + TypeScript, styled with [Tailwind CSS v4](https://tailwindcss.com) and
[shadcn/ui](https://ui.shadcn.com) (Radix primitives). Unlike the very first version of this
frontend, Node is now a **required build-time dependency** - the source is real `.tsx`/`.ts`, so
`frontend/dist/` can no longer be hand-maintained or committed; it's generated build output.

This directory is a member of the pnpm workspace rooted one level up at `crates/otto-ui/` (see
that directory's `package.json`) - `@tauri-apps/cli` lives there, not here, because the Tauri CLI
needs `tauri.conf.json`'s own directory as its working directory. Run `pnpm dev`/`pnpm build`
**from `crates/otto-ui/`**, not from here, to actually launch or build the app; the commands below
are for iterating on the frontend in isolation.

## Setup

```sh
pnpm --dir crates/otto-ui install
```

(installs both this package and the workspace root's `@tauri-apps/cli`)

## Developing the frontend in isolation

```sh
pnpm dev
```

Starts just the Vite dev server (`http://localhost:1420`) without Tauri - useful for iterating on
markup/styling in a regular browser tab. For the full app with the Tauri window, run
`pnpm --dir crates/otto-ui dev` instead (see the root README).

## Building

```sh
pnpm build
```

Type-checks with `tsc -b`, then bundles with Vite into `dist/` - this alone doesn't invoke
Tauri's bundler. For a full release build (frontend + platform installer), run
`pnpm --dir crates/otto-ui build`.

## Testing

```sh
pnpm test           # run once (what CI runs)
pnpm test:watch     # watch mode
pnpm test:coverage  # with a coverage report
```

[Vitest](https://vitest.dev) + [React Testing Library](https://testing-library.com/react), configured via the
`test` block in `vite.config.ts` (reuses the same Vite config/plugins/alias rather than a separate config file -
`globals: true` is required there for Testing Library's auto-cleanup-after-each-test to register itself).
`src/lib/tauri.ts` is mocked with `vi.mock` in component tests rather than hit for real - no live Tauri runtime
exists under `jsdom`. Test files sit next to what they test (`Sidebar.tsx` / `Sidebar.test.tsx`), not in a
separate `__tests__/` tree.

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
