# macOS App Core (M2, plan 3a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A working Secopy app for Apple Silicon Macs. You pick or drop a source and a
destination, see what will happen, copy and verify, follow the progress and read an honest
summary. Each release carries an unsigned `.dmg`.

**Architecture:** `crates/secopy-app` (Tauri 2) keeps the session (scan → selection →
pre-flight → plan) and runs one job at a time on a thread. Small commands return summaries,
progress arrives on a channel twice a second, and the finished list is fetched in pages.
`ui/` (Svelte 5 + TypeScript) owns the form and the screens, and talks to the app through
one `Api` object that tests replace. The TypeScript types are generated from the Rust DTOs.

**Tech Stack:** Rust (edition 2024) with `tauri` 2.12, `tauri-plugin-dialog` 2.7,
`tauri-plugin-opener` 2.6, `tauri-specta`/`specta` 2.0.0-rc.25, `specta-typescript` 0.0.12;
Node 24 with Svelte 5, Vite 8, TypeScript 6, Vitest 5, happy-dom, `@testing-library/svelte`,
`svelte-check`, `@tauri-apps/cli` 2.12.

**Spec:** [2026-09-27-macos-app-core-design.md](../specs/2026-09-27-macos-app-core-design.md),
requirements in [RFD 0001](../../rfd/0001-secopy.md) §5. Issue: #15. Roadmap:
[2026-09-26-v1-roadmap.md](2026-09-26-v1-roadmap.md).

**How this plan was checked:** every task was built first in a throwaway prototype, and the
code below is that prototype's code. Every commit passed the following:
- `cargo fmt --check`
- clippy (the app on macOS; the engine also for Linux and Windows)
- `cargo test --workspace`
- `svelte-check` with 0 errors and 0 warnings
- the Vitest suite

The app was built into a `.dmg` and launched. Its look on screen hasn't been checked (no
screen recording in that session); the manual checklist in Task 9 covers that.

## Global Constraints

- Work on the branch `feat/15-macos-app` (issue #15). Conventional Commits exactly as in
  `AGENTS.md`, with the messages given in each task; each ends with `Refs: #15`. Commit only
  when the user has asked for this plan to be executed.
- Before every commit, all of these must pass:
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - from `ui/` (once there is one): `npm run check` with 0 errors and 0 warnings, and
    `npm test`
- Never bump versions or edit `CHANGELOG.md`. The app has no version of its own:
  `tauri.conf.json` has none, so Tauri reads the workspace `Cargo.toml`, and
  `ui/package.json` is private with no version.
- App identifier `com.belisoft.secopy`; product name `Secopy`; Apple Silicon only
  (`aarch64-apple-darwin`).
- The engine (`secopy-core`) doesn't change in this plan.
- The logic lives in `session` and `jobs`, which don't use Tauri types, so it is tested with
  `cargo test`. Commands stay thin.
- DTO numbers: counts are `u32`; bytes and milliseconds are `u64` with
  `#[specta(type = specta_typescript::Number)]` on each field. Never `f64`: it becomes
  `number | null` in TypeScript.
- `ui/src/lib/bindings.ts` is generated (`SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app`)
  and never edited by hand.
- Progress reaches the UI twice a second (`PROGRESS_INTERVAL` = 500 ms); bars animate for
  0.5 s.
- The destination is never filled in automatically. Nothing is ever written to the source.
- npm plugin packages and their Rust crates must share the same minor version (dialog `~2.7`).
- UI tests use `fakeApi()`, never Tauri. Components get the API with `useApi()`.
- Figures use tabular numerals (set once in `app.css`); hashes use `.mono`.

## Review Focus

The five input classes most likely to hurt someone using the app. Each has a pinning test
in the task named:

1. **Picking a new source while a large scan is still running.** The older scan's result
   must never replace the newer one. Pinned in Task 2
   (`a_scan_replaced_by_a_newer_one_is_dropped`) and Task 6 (the view ignores
   `stale: true`).
2. **Offloading into a folder that already has files** (the second card of the day, or the
   wrong day's folder). The warning shows how many items are there without blocking, and
   identical files are counted as skipped. Pinned in Task 2
   (`a_non_empty_copy_root_is_counted_without_hidden_files`) and Task 6
   (`a non-empty copy root is a warning, not a block`).
3. **A job with 100,000+ files.** The finished list stays small in the DOM and is fetched
   in pages. Pinned in Task 7 (`the finished list only renders the visible rows and
   fetches their page`) and Task 3 (`finished_rows_come_in_pages`).
4. **Closing or quitting during a copy.** It asks first; if closed anyway, the job is
   cancelled and waited for, so no partial file is left. Pinned in Task 8 (`closing during
   a copy asks, and stays open unless confirmed`). The quit path is covered by Task 9's
   manual checklist, item 9.
5. **Something blocks the copy, or it can't start** (the destination inside the source, not
   enough space, the source changed since pre-flight). Start stays disabled with the
   reason, and a failed start returns to setup with the message. Pinned in Task 2
   (`a_destination_inside_the_source_blocks_start`), Task 6 (`a blocker is shown and Start
   stays disabled`, `not enough space blocks Start`) and Task 8 (`a job that can't start
   goes back to setup with the reason`).

## File map

| File | Responsibility | Task |
|---|---|---|
| `crates/secopy-app/{Cargo.toml,build.rs,tauri.conf.json,capabilities/,icons/}` | The Tauri app's setup | 1 |
| `crates/secopy-app/src/dto.rs` | What the UI sees | 2 |
| `crates/secopy-app/src/session.rs` | The main window's state | 2, 3 |
| `crates/secopy-app/src/jobs.rs` | Running a job, progress, summary, report, retry | 3 |
| `crates/secopy-app/src/commands.rs`, `lib.rs` | Commands, bindings, app start and quit | 1, 4 |
| `ui/{package.json,vite.config.ts,tsconfig.json,index.html}`, `ui/src/{main.ts,app.css}` | The UI's setup and tokens | 1 |
| `ui/src/lib/{format,rate,api,headline}.ts`, `ui/src/lib/bindings.ts` | Shared UI logic, generated types | 4, 5, 8 |
| `ui/src/components/{Setup,ExtensionChips,PreflightPanel}.svelte` | Main window | 6 |
| `ui/src/components/{JobProgress,ProgressBar,FinishedList}.svelte` | Progress view | 7 |
| `ui/src/components/Summary.svelte`, `ui/src/App.svelte` | Summary, screens | 8 |
| `.github/workflows/{ci,release-build}.yml` | CI and release builds | 1, 9 |
| `README.md`, `AGENTS.md`, `docs/testing/macos-app-checklist.md` | Docs | 9 |

---

### Task 1: Scaffold the Tauri app and the Svelte UI

Creates the two new parts of the app and proves they build.

- **`crates/secopy-app`**, the Tauri 2 shell:
  - `tauri.conf.json`: identifier `com.belisoft.secopy`, no `version` (it comes from
    `Cargo.toml`), dev URL, `frontendDist` pointing at `ui/dist`, a strict CSP.
  - capabilities: open/save/ask dialogs, reveal in Finder, open a path.
  - the icon set, generated from a placeholder drawn by a small Python script.
- **`ui/`**, Svelte 5 + TypeScript on Vite 8:
  - the dark design tokens of RFD §5.6 in `app.css`
  - Vitest with happy-dom (jsdom 30 needs Node ≥ 24.15)
  - `svelte-check` on TypeScript 6 (it doesn't support 7 yet)

The Tauri CLI runs from `ui/` and finds the app through `TAURI_APP_PATH` in the `tauri`
npm script. Debug builds (`cargo build`, `cargo test`, clippy) don't need `ui/dist`; only
`tauri build` does. The npm plugin versions must match the crates' minor versions (the
dialog plugin is `~2.7` on both sides), or `tauri build` refuses to run.

**Files:**
- Create: `crates/secopy-app/Cargo.toml`, `crates/secopy-app/build.rs`, `crates/secopy-app/capabilities/default.json`, `crates/secopy-app/icons/make-source-icon.py`, `crates/secopy-app/src/lib.rs`, `crates/secopy-app/src/main.rs`, `crates/secopy-app/tauri.conf.json`, `ui/index.html`, `ui/package.json`, `ui/src/App.svelte`, `ui/src/App.test.ts`, `ui/src/app.css`, `ui/src/main.ts`, `ui/svelte.config.js`, `ui/tsconfig.json`, `ui/vite.config.ts`
- Modify: `.gitignore`, `Cargo.toml`, `.github/workflows/ci.yml`
- Generated: `Cargo.lock` (Cargo updates it; commit it)
- Generated: `ui/package-lock.json` (from `npm install` in Step 3; commit it)
- Generated: the icon files (from Step 3's commands; commit them)

**Interfaces:**
- Consumes: nothing.
- Produces: `secopy_app::run()`; the `secopy` binary; `ui/` with `npm run dev|build|test|check|tauri`;
  the CSS tokens `--bg --surface --surface-raised --border --text --text-muted --text-faint
  --accent --success --warning --danger --radius --gap --font-mono` and the classes `.mono`,
  `button.primary`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/App.test.ts`:

```ts
import { expect, test } from "vitest";
import { render, screen } from "@testing-library/svelte";
import App from "./App.svelte";

test("shows the app name", () => {
  render(App);
  expect(screen.getByRole("heading", { name: "Secopy" })).toBeTruthy();
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && npm test`
Expected: fails: there is no `package.json` yet (npm reports a missing script or file).

- [ ] **Step 3: Implement**

Create the files below. Then, from the repository root:

```bash
cd ui && npm install && cd ..
python3 crates/secopy-app/icons/make-source-icon.py crates/secopy-app/icons/icon-source.png
(cd ui && npx tauri icon ../crates/secopy-app/icons/icon-source.png -o ../crates/secopy-app/icons)
# Only the macOS icons are used:
(cd crates/secopy-app/icons && rm -rf android ios Square*.png StoreLogo.png icon.ico 64x64.png)
```

Change `.gitignore`:

```diff
diff --git a/.gitignore b/.gitignore
index 5b47fcc..7a6bba5 100644
--- a/.gitignore
+++ b/.gitignore
@@ -12,3 +12,6 @@ dist/
 # Editors
 .idea/
 *.swp
+
+# Tauri
+crates/secopy-app/gen/
```

Change `Cargo.toml`:

```diff
diff --git a/Cargo.toml b/Cargo.toml
index 4b8f590..733d40b 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -17,6 +17,13 @@ libc = "0.2"
 serde = { version = "1.0.229", features = ["derive"] }
 serde_json = "1.0.151"
 tempfile = "3.27.0"
+specta = { version = "=2.0.0-rc.25", features = ["derive"] }
+specta-typescript = "=0.0.12"
+tauri = "2.12.0"
+tauri-build = "2.6.3"
+tauri-plugin-dialog = "2.7.3"
+tauri-plugin-opener = "2.6.0"
+tauri-specta = { version = "=2.0.0-rc.25", features = ["derive", "typescript"] }
 thiserror = "2.0.21"
 walkdir = "2.5.0"
 windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Storage_FileSystem", "Win32_System_Power"] }
```

Create `crates/secopy-app/Cargo.toml`:

```toml
[package]
name = "secopy-app"
description = "Secopy desktop app: the Tauri shell around secopy-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[lib]
name = "secopy_app"

[[bin]]
name = "secopy"
path = "src/main.rs"

[build-dependencies]
tauri-build.workspace = true

[dependencies]
chrono.workspace = true
secopy-core.workspace = true
serde.workspace = true
specta.workspace = true
specta-typescript.workspace = true
tauri.workspace = true
tauri-plugin-dialog.workspace = true
tauri-plugin-opener.workspace = true
tauri-specta.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

Create `crates/secopy-app/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `crates/secopy-app/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "What the main window may do: pick files and folders, reveal and open results.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:allow-open",
    "dialog:allow-save",
    "dialog:allow-ask",
    "opener:allow-reveal-item-in-dir",
    { "identifier": "opener:allow-open-path", "allow": [{ "path": "**" }] }
  ]
}
```

Create `crates/secopy-app/icons/make-source-icon.py`:

```python
#!/usr/bin/env python3
"""Draws the placeholder app icon (1024x1024 PNG) with no dependencies: two cards, the
front one with a check mark. Regenerate the icon set with `npm run tauri icon` from ui/."""
import struct
import sys
import zlib

N = 1024
BG, BACK, FRONT, TICK = (17, 18, 20), (46, 48, 53), (79, 140, 255), (255, 255, 255)


def rounded(x, y, x0, y0, x1, y1, r):
    """Inside a rounded rectangle?"""
    if not (x0 <= x < x1 and y0 <= y < y1):
        return False
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def near_segment(x, y, ax, ay, bx, by, w):
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
    px, py = ax + t * dx, ay + t * dy
    return (x - px) ** 2 + (y - py) ** 2 <= w * w


rows = []
for y in range(N):
    row = bytearray([0])
    for x in range(N):
        a, c = 0, BG
        if rounded(x, y, 100, 100, 924, 924, 185):
            a = 255
            if rounded(x, y, 250, 210, 700, 660, 70):
                c = BACK
            if rounded(x, y, 330, 360, 780, 810, 70):
                c = FRONT
                if near_segment(x, y, 440, 590, 530, 680, 34) or near_segment(x, y, 530, 680, 680, 490, 34):
                    c = TICK
        row += bytes(c) + bytes([a])
    rows.append(bytes(row))


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")
open(sys.argv[1] if len(sys.argv) > 1 else "icon-source.png", "wb").write(png)
```

Create `crates/secopy-app/src/lib.rs`:

```rust
//! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.

/// Starts the app. Blocks until the last window closes.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to start Secopy");
}
```

Create `crates/secopy-app/src/main.rs`:

```rust
fn main() {
    secopy_app::run();
}
```

Create `crates/secopy-app/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Secopy",
  "identifier": "com.belisoft.secopy",
  "build": {
    "frontendDist": "../../ui/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": { "script": "npm run dev", "cwd": "../../ui" },
    "beforeBuildCommand": { "script": "npm run build", "cwd": "../../ui" }
  },
  "app": {
    "windows": [
      {
        "title": "Secopy",
        "width": 900,
        "height": 780,
        "minWidth": 720,
        "minHeight": 560
      }
    ],
    "security": {
      "csp": "default-src 'self'; connect-src ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["dmg"],
    "category": "Utility",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns"]
  }
}
```

Create `ui/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Secopy</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Create `ui/package.json`:

```json
{
  "name": "secopy-ui",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "test": "vitest run",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "tauri": "TAURI_APP_PATH=../crates/secopy-app TAURI_FRONTEND_PATH=. tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.12.0",
    "@tauri-apps/plugin-dialog": "~2.7.0",
    "@tauri-apps/plugin-opener": "^2.6.0"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^7.3.1",
    "@tauri-apps/cli": "^2.12.0",
    "@testing-library/svelte": "^5.4.2",
    "@tsconfig/svelte": "^5.0.8",
    "happy-dom": "^20.14.5",
    "svelte": "^5.57.1",
    "svelte-check": "^4.7.6",
    "typescript": "~6.0.3",
    "vite": "^8.3.1",
    "vitest": "^5.0.2"
  }
}
```

Create `ui/src/App.svelte`:

```svelte
<script lang="ts">
</script>

<main>
  <h1>Secopy</h1>
</main>

<style>
  main {
    padding: 24px;
  }

  h1 {
    margin: 0;
    font-size: 18px;
  }
</style>
```

Create `ui/src/app.css`:

```css
/* Design tokens (RFD §5.6): dark only in v1; components use these, never raw colours. */
:root {
  --bg: #111214;
  --surface: #1a1b1e;
  --surface-raised: #232428;
  --border: #2e3035;
  --text: #e6e7e9;
  --text-muted: #9a9da3;
  --text-faint: #6b6e75;
  --accent: #4f8cff;
  --success: #3ecf8e;
  --warning: #f5a524;
  --danger: #f25f5c;

  --radius: 8px;
  --gap: 12px;
  --font-mono: ui-monospace, "SF Mono", Menlo, monospace;

  color-scheme: dark;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-size: 14px;
  color: var(--text);
  background: var(--bg);
  /* Figures keep their width, so columns stay still while numbers change. */
  font-variant-numeric: tabular-nums;
}

body {
  margin: 0;
}

button {
  font: inherit;
  color: var(--text);
  background: var(--surface-raised);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 6px 12px;
  cursor: pointer;
}

button:disabled {
  color: var(--text-faint);
  cursor: default;
}

button.primary {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.mono {
  font-family: var(--font-mono);
}

@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
  }
}
```

Create `ui/src/main.ts`:

```ts
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

const app = mount(App, { target: document.getElementById("app")! });

export default app;
```

Create `ui/svelte.config.js`:

```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default { preprocess: vitePreprocess() };
```

Create `ui/tsconfig.json`:

```json
{
  "extends": "@tsconfig/svelte/tsconfig.json",
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "verbatimModuleSyntax": true,
    "types": ["svelte", "vite/client"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte", "vite.config.ts"]
}
```

Create `ui/vite.config.ts`:

```ts
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";

// Tauri serves the dev build from a fixed port and loads `dist/` in release builds.
export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "safari16", outDir: "dist" },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts"],
  },
});
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also, from `ui/`: `npm test && npm run check && npm run build`. Expected: 1 test passes,
0 errors and 0 warnings, and `dist/` is built. Then `npm run tauri -- build --target
aarch64-apple-darwin --bundles dmg` produces `target/aarch64-apple-darwin/release/bundle/dmg/Secopy_0.0.0_aarch64.dmg`.
The first `.dmg` build may fail in the step that styles the Finder window, while macOS asks
whether the terminal may control Finder. Allow it and build again.

- [ ] **Step 5: Commit, then check the UI in CI**

```bash
git add -A
git commit -F - <<'EOF'
feat(app): add the tauri app and svelte ui scaffold

The app crate (Tauri 2, identifier com.belisoft.secopy, version from
Cargo.toml) and the ui/ folder (Svelte 5, TypeScript, Vite, Vitest)
with the dark design tokens of RFD §5.6. Debug builds don't need the
built UI; tauri build does.

Refs: #15
EOF
```


Change `.github/workflows/ci.yml`:

```diff
diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml
index bae2c89..e49a486 100644
--- a/.github/workflows/ci.yml
+++ b/.github/workflows/ci.yml
@@ -19,7 +19,18 @@ jobs:
     steps:
       - uses: actions/checkout@v7
       - uses: Swatinem/rust-cache@v2
+      - uses: actions/setup-node@v7
+        with:
+          node-version: 24
+          cache: npm
+          cache-dependency-path: ui/package-lock.json
       - run: brew install xxhash
+      - run: npm ci
+        working-directory: ui
+      - run: npm run check
+        working-directory: ui
+      - run: npm test
+        working-directory: ui
       - run: cargo fmt --all --check
       - run: cargo clippy --workspace --all-targets --locked -- -D warnings
       - run: cargo test --workspace --locked
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: check the ui in ci

The macOS job installs the UI's packages and runs svelte-check and the
Vitest suite before the Rust checks.

Refs: #15
EOF
```


---

### Task 2: Session: the main window's state (`session`, `dto`)

`Session` holds what the main window shows: the scanned source, the extension filter's
selection, the destination's pre-flight and the resolved plan. Each change recomputes only
what depends on it and returns one `SessionView`, so the UI never has to combine partial
answers.

- **Scans:** a scan runs without the session locked. `begin_scan` hands out a ticket, and
  `finish_scan` keeps the result only if no newer scan began (FR-3).
- **The copy root** (FR-4) is `DEST/<folder name>` for "copy the folder itself", else `DEST`.
- **The non-empty warning:** `existing_items` counts the visible items already in the copy
  root, so `.DS_Store` doesn't count.

`dto` defines what the UI sees:
- paths are strings (lossy for names that aren't UTF-8)
- counts are `u32`
- bytes and milliseconds are `u64`, exported to TypeScript as `number` with
  `#[specta(type = specta_typescript::Number)]` on each field. `f64` would come out as
  `number | null`.

**Files:**
- Create: `crates/secopy-app/src/dto.rs`, `crates/secopy-app/src/session.rs`
- Modify: `crates/secopy-app/src/lib.rs`

**Interfaces:**
- Consumes: `secopy-core`'s `scan`, `preflight`, `Plan::resolve`, `fsinfo::fs_info`.
- Produces: `session::{Session, Ready { source, plan, label, copy_root }, scan_source(&Source) -> Result<Scan, String>}`;
  `Session::{new, source_for(&[PathBuf], contents_only) -> Result<Source, String>, begin_scan() -> u64, finish_scan(ticket, Source, Scan) -> SessionView, install_retry(Source, Selection) -> SessionView, clear_source(), set_filter(Option<Vec<ExtensionKey>>), set_destination(Option<PathBuf>), set_policy(ConflictPolicy), view(), ready() -> Option<Ready>}`;
  `dto::{SessionView, SourceView, ExtensionView, ExtensionKey, DestinationView, FileProblemView, ConflictPolicy, PlanView, ProgressView, JobPhase, ActiveFileView, SmallFilesView, FinishedRow, RowStatus, SummaryView, JobOutcome, show, count}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-app/src/session.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, files: &[(&str, &[u8])]) {
        for (rel, data) in files {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, data).unwrap();
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        card: PathBuf,
        dest: PathBuf,
    }

    /// CARD/ with two .mov (7 bytes) and one .xml (3 bytes); an empty dest/.
    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        write(
            &card,
            &[
                ("A001.mov", b"movie-a"),
                ("B002.mov", b"movie-b"),
                ("A001.xml", b"xml"),
            ],
        );
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        Fixture {
            _dir: dir,
            card,
            dest,
        }
    }

    fn pick(session: &mut Session, paths: &[PathBuf], contents_only: bool) -> SessionView {
        let ticket = session.begin_scan();
        let source = Session::source_for(paths, contents_only).unwrap();
        let scan = scan_source(&source).unwrap();
        session.finish_scan(ticket, source, scan)
    }

    #[test]
    fn a_folder_shows_counts_and_extensions_largest_first() {
        let f = fixture();
        let mut s = Session::new();
        let view = pick(&mut s, std::slice::from_ref(&f.card), false);
        let src = view.source.unwrap();
        assert!(src.is_folder && !src.contents_only);
        assert_eq!(src.root_dir.as_deref(), Some("CARD"));
        assert_eq!((src.files, src.bytes), (3, 17.0));
        let labels: Vec<_> = src.extensions.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, [".mov", ".xml"]);
        assert_eq!((view.selected_files, view.selected_bytes), (3, 17.0));
        assert_eq!(src.selected_extensions, None);
    }

    #[test]
    fn the_filter_narrows_the_selection() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_filter(Some(vec![Some("mov".into())]));
        assert_eq!((view.selected_files, view.selected_bytes), (2, 14.0));
        assert_eq!(
            view.source.unwrap().selected_extensions,
            Some(vec![Some("mov".into())])
        );
    }

    #[test]
    fn a_folder_and_files_together_are_refused() {
        let f = fixture();
        let err =
            Session::source_for(&[f.card.clone(), f.card.join("A001.mov")], false).unwrap_err();
        assert!(err.contains("not both"), "{err}");
    }

    #[test]
    fn copy_root_follows_the_folder_or_contents_choice() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_destination(Some(f.dest.clone()));
        assert_eq!(
            view.destination.unwrap().copy_root,
            show(&f.dest.join("CARD"))
        );
        let view = pick(&mut s, std::slice::from_ref(&f.card), true);
        assert_eq!(view.destination.unwrap().copy_root, show(&f.dest));
    }

    #[test]
    fn a_non_empty_copy_root_is_counted_without_hidden_files() {
        let f = fixture();
        write(
            &f.dest,
            &[
                ("CARD/old.mov", b"x"),
                ("CARD/.DS_Store", b"x"),
                ("CARD/sub/y.mov", b"y"),
            ],
        );
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
        assert_eq!(dest.existing_items, Some(2), "old.mov and sub/");
        assert!(dest.blocker.is_none());
    }

    #[test]
    fn a_missing_copy_root_has_no_existing_items() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
        assert_eq!(dest.existing_items, None);
    }

    #[test]
    fn conflicts_are_counted_and_the_policy_changes_the_plan() {
        let f = fixture();
        write(&f.dest, &[("CARD/A001.mov", b"other content")]);
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_destination(Some(f.dest.clone()));
        let dest = view.destination.unwrap();
        assert_eq!((dest.identical, dest.differs), (0, 1));
        assert_eq!(view.conflicts, ConflictPolicy::KeepBoth);
        assert_eq!(view.plan.unwrap().files_to_write, 3);
        let view = s.set_policy(ConflictPolicy::Skip);
        assert_eq!(view.plan.unwrap().files_to_write, 2);
        assert!(s.ready().is_some());
    }

    #[test]
    fn a_destination_inside_the_source_blocks_start() {
        let f = fixture();
        let inside = f.card.join("backup");
        fs::create_dir_all(&inside).unwrap();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_destination(Some(inside));
        let blocker = view.destination.unwrap().blocker.unwrap();
        assert!(
            blocker.starts_with("The destination is the source"),
            "{blocker}"
        );
        assert!(view.plan.is_none());
        assert!(s.ready().is_none());
    }

    #[test]
    fn a_destination_alone_shows_its_free_space_or_why_it_cant_be_used() {
        let f = fixture();
        let mut s = Session::new();
        let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
        assert!(dest.free_bytes > 0.0 && dest.blocker.is_none());
        let missing = s
            .set_destination(Some(f.dest.join("nope")))
            .destination
            .unwrap();
        assert_eq!(
            missing.blocker.as_deref(),
            Some("The destination is not an existing folder")
        );
    }

    #[test]
    fn a_scan_replaced_by_a_newer_one_is_dropped() {
        let f = fixture();
        let mut s = Session::new();
        let old = s.begin_scan();
        let newer = s.begin_scan();
        let source = Session::source_for(std::slice::from_ref(&f.card), false).unwrap();
        let view = s.finish_scan(old, source.clone(), scan_source(&source).unwrap());
        assert!(view.stale && view.source.is_none());
        let view = s.finish_scan(newer, source.clone(), scan_source(&source).unwrap());
        assert!(!view.stale && view.source.is_some());
    }

    #[test]
    fn nothing_to_write_is_not_ready() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        s.set_destination(Some(f.dest.clone()));
        s.set_filter(Some(vec![Some("wav".into())]));
        assert!(s.ready().is_none());
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-app --lib session`
Expected: compile errors: `session` and `dto` don't exist yet.

- [ ] **Step 3: Implement**

Create `crates/secopy-app/src/dto.rs`:

```rust
//! The data the UI sees (RFD §5.2–§5.4). Plain, serializable summaries: paths are strings
//! (names that aren't UTF-8 are shown lossily), byte counts are `f64` (exact up to 9 PB,
//! and JavaScript has no 64-bit integers), counts are `u32`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Everything the main window shows. Every session command returns the whole view, so the
/// UI never has to combine partial answers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub source: Option<SourceView>,
    /// Files and bytes the extension filter keeps.
    pub selected_files: u32,
    pub selected_bytes: f64,
    pub destination: Option<DestinationView>,
    pub conflicts: ConflictPolicy,
    pub plan: Option<PlanView>,
    /// A newer scan replaced this one while it ran (FR-3); the UI keeps its current view.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    /// What the user picked: the folder, or "3 files".
    pub label: String,
    pub is_folder: bool,
    /// Copy only what's inside the folder (FR-4b) instead of the folder itself.
    pub contents_only: bool,
    /// The folder created for "copy the folder itself", e.g. "CLIP".
    pub root_dir: Option<String>,
    pub files: u32,
    pub bytes: f64,
    /// Sorted by bytes, largest first (FR-8).
    pub extensions: Vec<ExtensionView>,
    /// `None` = every extension; otherwise the selected keys (FR-8).
    pub selected_extensions: Option<Vec<ExtensionKey>>,
    pub skipped_hidden: u32,
    pub skipped_symlinks: u32,
    /// Things that couldn't be read while scanning, first 20.
    pub problems: Vec<String>,
    pub problem_count: u32,
}

/// A lowercase extension without the dot; `None` = files without one (FR-9).
pub type ExtensionKey = Option<String>;

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionView {
    pub key: ExtensionKey,
    /// ".mov", or "(no extension)".
    pub label: String,
    pub files: u32,
    pub bytes: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DestinationView {
    pub path: String,
    /// Where the files will land ("Files will go to", FR-4).
    pub copy_root: String,
    /// Stops the job (FR-16); Start stays disabled.
    pub blocker: Option<String>,
    pub free_bytes: f64,
    pub fs_kind: String,
    /// Items already in the copy root, hidden ones not counted; `None` if it doesn't exist.
    /// More than zero shows the non-empty warning.
    pub existing_items: Option<u32>,
    /// Files that will fail, first 100 (FR-16).
    pub problems: Vec<FileProblemView>,
    pub problem_count: u32,
    /// Same size and date at the destination: skipped, not checked (FR-17).
    pub identical: u32,
    /// Different files with the same name (FR-17).
    pub differs: u32,
    /// Partial files left by an interrupted copy, to be replaced (FR-18).
    pub stale_partials: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileProblemView {
    pub path: String,
    pub reason: String,
}

/// What to do with files that exist but differ (FR-17).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    #[default]
    KeepBoth,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub files_to_write: u32,
    pub bytes_to_write: f64,
    /// Not enough free space (FR-16).
    pub blocker: Option<String>,
}

/// Sent twice a second while a job runs (RFD §5.3, NFR-5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub phase: JobPhase,
    pub elapsed_ms: f64,
    pub paused: bool,
    pub verify: bool,
    pub total_files: u32,
    /// Bytes the job writes; skipped files are not included.
    pub total_bytes: f64,
    pub copied_bytes: f64,
    pub verified_bytes: f64,
    pub files_done: u32,
    pub files_skipped: u32,
    pub files_failed: u32,
    /// Files of 8 MiB or more in progress.
    pub active: Vec<ActiveFileView>,
    /// Smaller files in progress, summed into one row.
    pub small_files: Option<SmallFilesView>,
    /// Set once, when the job has stopped for good.
    pub fatal: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobPhase {
    #[default]
    Copying,
    Verifying,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFileView {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub verifying: bool,
    pub size: f64,
    pub bytes_done: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SmallFilesView {
    pub count: u32,
    pub size: f64,
    pub bytes_done: f64,
}

/// One row of the finished list (RFD §5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FinishedRow {
    pub id: u32,
    pub path: String,
    /// Differs from `path` when the copy was kept under a new name.
    pub final_path: String,
    pub size: f64,
    pub seconds: f64,
    pub hash: Option<String>,
    pub status: RowStatus,
    /// Why it failed or was skipped.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RowStatus {
    Copied,
    Verified,
    Skipped,
    Failed,
}

/// The summary after a job (RFD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub outcome: JobOutcome,
    /// "All 1,000 files copied and verified", "3 files failed", …
    pub headline: String,
    pub verify: bool,
    pub files: u32,
    pub copied: u32,
    pub verified: u32,
    pub skipped_identical: u32,
    pub skipped_different: u32,
    pub failed: u32,
    pub not_started: u32,
    pub bytes_written: f64,
    pub seconds: f64,
    /// Failed files with their reasons, first 1,000.
    pub failures: Vec<FinishedRow>,
    pub copy_root: String,
    pub checksum_file: Option<String>,
    pub checksum_error: Option<String>,
    /// The text report saved in the app's data folder (FR-35).
    pub report_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobOutcome {
    Complete,
    Failures,
    Cancelled,
    Stopped,
}

/// Paths shown to people: lossy for names that aren't UTF-8.
pub fn show(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Counts shown in the UI; clamped rather than wrapped past `u32::MAX`.
pub fn count(n: impl TryInto<u32>) -> u32 {
    n.try_into().unwrap_or(u32::MAX)
}

/// Bytes shown in the UI.
pub fn bytes(n: u64) -> f64 {
    n as f64
}
```

Change `crates/secopy-app/src/lib.rs`:

```diff
diff --git a/crates/secopy-app/src/lib.rs b/crates/secopy-app/src/lib.rs
index 2ea2c0b..0b1b0ff 100644
--- a/crates/secopy-app/src/lib.rs
+++ b/crates/secopy-app/src/lib.rs
@@ -1,5 +1,8 @@
 //! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.
 
+pub mod dto;
+pub mod session;
+
 /// Starts the app. Blocks until the last window closes.
 pub fn run() {
     tauri::Builder::default()
```

Add the implementation at the top of `crates/secopy-app/src/session.rs`, above the tests:

```rust
//! The main window's state (RFD §5.2): source → selection → destination check → plan.
//! Every change recomputes what depends on it and returns one [`SessionView`].

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::filter::ExtensionFilter;
use secopy_core::fsinfo::{self, FsKind};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::{Blocker, ConflictKind, Preflight, preflight};
use secopy_core::scan::{self, Scan, ScanOptions, Selection};
use secopy_core::source::{DirMode, Source};

use crate::dto::{
    ConflictPolicy, DestinationView, ExtensionKey, ExtensionView, FileProblemView, PlanView,
    SessionView, SourceView, bytes, count, show,
};

/// Per-file problems sent to the UI; the rest are only counted.
const PROBLEMS_SHOWN: usize = 100;
/// Scan problems sent to the UI.
const SCAN_PROBLEMS_SHOWN: usize = 20;

#[derive(Default)]
pub struct Session {
    /// Increases with every scan; only the newest scan's result is kept (FR-3).
    generation: u64,
    source: Option<Picked>,
    filter: ExtensionFilter,
    selection: Option<Selection>,
    dest: Option<PathBuf>,
    policy: ConflictPolicy,
    checked: Option<Result<Preflight, Blocker>>,
    plan: Option<Plan>,
}

struct Picked {
    label: String,
    source: Source,
    scan: Scan,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// One folder → a folder source; otherwise only files (RFD Q6).
    pub fn source_for(paths: &[PathBuf], contents_only: bool) -> Result<Source, String> {
        match paths {
            [] => Err("nothing was picked".into()),
            [only] if only.is_dir() => Ok(Source::Directory {
                path: only.clone(),
                mode: if contents_only {
                    DirMode::ContentsOnly
                } else {
                    DirMode::FolderItself
                },
            }),
            _ if paths.iter().any(|p| p.is_dir()) => {
                Err("Pick one folder, or only files — not both.".into())
            }
            _ => Ok(Source::Files(paths.to_vec())),
        }
    }

    /// Starts a scan and returns its ticket. Scanning runs without the session locked;
    /// [`finish_scan`](Self::finish_scan) then keeps the result only if no newer scan began.
    pub fn begin_scan(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn finish_scan(&mut self, ticket: u64, source: Source, scan: Scan) -> SessionView {
        if ticket != self.generation {
            return SessionView {
                stale: true,
                ..self.view()
            };
        }
        self.source = Some(Picked {
            label: label(&source),
            source,
            scan,
        });
        self.filter = ExtensionFilter::All;
        self.recompute();
        self.view()
    }

    /// For "Retry failed": the failed files of the last job, checked again (RFD §5.4).
    pub fn install_retry(&mut self, source: Source, selection: Selection) -> SessionView {
        self.generation += 1;
        let scan = Scan {
            files: selection.files.clone(),
            ..Scan::default()
        };
        self.source = Some(Picked {
            label: format!("Retry: {} failed files", selection.files.len()),
            source,
            scan,
        });
        self.filter = ExtensionFilter::All;
        self.selection = Some(selection);
        self.recheck();
        self.view()
    }

    pub fn clear_source(&mut self) -> SessionView {
        self.generation += 1;
        self.source = None;
        self.filter = ExtensionFilter::All;
        self.recompute();
        self.view()
    }

    /// `None` selects every extension, which also recreates empty folders (FR-10).
    pub fn set_filter(&mut self, selected: Option<Vec<ExtensionKey>>) -> SessionView {
        self.filter = match selected {
            None => ExtensionFilter::All,
            Some(keys) => ExtensionFilter::Only(keys.into_iter().collect::<BTreeSet<_>>()),
        };
        self.recompute();
        self.view()
    }

    pub fn set_destination(&mut self, dest: Option<PathBuf>) -> SessionView {
        self.dest = dest;
        self.recheck();
        self.view()
    }

    pub fn set_policy(&mut self, policy: ConflictPolicy) -> SessionView {
        self.policy = policy;
        self.replan();
        self.view()
    }

    /// The source and plan to start a job with; `None` while anything blocks Start.
    pub fn ready(&self) -> Option<(&Source, &Plan, String)> {
        let picked = self.source.as_ref()?;
        let plan = self.plan.as_ref()?;
        (plan.blockers().is_empty() && !plan.files.is_empty())
            .then(|| (&picked.source, plan, picked.label.clone()))
    }

    fn recompute(&mut self) {
        self.selection = self.source.as_ref().map(|p| p.scan.select(&self.filter));
        self.recheck();
    }

    fn recheck(&mut self) {
        self.checked = match (&self.source, &self.selection, &self.dest) {
            (Some(picked), Some(sel), Some(dest)) => Some(preflight(&picked.source, sel, dest)),
            _ => None,
        };
        self.replan();
    }

    fn replan(&mut self) {
        self.plan = match (&self.selection, &self.checked) {
            (Some(sel), Some(Ok(pf))) => Some(Plan::resolve(sel, pf, policy(self.policy))),
            _ => None,
        };
    }

    pub fn view(&self) -> SessionView {
        let selection = self.selection.as_ref();
        SessionView {
            source: self.source.as_ref().map(|p| self.source_view(p)),
            selected_files: selection.map_or(0, |s| count(s.files.len())),
            selected_bytes: selection.map_or(0.0, |s| bytes(s.total_bytes)),
            destination: self.dest.as_ref().map(|d| self.destination_view(d)),
            conflicts: self.policy,
            plan: self.plan.as_ref().map(plan_view),
            stale: false,
        }
    }

    fn source_view(&self, picked: &Picked) -> SourceView {
        let scan = &picked.scan;
        let mut extensions: Vec<ExtensionView> = scan
            .ext_stats
            .iter()
            .map(|(key, stat)| ExtensionView {
                key: key.clone(),
                label: key
                    .as_ref()
                    .map_or("(no extension)".to_string(), |k| format!(".{k}")),
                files: count(stat.files),
                bytes: bytes(stat.bytes),
            })
            .collect();
        extensions.sort_by(|a, b| b.bytes.total_cmp(&a.bytes));
        let (is_folder, contents_only) = match &picked.source {
            Source::Directory { mode, .. } => (true, *mode == DirMode::ContentsOnly),
            Source::Files(_) => (false, false),
        };
        SourceView {
            label: picked.label.clone(),
            is_folder,
            contents_only,
            root_dir: scan.root_dir.as_deref().map(show),
            files: count(scan.files.len()),
            bytes: bytes(scan.files.iter().map(|f| f.size).sum()),
            extensions,
            selected_extensions: match &self.filter {
                ExtensionFilter::All => None,
                ExtensionFilter::Only(keys) => Some(keys.iter().cloned().collect()),
            },
            skipped_hidden: count(scan.skipped_hidden),
            skipped_symlinks: count(scan.skipped_symlinks.len()),
            problems: scan
                .problems
                .iter()
                .take(SCAN_PROBLEMS_SHOWN)
                .map(|p| format!("{}: {}", show(&p.path), p.message))
                .collect(),
            problem_count: count(scan.problems.len()),
        }
    }

    fn destination_view(&self, dest: &Path) -> DestinationView {
        let copy_root = self.copy_root(dest);
        let mut view = DestinationView {
            path: show(dest),
            copy_root: show(&copy_root),
            blocker: None,
            free_bytes: 0.0,
            fs_kind: String::new(),
            existing_items: existing_items(&copy_root),
            problems: Vec::new(),
            problem_count: 0,
            identical: 0,
            differs: 0,
            stale_partials: 0,
        };
        match &self.checked {
            Some(Ok(pf)) => {
                let sel = self
                    .selection
                    .as_ref()
                    .expect("checked implies a selection");
                view.free_bytes = bytes(pf.fs.free_bytes);
                view.fs_kind = fs_label(&pf.fs.kind);
                view.problems = pf
                    .file_problems
                    .iter()
                    .take(PROBLEMS_SHOWN)
                    .map(|p| FileProblemView {
                        path: show(&sel.files[p.id].rel),
                        reason: p.kind.to_error().to_string(),
                    })
                    .collect();
                view.problem_count = count(pf.file_problems.len());
                view.identical = count(
                    pf.conflicts
                        .iter()
                        .filter(|c| c.kind == ConflictKind::Identical)
                        .count(),
                );
                view.differs = count(pf.conflicts.len()) - view.identical;
                view.stale_partials = count(pf.stale_partials.len());
            }
            Some(Err(blocker)) => view.blocker = Some(sentence(blocker.to_string())),
            // No source yet: show what the destination is, or why it can't be used.
            None => match fsinfo::fs_info(dest) {
                Ok(info) => {
                    view.free_bytes = bytes(info.free_bytes);
                    view.fs_kind = fs_label(&info.kind);
                }
                Err(_) if !dest.is_dir() => {
                    view.blocker = Some(sentence(Blocker::DestMissing.to_string()))
                }
                Err(e) => {
                    view.blocker = Some(sentence(Blocker::DestNotWritable(e.into()).to_string()))
                }
            },
        }
        view
    }

    /// Where the files land: `DEST/<folder name>` for "copy the folder itself" (FR-4a),
    /// else `DEST`.
    fn copy_root(&self, dest: &Path) -> PathBuf {
        match self.source.as_ref().and_then(|p| p.scan.root_dir.as_ref()) {
            Some(root) => dest.join(root),
            None => dest.to_path_buf(),
        }
    }
}

fn policy(p: ConflictPolicy) -> DiffersPolicy {
    match p {
        ConflictPolicy::KeepBoth => DiffersPolicy::KeepBoth,
        ConflictPolicy::Overwrite => DiffersPolicy::Overwrite,
        ConflictPolicy::Skip => DiffersPolicy::Skip,
    }
}

fn plan_view(plan: &Plan) -> PlanView {
    PlanView {
        files_to_write: count(plan.files.iter().filter(|f| f.action.writes()).count()),
        bytes_to_write: bytes(plan.bytes_to_write()),
        blocker: plan.blockers().first().map(|b| sentence(b.to_string())),
    }
}

fn label(source: &Source) -> String {
    match source {
        Source::Directory { path, .. } => show(path),
        Source::Files(files) if files.len() == 1 => show(&files[0]),
        Source::Files(files) => format!("{} files", files.len()),
    }
}

/// Visible items in `dir` (names starting with `.` aren't counted); `None` if it isn't a
/// folder.
fn existing_items(dir: &Path) -> Option<u32> {
    let entries = fs::read_dir(dir).ok()?;
    Some(count(
        entries
            .filter_map(Result::ok)
            .filter(|e| !e.file_name().as_encoded_bytes().starts_with(b"."))
            .count(),
    ))
}

fn fs_label(kind: &FsKind) -> String {
    match kind {
        FsKind::Apfs => "APFS".into(),
        FsKind::HfsPlus => "Mac OS Extended".into(),
        FsKind::ExFat => "exFAT".into(),
        FsKind::Fat => "FAT32".into(),
        FsKind::Ntfs => "NTFS".into(),
        FsKind::Smb => "network (SMB)".into(),
        FsKind::Nfs => "network (NFS)".into(),
        other => format!("{other:?}"),
    }
}

/// Engine messages start in lower case; the UI shows them as sentences.
fn sentence(text: String) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// Scans `source` (the slow part of picking a source); called without the session lock.
pub fn scan_source(source: &Source) -> Result<Scan, String> {
    scan::scan(source, &ScanOptions::default()).map_err(|e| sentence(e.to_string()))
}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(app): keep the main window's session state

Scan, filter, destination check and plan for the main window, each
change returning one SessionView (RFD §5.2). A newer scan replaces an
older one (FR-3). The copy root and the items already in it feed the
non-empty destination warning.

Refs: #15
EOF
```


---

### Task 3: Jobs: run a copy from the app (`jobs`)

`Jobs` runs one job at a time on its own thread with `run_job`, and keeps what the UI asks
for:

- **Progress:** twice a second (`PROGRESS_INTERVAL`, NFR-5), through a `ProgressSink`: a
  Tauri channel in the app, a collector in tests. Files of 8 MiB or more get their own
  active row; smaller ones are summed into one row. A final message with phase `done` is
  sent after the job's results are stored, so the UI can fetch the summary right away.
- **The finished list:** fetched a page at a time from the stored outcomes, optionally only
  the failures.
- **The summary**, built from the engine's `Report` counts.
- **The report:** saved to the app's data folder, named like the checksum file (FR-35);
  "Save report…" writes a copy anywhere.
- **Retry failed:** returns the failed files as a `Selection` (via `Selection::subset`)
  with the job's source.

**Files:**
- Create: `crates/secopy-app/src/jobs.rs`
- Modify: `crates/secopy-app/src/dto.rs`, `crates/secopy-app/src/lib.rs`, `crates/secopy-app/src/session.rs`

**Interfaces:**
- Consumes: `session::Ready`; `secopy-core`'s `run_job`, `Report`, `JobControl`.
- Produces: `jobs::{Jobs, ProgressSink, PROGRESS_INTERVAL}`; `Jobs::{new(reports_dir), start(Ready, verify, impl ProgressSink) -> Result<(), String>, is_running, pause, resume, cancel, wait, finished_page(offset, limit, failed_only) -> Vec<FinishedRow>, summary() -> Option<SummaryView>, save_report(&Path) -> Result<(), String>, retry() -> Option<(Source, Selection)>}`.
  `Session::ready()` now returns `Ready` (with the copy root) instead of a tuple.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-app/src/jobs.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, scan_source};

    #[derive(Clone, Default)]
    struct Collect(Arc<Mutex<Vec<ProgressView>>>);

    impl ProgressSink for Collect {
        fn send(&self, view: ProgressView) {
            self.0.lock().unwrap().push(view);
        }
    }

    impl Collect {
        fn last(&self) -> ProgressView {
            self.0.lock().unwrap().last().cloned().unwrap()
        }
    }

    struct Fixture {
        dir: tempfile::TempDir,
        dest: PathBuf,
        session: Session,
        jobs: Jobs,
    }

    /// CARD/ with `n` files of `size` bytes, picked, and dest/ chosen.
    fn fixture(n: usize, size: usize) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        for i in 0..n {
            fs::write(card.join(format!("C{i:04}.mov")), vec![i as u8; size]).unwrap();
        }
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let mut session = Session::new();
        let ticket = session.begin_scan();
        let source = Session::source_for(std::slice::from_ref(&card), false).unwrap();
        let scan = scan_source(&source).unwrap();
        session.finish_scan(ticket, source, scan);
        session.set_destination(Some(dest.clone()));
        let jobs = Jobs::new(dir.path().join("reports"));
        Fixture {
            dir,
            dest,
            session,
            jobs,
        }
    }

    fn run(f: &Fixture, verify: bool) -> Collect {
        let sink = Collect::default();
        f.jobs
            .start(f.session.ready().unwrap(), verify, sink.clone())
            .unwrap();
        f.jobs.wait();
        sink
    }

    #[test]
    fn a_job_copies_verifies_and_ends_with_a_done_view() {
        let f = fixture(5, 1000);
        let sink = run(&f, true);
        let last = sink.last();
        assert_eq!(last.phase, JobPhase::Done);
        assert_eq!(last.files_done, 5);
        assert_eq!(last.verified_bytes, 5000.0);
        assert!(!f.jobs.is_running());
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Complete);
        assert_eq!((s.files, s.verified, s.failed), (5, 5, 0));
        assert_eq!(s.copy_root, show(&f.dest.join("CARD")));
        assert!(s.checksum_file.is_some());
        assert!(f.dest.join("CARD/C0004.mov").is_file());
    }

    #[test]
    fn the_report_is_saved_in_the_reports_folder() {
        let f = fixture(2, 10);
        run(&f, false);
        let report = PathBuf::from(f.jobs.summary().unwrap().report_file.unwrap());
        assert!(report.starts_with(f.dir.path().join("reports")));
        assert!(
            fs::read_to_string(&report)
                .unwrap()
                .contains("Result:       complete")
        );
        assert!(report.with_extension("json").is_file());
    }

    #[test]
    fn finished_rows_come_in_pages() {
        let f = fixture(7, 10);
        run(&f, false);
        let first = f.jobs.finished_page(0, 5, false);
        let rest = f.jobs.finished_page(5, 5, false);
        assert_eq!((first.len(), rest.len()), (5, 2));
        assert!(
            first
                .iter()
                .all(|r| r.status == RowStatus::Copied && r.hash.is_some())
        );
        assert!(f.jobs.finished_page(0, 10, true).is_empty(), "no failures");
    }

    #[test]
    fn save_report_writes_text_and_json() {
        let f = fixture(1, 10);
        run(&f, true);
        let path = f.dir.path().join("mine.txt");
        f.jobs.save_report(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().starts_with("Secopy "));
        assert!(path.with_extension("json").is_file());
    }

    #[test]
    fn cancelling_ends_the_job_as_cancelled() {
        let f = fixture(200, 50_000);
        let sink = Collect::default();
        f.jobs
            .start(f.session.ready().unwrap(), true, sink.clone())
            .unwrap();
        f.jobs.cancel();
        f.jobs.wait();
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Cancelled);
        assert_eq!(sink.last().phase, JobPhase::Done);
    }

    #[test]
    fn a_second_job_cannot_start_while_one_runs() {
        let f = fixture(200, 50_000);
        f.jobs
            .start(f.session.ready().unwrap(), true, Collect::default())
            .unwrap();
        f.jobs.pause();
        let err = f
            .jobs
            .start(f.session.ready().unwrap(), true, Collect::default())
            .unwrap_err();
        assert!(err.contains("already running"));
        f.jobs.cancel();
        f.jobs.wait();
    }

    #[test]
    fn a_paused_job_reports_paused_and_finishes_after_resume() {
        let f = fixture(50, 20_000);
        let sink = Collect::default();
        f.jobs
            .start(f.session.ready().unwrap(), true, sink.clone())
            .unwrap();
        f.jobs.pause();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !sink.0.lock().unwrap().iter().any(|v| v.paused) {
            assert!(Instant::now() < deadline, "no paused progress");
            std::thread::sleep(Duration::from_millis(20));
        }
        f.jobs.resume();
        f.jobs.wait();
        assert_eq!(f.jobs.summary().unwrap().outcome, JobOutcome::Complete);
    }

    #[cfg(unix)]
    #[test]
    fn retry_offers_only_the_failed_files() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture(3, 10);
        let locked = f.dir.path().join("CARD/C0001.mov");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&locked).is_ok() {
            return; // running as root: permissions are not enforced
        }
        run(&f, true);
        let s = f.jobs.summary().unwrap();
        assert_eq!((s.outcome, s.failed), (JobOutcome::Failures, 1));
        assert_eq!(
            s.failures[0].path,
            show(&Path::new("CARD").join("C0001.mov"))
        );
        assert!(
            s.failures[0]
                .reason
                .as_deref()
                .unwrap()
                .starts_with("Cannot read source")
        );
        let (_, sel) = f.jobs.retry().unwrap();
        assert_eq!(sel.files.len(), 1);
        assert_eq!(f.jobs.finished_page(0, 10, true).len(), 1);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-app --lib jobs`
Expected: compile errors: `jobs` doesn't exist and `Ready` isn't defined.

- [ ] **Step 3: Implement**

Change `crates/secopy-app/src/dto.rs`:

```diff
diff --git a/crates/secopy-app/src/dto.rs b/crates/secopy-app/src/dto.rs
index b3a7e79..d6a5c58 100644
--- a/crates/secopy-app/src/dto.rs
+++ b/crates/secopy-app/src/dto.rs
@@ -191,8 +191,8 @@ pub enum RowStatus {
 #[serde(rename_all = "camelCase")]
 pub struct SummaryView {
     pub outcome: JobOutcome,
-    /// "All 1,000 files copied and verified", "3 files failed", …
-    pub headline: String,
+    /// Why the job stopped, for `JobOutcome::Stopped`.
+    pub stopped_because: Option<String>,
     pub verify: bool,
     pub files: u32,
     pub copied: u32,
```

Add the implementation at the top of `crates/secopy-app/src/jobs.rs`, above the tests:

```rust
//! Runs one copy job at a time on its own thread and keeps what the UI asks for: progress
//! twice a second, pages of finished files, the summary and the report (RFD §5.3, §5.4,
//! FR-35).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use secopy_core::checksum_file;
use secopy_core::control::JobControl;
use secopy_core::job::{
    Event, FileOutcome, FileStatus, JobOptions, JobReport, Progress, SkipReason, run_job,
};
use secopy_core::plan::Plan;
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::Selection;
use secopy_core::source::Source;

use crate::dto::{
    ActiveFileView, FinishedRow, JobOutcome, JobPhase, ProgressView, RowStatus, SmallFilesView,
    SummaryView, bytes, count, show,
};
use crate::session::Ready;

/// Progress reaches the UI twice a second (NFR-5).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
/// Files at least this big get their own row in the active list (RFD §5.3).
const OWN_ROW: u64 = 8 << 20;
/// Failures listed in the summary; the finished list has all of them.
const FAILURES_SHOWN: usize = 1000;

/// Where progress goes: a Tauri channel in the app, a collector in tests.
pub trait ProgressSink: Send + Sync + 'static {
    fn send(&self, view: ProgressView);
}

/// The one job the app runs at a time.
pub struct Jobs {
    reports_dir: PathBuf,
    current: Mutex<Option<Arc<Job>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

struct Job {
    ready: Ready,
    verify: bool,
    control: JobControl,
    started: DateTime<Local>,
    clock: Instant,
    /// In the order files finished.
    outcomes: Mutex<Vec<FileOutcome>>,
    failed: AtomicU32,
    done: Mutex<Option<Done>>,
}

struct Done {
    report: JobReport,
    finished: DateTime<Local>,
    report_file: Option<PathBuf>,
}

impl Jobs {
    /// `reports_dir` is where every job's report is saved (FR-35).
    pub fn new(reports_dir: PathBuf) -> Self {
        Self {
            reports_dir,
            current: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts copying `ready`. Fails if a job is already running.
    pub fn start(&self, ready: Ready, verify: bool, sink: impl ProgressSink) -> Result<(), String> {
        if self.is_running() {
            return Err("A copy is already running.".into());
        }
        let job = Arc::new(Job {
            ready,
            verify,
            control: JobControl::new(),
            started: Local::now(),
            clock: Instant::now(),
            outcomes: Mutex::new(Vec::new()),
            failed: AtomicU32::new(0),
            done: Mutex::new(None),
        });
        *self.current.lock().expect("jobs lock poisoned") = Some(job.clone());
        let reports_dir = self.reports_dir.clone();
        let handle = std::thread::spawn(move || job.run(&sink, &reports_dir));
        *self.thread.lock().expect("jobs lock poisoned") = Some(handle);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.job()
            .is_some_and(|j| j.done.lock().expect("job lock poisoned").is_none())
    }

    pub fn pause(&self) {
        if let Some(job) = self.job() {
            job.control.pause();
        }
    }

    pub fn resume(&self) {
        if let Some(job) = self.job() {
            job.control.resume();
        }
    }

    /// Stops the job: the file in progress is removed, finished files stay (FR-23).
    pub fn cancel(&self) {
        if let Some(job) = self.job() {
            job.control.cancel();
        }
    }

    /// Waits for the job's thread to end (after a cancel, or at quit).
    pub fn wait(&self) {
        let handle = self.thread.lock().expect("jobs lock poisoned").take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    /// Rows of the finished list, in the order files finished.
    pub fn finished_page(&self, offset: u32, limit: u32, failed_only: bool) -> Vec<FinishedRow> {
        let Some(job) = self.job() else {
            return Vec::new();
        };
        let outcomes = job.outcomes.lock().expect("job lock poisoned");
        outcomes
            .iter()
            .filter(|o| !failed_only || matches!(o.status, FileStatus::Failed(_)))
            .skip(offset as usize)
            .take(limit as usize)
            .map(row)
            .collect()
    }

    /// The summary once the job has ended; `None` while it runs.
    pub fn summary(&self) -> Option<SummaryView> {
        let job = self.job()?;
        let done = job.done.lock().expect("job lock poisoned");
        let done = done.as_ref()?;
        let report = job.report(done);
        let outcomes = job.outcomes.lock().expect("job lock poisoned");
        let c = &report.counts;
        Some(SummaryView {
            outcome: if done.report.fatal.is_some() {
                JobOutcome::Stopped
            } else if done.report.cancelled {
                JobOutcome::Cancelled
            } else if c.failed > 0 {
                JobOutcome::Failures
            } else {
                JobOutcome::Complete
            },
            stopped_because: done.report.fatal.as_ref().map(|f| sentence(&f.to_string())),
            verify: job.verify,
            files: count(c.files),
            copied: count(c.copied),
            verified: count(c.verified),
            skipped_identical: count(c.skipped_identical),
            skipped_different: count(c.skipped_different),
            failed: count(c.failed),
            not_started: count(c.not_started),
            bytes_written: bytes(c.bytes_written),
            seconds: done.report.elapsed.as_secs_f64(),
            failures: outcomes
                .iter()
                .filter(|o| matches!(o.status, FileStatus::Failed(_)))
                .take(FAILURES_SHOWN)
                .map(row)
                .collect(),
            copy_root: show(&job.ready.copy_root),
            checksum_file: done.report.checksum_file.as_deref().map(show),
            checksum_error: done.report.checksum_error.clone(),
            report_file: done.report_file.as_deref().map(show),
        })
    }

    /// "Save report…": the text report at `path` and the JSON next to it (FR-35).
    pub fn save_report(&self, path: &Path) -> Result<(), String> {
        let job = self.job().ok_or("There is no report yet.")?;
        let done = job.done.lock().expect("job lock poisoned");
        let done = done.as_ref().ok_or("The copy is still running.")?;
        let report = job.report(done);
        let write = |p: &Path, body: String| fs::write(p, body).map_err(|e| e.to_string());
        write(path, report.to_text())?;
        write(&path.with_extension("json"), report.to_json())
    }

    /// The failed files of the last job, for "Retry failed" (RFD §5.4).
    pub fn retry(&self) -> Option<(Source, Selection)> {
        let job = self.job()?;
        let ids: Vec<usize> = job
            .outcomes
            .lock()
            .expect("job lock poisoned")
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
            .map(|o| o.id)
            .collect();
        if ids.is_empty() {
            return None;
        }
        let plan = &job.ready.plan;
        let all = Selection {
            files: plan.files.iter().map(|f| f.entry.clone()).collect(),
            dirs: plan.dirs.clone(),
            total_bytes: plan.total_bytes(),
        };
        Some((job.ready.source.clone(), all.subset(&ids)))
    }

    fn job(&self) -> Option<Arc<Job>> {
        self.current.lock().expect("jobs lock poisoned").clone()
    }
}

impl Job {
    fn run(&self, sink: &impl ProgressSink, reports_dir: &Path) {
        let opts = JobOptions {
            verify: self.verify,
            progress_interval: PROGRESS_INTERVAL,
            ..JobOptions::default()
        };
        let plan: &Plan = &self.ready.plan;
        let report = run_job(plan, &opts, &self.control, &|event| match event {
            Event::Progress(p) => sink.send(self.progress(&p, false, None)),
            Event::FileFinished(o) => {
                if matches!(o.status, FileStatus::Failed(_)) {
                    self.failed.fetch_add(1, Relaxed);
                }
                self.outcomes.lock().expect("job lock poisoned").push(o);
            }
        });
        let mut done = Done {
            finished: Local::now(),
            report_file: None,
            report,
        };
        done.report_file = self.save(&done, reports_dir);
        let fatal = done.report.fatal.as_ref().map(|f| sentence(&f.to_string()));
        let last = Progress {
            total_files: count(plan.files.len()).into(),
            ..Progress::default()
        };
        let mut view = self.progress(&last, true, fatal);
        view.copied_bytes = view.total_bytes;
        view.verified_bytes = if self.verify { view.total_bytes } else { 0.0 };
        view.files_done = count(done.report.outcomes.len());
        view.files_skipped = count(done.report.skipped().count());
        *self.done.lock().expect("job lock poisoned") = Some(done);
        sink.send(view);
    }

    fn progress(&self, p: &Progress, finished: bool, fatal: Option<String>) -> ProgressView {
        let total_bytes = self.ready.plan.bytes_to_write();
        let mut small = SmallFilesView {
            count: 0,
            size: 0.0,
            bytes_done: 0.0,
        };
        let mut active = Vec::new();
        for f in &p.active {
            if f.size >= OWN_ROW {
                active.push(ActiveFileView {
                    id: count(f.id),
                    name: f
                        .rel
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    path: show(&f.rel),
                    verifying: f.phase == secopy_core::job::Phase::Verifying,
                    size: bytes(f.size),
                    bytes_done: bytes(f.bytes_done),
                });
            } else {
                small.count += 1;
                small.size += bytes(f.size);
                small.bytes_done += bytes(f.bytes_done);
            }
        }
        let copying = p
            .active
            .iter()
            .any(|f| f.phase == secopy_core::job::Phase::Copying);
        ProgressView {
            phase: if finished {
                JobPhase::Done
            } else if self.verify && !copying && p.copied_bytes >= total_bytes {
                JobPhase::Verifying
            } else {
                JobPhase::Copying
            },
            elapsed_ms: self.clock.elapsed().as_secs_f64() * 1000.0,
            paused: p.paused,
            verify: self.verify,
            total_files: count(self.ready.plan.files.len()),
            total_bytes: bytes(total_bytes),
            copied_bytes: bytes(p.copied_bytes),
            verified_bytes: bytes(p.verified_bytes),
            files_done: count(p.files_done),
            files_skipped: count(p.files_skipped),
            files_failed: self.failed.load(Relaxed),
            active,
            small_files: (small.count > 0).then_some(small),
            fatal,
        }
    }

    fn report(&self, done: &Done) -> Report {
        let meta = JobMeta {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            source: self.ready.label.clone(),
            verify: self.verify,
            started: self.started,
            finished: done.finished,
        };
        let mut job_report = done.report.clone();
        job_report.outcomes = self.outcomes.lock().expect("job lock poisoned").clone();
        Report::new(&self.ready.plan, &job_report, &meta)
    }

    /// Saves the report in the app's data folder, named like the checksum file (FR-35).
    fn save(&self, done: &Done, reports_dir: &Path) -> Option<PathBuf> {
        fs::create_dir_all(reports_dir).ok()?;
        let stem = match &done.report.checksum_file {
            Some(path) => path.file_stem()?.to_string_lossy().into_owned(),
            None => checksum_file::file_name(self.started).replace(".xxh64", ""),
        };
        self.report(done)
            .write(reports_dir, &stem)
            .ok()
            .map(|(text, _)| text)
    }
}

fn row(o: &FileOutcome) -> FinishedRow {
    let (status, reason) = match &o.status {
        FileStatus::Copied => (RowStatus::Copied, None),
        FileStatus::Verified => (RowStatus::Verified, None),
        FileStatus::Skipped(SkipReason::Identical) => (
            RowStatus::Skipped,
            Some("Already at the destination (not checked)".to_string()),
        ),
        FileStatus::Skipped(SkipReason::Differs) => (
            RowStatus::Skipped,
            Some("A different file with this name was kept".to_string()),
        ),
        FileStatus::Failed(e) => (RowStatus::Failed, Some(sentence(&e.to_string()))),
    };
    FinishedRow {
        id: count(o.id),
        path: show(&o.rel),
        final_path: show(&o.final_rel),
        size: bytes(o.size),
        seconds: o.elapsed.as_secs_f64(),
        hash: o.hash.map(secopy_core::hash::to_hex),
        status,
        reason,
    }
}

fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
```

Change `crates/secopy-app/src/lib.rs`:

```diff
diff --git a/crates/secopy-app/src/lib.rs b/crates/secopy-app/src/lib.rs
index 0b1b0ff..67530db 100644
--- a/crates/secopy-app/src/lib.rs
+++ b/crates/secopy-app/src/lib.rs
@@ -1,6 +1,7 @@
 //! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.
 
 pub mod dto;
+pub mod jobs;
 pub mod session;
 
 /// Starts the app. Blocks until the last window closes.
```

Change `crates/secopy-app/src/session.rs`:

```diff
diff --git a/crates/secopy-app/src/session.rs b/crates/secopy-app/src/session.rs
index b857460..131d95a 100644
--- a/crates/secopy-app/src/session.rs
+++ b/crates/secopy-app/src/session.rs
@@ -35,6 +35,16 @@ pub struct Session {
     plan: Option<Plan>,
 }
 
+/// Everything a job needs from the main window.
+pub struct Ready {
+    pub source: Source,
+    pub plan: Plan,
+    /// The source as shown, for the report.
+    pub label: String,
+    /// Where the files land, for Reveal in Finder.
+    pub copy_root: PathBuf,
+}
+
 struct Picked {
     label: String,
     source: Source,
@@ -137,12 +147,16 @@ impl Session {
         self.view()
     }
 
-    /// The source and plan to start a job with; `None` while anything blocks Start.
-    pub fn ready(&self) -> Option<(&Source, &Plan, String)> {
+    /// What a job starts with; `None` while anything blocks Start.
+    pub fn ready(&self) -> Option<Ready> {
         let picked = self.source.as_ref()?;
         let plan = self.plan.as_ref()?;
-        (plan.blockers().is_empty() && !plan.files.is_empty())
-            .then(|| (&picked.source, plan, picked.label.clone()))
+        (plan.blockers().is_empty() && !plan.files.is_empty()).then(|| Ready {
+            source: picked.source.clone(),
+            plan: plan.clone(),
+            label: picked.label.clone(),
+            copy_root: self.copy_root(&plan.dest),
+        })
     }
 
     fn recompute(&mut self) {
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

The pause and cancel tests use threads. Run them a few times:
`for i in $(seq 1 10); do cargo test -q -p secopy-app || break; done`. Expected: 10 clean runs.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(app): run copy jobs from the app

One job at a time on its own thread: progress twice a second through a
ProgressSink, the finished list in pages, the summary, the report saved
in the app's data folder (FR-35), and the failed files for Retry failed.

Refs: #15
EOF
```


---

### Task 4: Commands and generated TypeScript bindings

Thin `#[tauri::command]` wrappers around `session` and `jobs`. Anything that touches the
disk runs on a blocking thread (`spawn_blocking`), so the window never freezes. Tauri runs
non-async commands on the main thread, so only the instant ones (pause, resume, cancel,
running) are sync.

`tauri-specta` generates `ui/src/lib/bindings.ts` from the commands and DTOs. A test fails
when the committed file is out of date; `SECOPY_UPDATE_BINDINGS=1` rewrites it.

`run()` builds the app. On `RunEvent::ExitRequested` it cancels a running job and waits for
it, so quitting mid-copy leaves no partial file behind.

**Files:**
- Create: `crates/secopy-app/src/commands.rs`
- Modify: `crates/secopy-app/src/dto.rs`, `crates/secopy-app/src/jobs.rs`, `crates/secopy-app/src/lib.rs`, `crates/secopy-app/src/session.rs`
- Generated: `ui/src/lib/bindings.ts` (from `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app`; commit it)

**Interfaces:**
- Consumes: Tasks 2–3.
- Produces: `commands::AppState { session, jobs }`; the commands `scan_source, clear_source,
  set_filter, set_destination, set_conflicts, session_view, start_job, pause_job, resume_job,
  cancel_job, job_running, finished_page, job_summary, save_report, retry_failed`;
  `secopy_app::{specta_builder, export_bindings}`; in TypeScript, `commands.*` and every DTO
  type, exported from `ui/src/lib/bindings.ts`.

- [ ] **Step 1: Implement**

After adding the code, generate the bindings once:
`SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app ui_bindings`. Check that the only
`number | null` in the file is `existingItems`.

Create `crates/secopy-app/src/commands.rs`:

```rust
//! The commands the UI calls (see `ui/src/lib/bindings.ts`, generated from these). Thin
//! wrappers: the work is in `session` and `jobs`, run off the main thread so the window
//! never freezes (NFR-5).

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::dto::{
    ConflictPolicy, ExtensionKey, FinishedRow, ProgressView, SessionView, SummaryView,
};
use crate::jobs::{Jobs, ProgressSink};
use crate::session::{Session, scan_source as scan};

/// Everything the app keeps between commands.
pub struct AppState {
    pub session: Mutex<Session>,
    pub jobs: Jobs,
}

impl AppState {
    pub fn new(reports_dir: PathBuf) -> Self {
        Self {
            session: Mutex::new(Session::new()),
            jobs: Jobs::new(reports_dir),
        }
    }
}

impl ProgressSink for Channel<ProgressView> {
    fn send(&self, view: ProgressView) {
        // The window may be gone (the app is quitting); the job carries on regardless.
        let _ = Channel::send(self, view);
    }
}

/// Runs `f` on a blocking thread with the app state: scanning and checking touch the disk.
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())
}

fn session(state: &AppState) -> std::sync::MutexGuard<'_, Session> {
    state.session.lock().expect("session lock poisoned")
}

/// Scans a picked or dropped source (FR-1..FR-3). A newer scan replaces an older one.
#[tauri::command]
#[specta::specta]
pub async fn scan_source(
    app: AppHandle,
    paths: Vec<String>,
    contents_only: bool,
) -> Result<SessionView, String> {
    blocking(app, move |state| {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let source = Session::source_for(&paths, contents_only)?;
        let ticket = session(state).begin_scan();
        let scanned = scan(&source)?;
        Ok(session(state).finish_scan(ticket, source, scanned))
    })
    .await?
}

/// Clears the source; the destination stays ("New copy", RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn clear_source(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| session(state).clear_source()).await
}

/// `None` selects every extension (FR-8, FR-10).
#[tauri::command]
#[specta::specta]
pub async fn set_filter(
    app: AppHandle,
    selected: Option<Vec<ExtensionKey>>,
) -> Result<SessionView, String> {
    blocking(app, move |state| session(state).set_filter(selected)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_destination(app: AppHandle, path: Option<String>) -> Result<SessionView, String> {
    blocking(app, move |state| {
        session(state).set_destination(path.map(PathBuf::from))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_conflicts(app: AppHandle, policy: ConflictPolicy) -> Result<SessionView, String> {
    blocking(app, move |state| session(state).set_policy(policy)).await
}

#[tauri::command]
#[specta::specta]
pub async fn session_view(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| session(state).view()).await
}

/// Starts copying what the main window shows; progress arrives on `on_progress`.
#[tauri::command]
#[specta::specta]
pub async fn start_job(
    app: AppHandle,
    verify: bool,
    on_progress: Channel<ProgressView>,
) -> Result<(), String> {
    blocking(app, move |state| {
        let ready = session(state)
            .ready()
            .ok_or("Nothing to copy, or something blocks the copy.")?;
        state.jobs.start(ready, verify, on_progress)
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub fn pause_job(app: AppHandle) {
    app.state::<AppState>().jobs.pause();
}

#[tauri::command]
#[specta::specta]
pub fn resume_job(app: AppHandle) {
    app.state::<AppState>().jobs.resume();
}

#[tauri::command]
#[specta::specta]
pub fn cancel_job(app: AppHandle) {
    app.state::<AppState>().jobs.cancel();
}

#[tauri::command]
#[specta::specta]
pub fn job_running(app: AppHandle) -> bool {
    app.state::<AppState>().jobs.is_running()
}

#[tauri::command]
#[specta::specta]
pub async fn finished_page(
    app: AppHandle,
    offset: u32,
    limit: u32,
    failed_only: bool,
) -> Result<Vec<FinishedRow>, String> {
    blocking(app, move |state| {
        state.jobs.finished_page(offset, limit, failed_only)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn job_summary(app: AppHandle) -> Result<Option<SummaryView>, String> {
    blocking(app, |state| state.jobs.summary()).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_report(app: AppHandle, path: String) -> Result<(), String> {
    blocking(app, move |state| {
        state.jobs.save_report(&PathBuf::from(path))
    })
    .await?
}

/// "Retry failed": only the failed files, checked again (RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn retry_failed(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| {
        let (source, selection) = state.jobs.retry().ok_or("No files failed.")?;
        Ok(session(state).install_retry(source, selection))
    })
    .await?
}
```

Change `crates/secopy-app/src/dto.rs`:

```diff
diff --git a/crates/secopy-app/src/dto.rs b/crates/secopy-app/src/dto.rs
index d6a5c58..88c4708 100644
--- a/crates/secopy-app/src/dto.rs
+++ b/crates/secopy-app/src/dto.rs
@@ -1,6 +1,7 @@
 //! The data the UI sees (RFD §5.2–§5.4). Plain, serializable summaries: paths are strings
-//! (names that aren't UTF-8 are shown lossily), byte counts are `f64` (exact up to 9 PB,
-//! and JavaScript has no 64-bit integers), counts are `u32`.
+//! (names that aren't UTF-8 are shown lossily), counts are `u32`. Bytes and milliseconds
+//! are `u64` exported as a TypeScript `number`, marked field by field: JavaScript numbers
+//! are exact up to 2^53, which is 9 PB or 285,000 years.
 
 use std::path::Path;
 
@@ -15,7 +16,8 @@ pub struct SessionView {
     pub source: Option<SourceView>,
     /// Files and bytes the extension filter keeps.
     pub selected_files: u32,
-    pub selected_bytes: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub selected_bytes: u64,
     pub destination: Option<DestinationView>,
     pub conflicts: ConflictPolicy,
     pub plan: Option<PlanView>,
@@ -34,7 +36,8 @@ pub struct SourceView {
     /// The folder created for "copy the folder itself", e.g. "CLIP".
     pub root_dir: Option<String>,
     pub files: u32,
-    pub bytes: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub bytes: u64,
     /// Sorted by bytes, largest first (FR-8).
     pub extensions: Vec<ExtensionView>,
     /// `None` = every extension; otherwise the selected keys (FR-8).
@@ -56,7 +59,8 @@ pub struct ExtensionView {
     /// ".mov", or "(no extension)".
     pub label: String,
     pub files: u32,
-    pub bytes: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub bytes: u64,
 }
 
 #[derive(Debug, Clone, PartialEq, Serialize, Type)]
@@ -67,7 +71,8 @@ pub struct DestinationView {
     pub copy_root: String,
     /// Stops the job (FR-16); Start stays disabled.
     pub blocker: Option<String>,
-    pub free_bytes: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub free_bytes: u64,
     pub fs_kind: String,
     /// Items already in the copy root, hidden ones not counted; `None` if it doesn't exist.
     /// More than zero shows the non-empty warning.
@@ -104,7 +109,8 @@ pub enum ConflictPolicy {
 #[serde(rename_all = "camelCase")]
 pub struct PlanView {
     pub files_to_write: u32,
-    pub bytes_to_write: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub bytes_to_write: u64,
     /// Not enough free space (FR-16).
     pub blocker: Option<String>,
 }
@@ -114,14 +120,18 @@ pub struct PlanView {
 #[serde(rename_all = "camelCase")]
 pub struct ProgressView {
     pub phase: JobPhase,
-    pub elapsed_ms: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub elapsed_ms: u64,
     pub paused: bool,
     pub verify: bool,
     pub total_files: u32,
     /// Bytes the job writes; skipped files are not included.
-    pub total_bytes: f64,
-    pub copied_bytes: f64,
-    pub verified_bytes: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub total_bytes: u64,
+    #[specta(type = specta_typescript::Number)]
+    pub copied_bytes: u64,
+    #[specta(type = specta_typescript::Number)]
+    pub verified_bytes: u64,
     pub files_done: u32,
     pub files_skipped: u32,
     pub files_failed: u32,
@@ -149,16 +159,20 @@ pub struct ActiveFileView {
     pub name: String,
     pub path: String,
     pub verifying: bool,
-    pub size: f64,
-    pub bytes_done: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub size: u64,
+    #[specta(type = specta_typescript::Number)]
+    pub bytes_done: u64,
 }
 
 #[derive(Debug, Clone, PartialEq, Serialize, Type)]
 #[serde(rename_all = "camelCase")]
 pub struct SmallFilesView {
     pub count: u32,
-    pub size: f64,
-    pub bytes_done: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub size: u64,
+    #[specta(type = specta_typescript::Number)]
+    pub bytes_done: u64,
 }
 
 /// One row of the finished list (RFD §5.3).
@@ -169,8 +183,10 @@ pub struct FinishedRow {
     pub path: String,
     /// Differs from `path` when the copy was kept under a new name.
     pub final_path: String,
-    pub size: f64,
-    pub seconds: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub size: u64,
+    #[specta(type = specta_typescript::Number)]
+    pub millis: u64,
     pub hash: Option<String>,
     pub status: RowStatus,
     /// Why it failed or was skipped.
@@ -201,8 +217,10 @@ pub struct SummaryView {
     pub skipped_different: u32,
     pub failed: u32,
     pub not_started: u32,
-    pub bytes_written: f64,
-    pub seconds: f64,
+    #[specta(type = specta_typescript::Number)]
+    pub bytes_written: u64,
+    #[specta(type = specta_typescript::Number)]
+    pub millis: u64,
     /// Failed files with their reasons, first 1,000.
     pub failures: Vec<FinishedRow>,
     pub copy_root: String,
@@ -230,8 +248,3 @@ pub fn show(path: &Path) -> String {
 pub fn count(n: impl TryInto<u32>) -> u32 {
     n.try_into().unwrap_or(u32::MAX)
 }
-
-/// Bytes shown in the UI.
-pub fn bytes(n: u64) -> f64 {
-    n as f64
-}
```

Change `crates/secopy-app/src/jobs.rs`:

```diff
diff --git a/crates/secopy-app/src/jobs.rs b/crates/secopy-app/src/jobs.rs
index 21eda51..f4c4687 100644
--- a/crates/secopy-app/src/jobs.rs
+++ b/crates/secopy-app/src/jobs.rs
@@ -22,7 +22,7 @@ use secopy_core::source::Source;
 
 use crate::dto::{
     ActiveFileView, FinishedRow, JobOutcome, JobPhase, ProgressView, RowStatus, SmallFilesView,
-    SummaryView, bytes, count, show,
+    SummaryView, count, show,
 };
 use crate::session::Ready;
 
@@ -169,8 +169,8 @@ impl Jobs {
             skipped_different: count(c.skipped_different),
             failed: count(c.failed),
             not_started: count(c.not_started),
-            bytes_written: bytes(c.bytes_written),
-            seconds: done.report.elapsed.as_secs_f64(),
+            bytes_written: c.bytes_written,
+            millis: done.report.elapsed.as_millis() as u64,
             failures: outcomes
                 .iter()
                 .filter(|o| matches!(o.status, FileStatus::Failed(_)))
@@ -253,7 +253,7 @@ impl Job {
         };
         let mut view = self.progress(&last, true, fatal);
         view.copied_bytes = view.total_bytes;
-        view.verified_bytes = if self.verify { view.total_bytes } else { 0.0 };
+        view.verified_bytes = if self.verify { view.total_bytes } else { 0 };
         view.files_done = count(done.report.outcomes.len());
         view.files_skipped = count(done.report.skipped().count());
         *self.done.lock().expect("job lock poisoned") = Some(done);
@@ -264,8 +264,8 @@ impl Job {
         let total_bytes = self.ready.plan.bytes_to_write();
         let mut small = SmallFilesView {
             count: 0,
-            size: 0.0,
-            bytes_done: 0.0,
+            size: 0,
+            bytes_done: 0,
         };
         let mut active = Vec::new();
         for f in &p.active {
@@ -279,13 +279,13 @@ impl Job {
                         .unwrap_or_default(),
                     path: show(&f.rel),
                     verifying: f.phase == secopy_core::job::Phase::Verifying,
-                    size: bytes(f.size),
-                    bytes_done: bytes(f.bytes_done),
+                    size: f.size,
+                    bytes_done: f.bytes_done,
                 });
             } else {
                 small.count += 1;
-                small.size += bytes(f.size);
-                small.bytes_done += bytes(f.bytes_done);
+                small.size += f.size;
+                small.bytes_done += f.bytes_done;
             }
         }
         let copying = p
@@ -300,13 +300,13 @@ impl Job {
             } else {
                 JobPhase::Copying
             },
-            elapsed_ms: self.clock.elapsed().as_secs_f64() * 1000.0,
+            elapsed_ms: self.clock.elapsed().as_millis() as u64,
             paused: p.paused,
             verify: self.verify,
             total_files: count(self.ready.plan.files.len()),
-            total_bytes: bytes(total_bytes),
-            copied_bytes: bytes(p.copied_bytes),
-            verified_bytes: bytes(p.verified_bytes),
+            total_bytes,
+            copied_bytes: p.copied_bytes,
+            verified_bytes: p.verified_bytes,
             files_done: count(p.files_done),
             files_skipped: count(p.files_skipped),
             files_failed: self.failed.load(Relaxed),
@@ -361,8 +361,8 @@ fn row(o: &FileOutcome) -> FinishedRow {
         id: count(o.id),
         path: show(&o.rel),
         final_path: show(&o.final_rel),
-        size: bytes(o.size),
-        seconds: o.elapsed.as_secs_f64(),
+        size: o.size,
+        millis: o.elapsed.as_millis() as u64,
         hash: o.hash.map(secopy_core::hash::to_hex),
         status,
         reason,
@@ -445,7 +445,7 @@ mod tests {
         let last = sink.last();
         assert_eq!(last.phase, JobPhase::Done);
         assert_eq!(last.files_done, 5);
-        assert_eq!(last.verified_bytes, 5000.0);
+        assert_eq!(last.verified_bytes, 5000);
         assert!(!f.jobs.is_running());
         let s = f.jobs.summary().unwrap();
         assert_eq!(s.outcome, JobOutcome::Complete);
```

Replace `crates/secopy-app/src/lib.rs` with:

```rust
//! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.

pub mod commands;
pub mod dto;
pub mod jobs;
pub mod session;

use tauri::{Manager, RunEvent};

use commands::AppState;

/// The commands and types the UI sees; `ui/src/lib/bindings.ts` is generated from this.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::scan_source,
        commands::clear_source,
        commands::set_filter,
        commands::set_destination,
        commands::set_conflicts,
        commands::session_view,
        commands::start_job,
        commands::pause_job,
        commands::resume_job,
        commands::cancel_job,
        commands::job_running,
        commands::finished_page,
        commands::job_summary,
        commands::save_report,
        commands::retry_failed,
    ])
}

/// Writes the TypeScript bindings for the UI.
pub fn export_bindings(path: &std::path::Path) -> Result<(), String> {
    specta_builder()
        .export(
            specta_typescript::Typescript::default().header(
                "// Generated from crates/secopy-app by `cargo test -p secopy-app`. Don't edit.\n",
            ),
            path,
        )
        .map_err(|e| e.to_string())
}

/// Starts the app. Blocks until it quits.
pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let reports = app.path().app_data_dir()?.join("reports");
            app.manage(AppState::new(reports));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start Secopy")
        .run(|app, event| {
            // Quitting during a copy: stop it first, so no partial file is left behind.
            if let RunEvent::ExitRequested { .. } = event {
                let jobs = &app.state::<AppState>().jobs;
                jobs.cancel();
                jobs.wait();
            }
        });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The UI's bindings must match the Rust commands. Set `SECOPY_UPDATE_BINDINGS=1` to
    /// rewrite them after changing a command or a DTO.
    #[test]
    fn ui_bindings_are_up_to_date() {
        let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/lib/bindings.ts");
        if std::env::var("SECOPY_UPDATE_BINDINGS").is_ok_and(|v| v == "1") {
            super::export_bindings(&committed).unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("bindings.ts");
        super::export_bindings(&fresh).unwrap();
        assert_eq!(
            std::fs::read_to_string(&committed).unwrap_or_default(),
            std::fs::read_to_string(&fresh).unwrap(),
            "ui/src/lib/bindings.ts is out of date: run SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app"
        );
    }
}
```

Change `crates/secopy-app/src/session.rs`:

```diff
diff --git a/crates/secopy-app/src/session.rs b/crates/secopy-app/src/session.rs
index 131d95a..5d31d7e 100644
--- a/crates/secopy-app/src/session.rs
+++ b/crates/secopy-app/src/session.rs
@@ -14,7 +14,7 @@ use secopy_core::source::{DirMode, Source};
 
 use crate::dto::{
     ConflictPolicy, DestinationView, ExtensionKey, ExtensionView, FileProblemView, PlanView,
-    SessionView, SourceView, bytes, count, show,
+    SessionView, SourceView, count, show,
 };
 
 /// Per-file problems sent to the UI; the rest are only counted.
@@ -184,7 +184,7 @@ impl Session {
         SessionView {
             source: self.source.as_ref().map(|p| self.source_view(p)),
             selected_files: selection.map_or(0, |s| count(s.files.len())),
-            selected_bytes: selection.map_or(0.0, |s| bytes(s.total_bytes)),
+            selected_bytes: selection.map_or(0, |s| s.total_bytes),
             destination: self.dest.as_ref().map(|d| self.destination_view(d)),
             conflicts: self.policy,
             plan: self.plan.as_ref().map(plan_view),
@@ -203,10 +203,10 @@ impl Session {
                     .as_ref()
                     .map_or("(no extension)".to_string(), |k| format!(".{k}")),
                 files: count(stat.files),
-                bytes: bytes(stat.bytes),
+                bytes: stat.bytes,
             })
             .collect();
-        extensions.sort_by(|a, b| b.bytes.total_cmp(&a.bytes));
+        extensions.sort_by_key(|e| std::cmp::Reverse(e.bytes));
         let (is_folder, contents_only) = match &picked.source {
             Source::Directory { mode, .. } => (true, *mode == DirMode::ContentsOnly),
             Source::Files(_) => (false, false),
@@ -217,7 +217,7 @@ impl Session {
             contents_only,
             root_dir: scan.root_dir.as_deref().map(show),
             files: count(scan.files.len()),
-            bytes: bytes(scan.files.iter().map(|f| f.size).sum()),
+            bytes: scan.files.iter().map(|f| f.size).sum(),
             extensions,
             selected_extensions: match &self.filter {
                 ExtensionFilter::All => None,
@@ -241,7 +241,7 @@ impl Session {
             path: show(dest),
             copy_root: show(&copy_root),
             blocker: None,
-            free_bytes: 0.0,
+            free_bytes: 0,
             fs_kind: String::new(),
             existing_items: existing_items(&copy_root),
             problems: Vec::new(),
@@ -256,7 +256,7 @@ impl Session {
                     .selection
                     .as_ref()
                     .expect("checked implies a selection");
-                view.free_bytes = bytes(pf.fs.free_bytes);
+                view.free_bytes = pf.fs.free_bytes;
                 view.fs_kind = fs_label(&pf.fs.kind);
                 view.problems = pf
                     .file_problems
@@ -281,7 +281,7 @@ impl Session {
             // No source yet: show what the destination is, or why it can't be used.
             None => match fsinfo::fs_info(dest) {
                 Ok(info) => {
-                    view.free_bytes = bytes(info.free_bytes);
+                    view.free_bytes = info.free_bytes;
                     view.fs_kind = fs_label(&info.kind);
                 }
                 Err(_) if !dest.is_dir() => {
@@ -316,7 +316,7 @@ fn policy(p: ConflictPolicy) -> DiffersPolicy {
 fn plan_view(plan: &Plan) -> PlanView {
     PlanView {
         files_to_write: count(plan.files.iter().filter(|f| f.action.writes()).count()),
-        bytes_to_write: bytes(plan.bytes_to_write()),
+        bytes_to_write: plan.bytes_to_write(),
         blocker: plan.blockers().first().map(|b| sentence(b.to_string())),
     }
 }
@@ -422,10 +422,10 @@ mod tests {
         let src = view.source.unwrap();
         assert!(src.is_folder && !src.contents_only);
         assert_eq!(src.root_dir.as_deref(), Some("CARD"));
-        assert_eq!((src.files, src.bytes), (3, 17.0));
+        assert_eq!((src.files, src.bytes), (3, 17));
         let labels: Vec<_> = src.extensions.iter().map(|e| e.label.as_str()).collect();
         assert_eq!(labels, [".mov", ".xml"]);
-        assert_eq!((view.selected_files, view.selected_bytes), (3, 17.0));
+        assert_eq!((view.selected_files, view.selected_bytes), (3, 17));
         assert_eq!(src.selected_extensions, None);
     }
 
@@ -435,7 +435,7 @@ mod tests {
         let mut s = Session::new();
         pick(&mut s, std::slice::from_ref(&f.card), false);
         let view = s.set_filter(Some(vec![Some("mov".into())]));
-        assert_eq!((view.selected_files, view.selected_bytes), (2, 14.0));
+        assert_eq!((view.selected_files, view.selected_bytes), (2, 14));
         assert_eq!(
             view.source.unwrap().selected_extensions,
             Some(vec![Some("mov".into())])
@@ -529,7 +529,7 @@ mod tests {
         let f = fixture();
         let mut s = Session::new();
         let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
-        assert!(dest.free_bytes > 0.0 && dest.blocker.is_none());
+        assert!(dest.free_bytes > 0 && dest.blocker.is_none());
         let missing = s
             .set_destination(Some(f.dest.join("nope")))
             .destination
```

- [ ] **Step 2: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(app): expose commands and generate typescript bindings

The UI's commands, run off the main thread when they touch the disk,
and ui/src/lib/bindings.ts generated from them by tauri-specta, with a
test that fails while it is out of date. Quitting during a copy cancels
it first, so no partial file is left behind.

Refs: #15
EOF
```


---

### Task 5: UI foundation: formatting, speed meters, the API

Three pieces the screens share:

- **`format.ts`:** bytes in decimal units like Finder, counts with separators, speeds,
  durations, percentages, plurals.
- **`rate.ts`:** `RateMeter` computes the current speed over the last 3 s, the average
  since the start, and the ETA, from the progress messages (RFD §5.3). It shows "—" until
  there are two samples.
- **`api.ts`:** everything the UI asks of the app: the commands (unwrapped, so errors
  arrive as thrown `Error`s with the app's message), the pickers, reveal and open, Finder
  drops and the window's close request. Components get it from the Svelte context
  (`useApi`), so tests pass `fakeApi()` from `src/test/fake-api.ts` instead.

**Files:**
- Create: `ui/src/lib/api.test.ts`, `ui/src/lib/api.ts`, `ui/src/lib/format.test.ts`, `ui/src/lib/format.ts`, `ui/src/lib/rate.test.ts`, `ui/src/lib/rate.ts`, `ui/src/test/fake-api.ts`

**Interfaces:**
- Consumes: Task 4's `bindings.ts`.
- Produces: `formatCount, formatBytes, formatSpeed, formatDuration, formatPercent, plural`;
  `RateMeter { push(t, bytes), current(), average(), eta(remaining) }`; `tauriApi`, `type Api`,
  `provideApi`, `useApi`, `apiContext`, `unwrap`; test helpers `fakeApi, sessionView,
  readyView, sourceView, destinationView, progressView, summaryView`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/api.test.ts`:

```ts
import { expect, test } from "vitest";
import { unwrap } from "./api";

test("ok results give their data", async () => {
  await expect(unwrap(Promise.resolve({ status: "ok", data: 42 }))).resolves.toBe(42);
});

test("error results throw the app's message", async () => {
  const failed = unwrap(Promise.resolve({ status: "error", error: "The destination is not an existing folder" }));
  await expect(failed).rejects.toThrow("The destination is not an existing folder");
});
```

Create `ui/src/lib/format.test.ts`:

```ts
import { describe, expect, test } from "vitest";
import { formatBytes, formatCount, formatDuration, formatPercent, formatSpeed, plural } from "./format";

describe("format", () => {
  test("counts use thousands separators", () => {
    expect(formatCount(1284)).toBe("1,284");
  });

  test("bytes use decimal units like Finder", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1000)).toBe("1.0 KB");
    expect(formatBytes(212_400_000_000)).toBe("212.4 GB");
  });

  test("speed and duration show a dash when unknown", () => {
    expect(formatSpeed(null)).toBe("—");
    expect(formatSpeed(1_210_000_000)).toBe("1.2 GB/s");
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(7_000)).toBe("0:07");
    expect(formatDuration(252_000)).toBe("4:12");
    expect(formatDuration(3_723_000)).toBe("1:02:03");
  });

  test("percent treats an empty total as done", () => {
    expect(formatPercent(0, 0)).toBe("100.0 %");
    expect(formatPercent(1482, 2124)).toBe("69.8 %");
  });

  test("plural", () => {
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(1284, "file")).toBe("1,284 files");
  });
});
```

Create `ui/src/lib/rate.test.ts`:

```ts
import { expect, test } from "vitest";
import { RateMeter } from "./rate";

test("unknown until two samples", () => {
  const m = new RateMeter();
  expect(m.current()).toBeNull();
  m.push(0, 0);
  expect(m.current()).toBeNull();
  expect(m.eta(100)).toBeNull();
});

test("speed is averaged over the last three seconds", () => {
  const m = new RateMeter();
  // 100 MB/s for 5 s, then 300 MB/s for 3 s.
  for (let t = 0; t <= 5000; t += 500) m.push(t, t * 100_000);
  const at5 = 5000 * 100_000;
  for (let t = 5500; t <= 8000; t += 500) m.push(t, at5 + (t - 5000) * 300_000);
  expect(m.current()).toBeCloseTo(300_000_000, -3);
  expect(m.average()).toBeCloseTo((at5 + 3000 * 300_000) / 8, -3);
});

test("eta from the current speed", () => {
  const m = new RateMeter();
  m.push(0, 0);
  m.push(1000, 50_000_000);
  expect(m.eta(100_000_000)).toBe(2000);
  expect(m.eta(0)).toBe(0);
});
```

Create `ui/src/test/fake-api.ts`:

```ts
// A fake `Api` for component tests: every call is recorded, and answers come from the
// views the test sets.

import { vi } from "vitest";
import type { Api } from "../lib/api";
import type {
  DestinationView,
  ProgressView,
  SessionView,
  SourceView,
  SummaryView,
} from "../lib/bindings";

export function sourceView(over: Partial<SourceView> = {}): SourceView {
  return {
    label: "/Volumes/CARD/DCIM",
    isFolder: true,
    contentsOnly: false,
    rootDir: "DCIM",
    files: 1284,
    bytes: 212_400_000_000,
    extensions: [
      { key: "mov", label: ".mov", files: 1020, bytes: 208_000_000_000 },
      { key: "wav", label: ".wav", files: 240, bytes: 4_100_000_000 },
      { key: "xml", label: ".xml", files: 24, bytes: 2_000_000 },
    ],
    selectedExtensions: null,
    skippedHidden: 37,
    skippedSymlinks: 0,
    problems: [],
    problemCount: 0,
    ...over,
  };
}

export function destinationView(over: Partial<DestinationView> = {}): DestinationView {
  return {
    path: "/Volumes/RAID/Day01",
    copyRoot: "/Volumes/RAID/Day01/DCIM",
    blocker: null,
    freeBytes: 1_800_000_000_000,
    fsKind: "APFS",
    existingItems: null,
    problems: [],
    problemCount: 0,
    identical: 0,
    differs: 0,
    stalePartials: 0,
    ...over,
  };
}

export function sessionView(over: Partial<SessionView> = {}): SessionView {
  return {
    source: null,
    selectedFiles: 0,
    selectedBytes: 0,
    destination: null,
    conflicts: "keepBoth",
    plan: null,
    stale: false,
    ...over,
  };
}

/** A session with a source and a destination, ready to start. */
export function readyView(over: Partial<SessionView> = {}): SessionView {
  return sessionView({
    source: sourceView(),
    selectedFiles: 1284,
    selectedBytes: 212_400_000_000,
    destination: destinationView(),
    plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, blocker: null },
    ...over,
  });
}

export function progressView(over: Partial<ProgressView> = {}): ProgressView {
  return {
    phase: "copying",
    elapsedMs: 0,
    paused: false,
    verify: true,
    totalFiles: 1284,
    totalBytes: 212_400_000_000,
    copiedBytes: 0,
    verifiedBytes: 0,
    filesDone: 0,
    filesSkipped: 0,
    filesFailed: 0,
    active: [],
    smallFiles: null,
    fatal: null,
    ...over,
  };
}

export function summaryView(over: Partial<SummaryView> = {}): SummaryView {
  return {
    outcome: "complete",
    stoppedBecause: null,
    verify: true,
    files: 1284,
    copied: 0,
    verified: 1284,
    skippedIdentical: 0,
    skippedDifferent: 0,
    failed: 0,
    notStarted: 0,
    bytesWritten: 212_400_000_000,
    millis: 252_000,
    failures: [],
    copyRoot: "/Volumes/RAID/Day01/DCIM",
    checksumFile: "/Volumes/RAID/Day01/secopy_2026-09-27_140302.xxh64",
    checksumError: null,
    reportFile: "/Users/me/Library/Application Support/com.belisoft.secopy/reports/r.txt",
    ...over,
  };
}

/** Every method is a spy; `session` is what the session commands answer. */
export function fakeApi(session: SessionView = sessionView()) {
  const state = {
    session,
    progress: null as ((p: ProgressView) => void) | null,
    drop: null as ((paths: string[], target: Element | null) => void) | null,
    close: null as ((prevent: () => void) => Promise<void>) | null,
  };
  const answer = () => Promise.resolve(state.session);
  const api = {
    scanSource: vi.fn(answer),
    clearSource: vi.fn(answer),
    setFilter: vi.fn(answer),
    setDestination: vi.fn(answer),
    setConflicts: vi.fn(answer),
    sessionView: vi.fn(answer),
    startJob: vi.fn((_verify: boolean, onProgress: (p: ProgressView) => void) => {
      state.progress = onProgress;
      return Promise.resolve(null);
    }),
    pauseJob: vi.fn(() => Promise.resolve()),
    resumeJob: vi.fn(() => Promise.resolve()),
    cancelJob: vi.fn(() => Promise.resolve()),
    jobRunning: vi.fn(() => Promise.resolve(false)),
    finishedPage: vi.fn((_o: number, _l: number, _f: boolean) => Promise.resolve([] as Awaited<ReturnType<Api["finishedPage"]>>)),
    jobSummary: vi.fn(() => Promise.resolve(summaryView() as SummaryView | null)),
    saveReport: vi.fn((_p: string) => Promise.resolve(null)),
    retryFailed: vi.fn(answer),
    pickFolder: vi.fn(() => Promise.resolve(["/Volumes/CARD/DCIM"] as string[] | null)),
    pickFiles: vi.fn(() => Promise.resolve(["/a.wav", "/b.wav"] as string[] | null)),
    pickDestination: vi.fn(() => Promise.resolve("/Volumes/RAID/Day01" as string | null)),
    pickReportPath: vi.fn((_s: string) => Promise.resolve("/tmp/report.txt" as string | null)),
    confirm: vi.fn((_m: string, _t: string) => Promise.resolve(true)),
    reveal: vi.fn((_p: string) => Promise.resolve()),
    openFile: vi.fn((_p: string) => Promise.resolve()),
    onDrop: vi.fn((handler: (paths: string[], target: Element | null) => void) => {
      state.drop = handler;
      return Promise.resolve(() => {});
    }),
    onCloseRequested: vi.fn((handler: (prevent: () => void) => Promise<void>) => {
      state.close = handler;
      return Promise.resolve(() => {});
    }),
  } satisfies Api;
  return { api, state };
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && npm test`
Expected: the new test files fail: `Failed to resolve import "./format"` (and `./rate`, `./api`).

- [ ] **Step 3: Implement**

Create `ui/src/lib/api.ts`:

```ts
// Everything the UI asks of the app, in one place. Components take it from the Svelte
// context (`useApi`), so tests can pass a fake instead of talking to Tauri.

import { Channel } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { getContext, setContext } from "svelte";
import {
  commands,
  type ConflictPolicy,
  type FinishedRow,
  type ProgressView,
  type SessionView,
  type SummaryView,
} from "./bindings";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: string };

/** Turns a command's error result into a thrown `Error` with the app's message. */
export async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

function asList(picked: string | string[] | null): string[] | null {
  if (picked === null) return null;
  return Array.isArray(picked) ? picked : [picked];
}

export const tauriApi = {
  scanSource: (paths: string[], contentsOnly: boolean): Promise<SessionView> =>
    unwrap(commands.scanSource(paths, contentsOnly)),
  clearSource: (): Promise<SessionView> => unwrap(commands.clearSource()),
  setFilter: (selected: (string | null)[] | null): Promise<SessionView> =>
    unwrap(commands.setFilter(selected)),
  setDestination: (path: string | null): Promise<SessionView> =>
    unwrap(commands.setDestination(path)),
  setConflicts: (policy: ConflictPolicy): Promise<SessionView> =>
    unwrap(commands.setConflicts(policy)),
  sessionView: (): Promise<SessionView> => unwrap(commands.sessionView()),

  startJob: (verify: boolean, onProgress: (p: ProgressView) => void): Promise<null> => {
    const channel = new Channel<ProgressView>();
    channel.onmessage = onProgress;
    return unwrap(commands.startJob(verify, channel));
  },
  pauseJob: (): Promise<void> => commands.pauseJob(),
  resumeJob: (): Promise<void> => commands.resumeJob(),
  cancelJob: (): Promise<void> => commands.cancelJob(),
  jobRunning: (): Promise<boolean> => commands.jobRunning(),
  finishedPage: (offset: number, limit: number, failedOnly: boolean): Promise<FinishedRow[]> =>
    unwrap(commands.finishedPage(offset, limit, failedOnly)),
  jobSummary: (): Promise<SummaryView | null> => unwrap(commands.jobSummary()),
  saveReport: (path: string): Promise<null> => unwrap(commands.saveReport(path)),
  retryFailed: (): Promise<SessionView> => unwrap(commands.retryFailed()),

  pickFolder: async (): Promise<string[] | null> =>
    asList(await open({ directory: true, multiple: false, title: "Copy from" })),
  pickFiles: async (): Promise<string[] | null> =>
    asList(await open({ directory: false, multiple: true, title: "Copy these files" })),
  pickDestination: async (): Promise<string | null> =>
    asList(await open({ directory: true, multiple: false, title: "Copy to" }))?.[0] ?? null,
  pickReportPath: (suggested: string): Promise<string | null> =>
    save({ defaultPath: suggested, filters: [{ name: "Text", extensions: ["txt"] }] }),
  confirm: (message: string, title: string): Promise<boolean> =>
    ask(message, { title, kind: "warning", okLabel: "Stop copying", cancelLabel: "Keep copying" }),
  reveal: (path: string): Promise<void> => revealItemInDir(path),
  openFile: (path: string): Promise<void> => openPath(path),

  /** Finder drops: the paths and the element under the pointer. */
  onDrop: (handler: (paths: string[], target: Element | null) => void): Promise<() => void> =>
    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      const { x, y } = event.payload.position.toLogical(window.devicePixelRatio);
      handler(event.payload.paths, document.elementFromPoint(x, y));
    }),
  /** Closing the window; call `prevent()` to keep it open. */
  onCloseRequested: (handler: (prevent: () => void) => Promise<void>): Promise<() => void> =>
    getCurrentWindow().onCloseRequested((event) => handler(() => event.preventDefault())),
};

export type Api = typeof tauriApi;

const KEY = Symbol("api");

export function provideApi(api: Api): void {
  setContext(KEY, api);
}

export function useApi(): Api {
  return getContext<Api>(KEY);
}
```

Create `ui/src/lib/format.ts`:

```ts
// How figures are shown (RFD §5.3): decimal units like Finder, tabular digits in the CSS.

const count = new Intl.NumberFormat("en-US");

/** 1284 → "1,284" */
export function formatCount(n: number): string {
  return count.format(n);
}

/** Decimal units, like Finder: 212400000000 → "212.4 GB". */
export function formatBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return unit === 0 ? `${n} B` : `${value.toFixed(1)} ${units[unit]}`;
}

/** Bytes per second → "1.2 GB/s"; "—" when unknown. */
export function formatSpeed(bytesPerSecond: number | null): string {
  return bytesPerSecond === null ? "—" : `${formatBytes(Math.round(bytesPerSecond))}/s`;
}

/** Milliseconds → "0:07", "4:12", "1:02:03"; "—" when unknown. */
export function formatDuration(ms: number | null): string {
  if (ms === null || !Number.isFinite(ms)) return "—";
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** done/total → "69.8 %"; an empty total counts as complete. */
export function formatPercent(done: number, total: number): string {
  const p = total === 0 ? 100 : (done * 100) / total;
  return `${p.toFixed(1)} %`;
}

/** "1 file", "2 files" */
export function plural(n: number, one: string, many = `${one}s`): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}
```

Create `ui/src/lib/rate.ts`:

```ts
// Speeds and ETAs from successive progress messages, averaged over the last few seconds so
// they don't jump around (RFD §5.3).

const WINDOW_MS = 3000;

type Sample = { t: number; bytes: number };

export class RateMeter {
  private samples: Sample[] = [];
  private first: Sample | null = null;

  /** Records `bytes` done at time `t` (milliseconds since the job started). */
  push(t: number, bytes: number): void {
    const sample = { t, bytes };
    this.first ??= sample;
    this.samples.push(sample);
    while (this.samples.length > 2 && t - this.samples[0].t > WINDOW_MS) {
      this.samples.shift();
    }
  }

  /** Bytes per second over the last few seconds; `null` until there are two samples. */
  current(): number | null {
    const [a, b] = [this.samples[0], this.samples[this.samples.length - 1]];
    if (!a || !b || b.t <= a.t) return null;
    return ((b.bytes - a.bytes) * 1000) / (b.t - a.t);
  }

  /** Bytes per second since the start; `null` until time has passed. */
  average(): number | null {
    const last = this.samples[this.samples.length - 1];
    if (!this.first || !last || last.t <= this.first.t) return null;
    return ((last.bytes - this.first.bytes) * 1000) / (last.t - this.first.t);
  }

  /** Milliseconds left for `remaining` bytes at the current speed; `null` when unknown. */
  eta(remaining: number): number | null {
    if (remaining <= 0) return 0;
    const speed = this.current();
    return speed && speed > 0 ? (remaining * 1000) / speed : null;
  }
}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(ui): add formatting, speed meters and the app api

Formatting like Finder, speeds and ETAs averaged over 3 s, and one Api
object for everything the UI asks of the app, which tests replace with
a fake.

Refs: #15
EOF
```


---

### Task 6: The main window

`Setup.svelte` is the main window of RFD §5.2:
- **FROM:** pickers and Finder drop, the summary line, folder-or-contents (which rescans),
  and the extension chips (`ExtensionChips.svelte`).
- **TO:** picker and drop, free space and file system, "Files will go to".
- **Pre-flight** (`PreflightPanel.svelte`): blockers, files that will fail, the non-empty
  warning, identical files, the Keep both / Overwrite / Skip choice, leftover partial files.
- **Mode**, and **Start**, which says what it will do.

A drop is routed by the element under the pointer (`[data-drop="from"|"to"]`). A command
error is shown where it happened.

**Files:**
- Create: `ui/src/components/ExtensionChips.svelte`, `ui/src/components/PreflightPanel.svelte`, `ui/src/components/Setup.svelte`, `ui/src/components/Setup.test.ts`
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes: Task 5.
- Produces: `Setup.svelte` with props `view` (bindable `SessionView`), `verify` (bindable
  boolean), `onStart()`; `ExtensionChips.svelte` (`extensions`, `selected`, `onChange`);
  `PreflightPanel.svelte` (`destination`, `plan`, `conflicts`, `onConflicts`).

- [ ] **Step 1: Write the failing tests**

Create `ui/src/components/Setup.test.ts`:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { SessionView } from "../lib/bindings";
import { destinationView, fakeApi, readyView, sessionView, sourceView } from "../test/fake-api";
import Setup from "./Setup.svelte";

function setup(view: SessionView = sessionView(), answer: SessionView = view) {
  const { api, state } = fakeApi(answer);
  const started: number[] = [];
  const result = render(Setup, {
    props: { view, verify: true, onStart: () => started.push(1) },
    context: apiContext(api),
  });
  return { api, state, started, ...result };
}

const start = () => screen.getByRole("button", { name: /copy & verify|start copy/i });

describe("Setup", () => {
  test("Start stays disabled until there is something to copy", () => {
    setup();
    expect(start()).toHaveProperty("disabled", true);
  });

  test("a ready session enables Start and says what it will do", async () => {
    const { started } = setup(readyView());
    const button = screen.getByRole("button", { name: "Copy & verify 1,284 files · 212.4 GB" });
    expect(button).toHaveProperty("disabled", false);
    await fireEvent.click(button);
    expect(started).toHaveLength(1);
  });

  test("Choose folder scans the picked folder", async () => {
    const { api } = setup(sessionView(), readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalledWith(["/Volumes/CARD/DCIM"], false));
    await screen.findByText("1,284 files · 212.4 GB · 37 hidden items skipped");
  });

  test("switching to 'only what's inside' rescans with contents only", async () => {
    const { api } = setup(sessionView(), readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    await fireEvent.click(await screen.findByLabelText("Copy only what's inside"));
    await waitFor(() => expect(api.scanSource).toHaveBeenLastCalledWith(["/Volumes/CARD/DCIM"], true));
  });

  test("a chip turned off narrows the filter; All selects everything again", async () => {
    const { api } = setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: /\.xml/ }));
    expect(api.setFilter).toHaveBeenLastCalledWith(["mov", "wav"]);
    await fireEvent.click(screen.getByRole("button", { name: "All" }));
    expect(api.setFilter).toHaveBeenLastCalledWith(null);
  });

  test("a Finder drop on FROM scans it, a drop on TO sets the destination", async () => {
    const { api, state, container } = setup(readyView());
    await waitFor(() => expect(state.drop).not.toBeNull());
    state.drop!(["/Volumes/CARD2"], container.querySelector('[data-drop="from"] p'));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalledWith(["/Volumes/CARD2"], false));
    state.drop!(["/Volumes/Backup"], container.querySelector('[data-drop="to"]'));
    await waitFor(() => expect(api.setDestination).toHaveBeenCalledWith("/Volumes/Backup"));
  });

  test("a blocker is shown and Start stays disabled", () => {
    setup(
      readyView({
        destination: destinationView({ blocker: "The destination is the source folder or inside it" }),
        plan: null,
      }),
    );
    expect(screen.getByRole("alert").textContent).toContain("inside it");
    expect(start()).toHaveProperty("disabled", true);
  });

  test("not enough space blocks Start", () => {
    setup(
      readyView({
        plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, blocker: "Not enough free space" },
      }),
    );
    expect(screen.getByRole("alert").textContent).toBe("Not enough free space");
    expect(screen.getByRole("button", { name: /copy & verify/i })).toHaveProperty("disabled", true);
  });

  test("a non-empty copy root is a warning, not a block", () => {
    setup(readyView({ destination: destinationView({ existingItems: 1204 }) }));
    screen.getByText(/already contains 1,204 items/);
    expect(start()).toHaveProperty("disabled", false);
  });

  test("files that will fail are listed with their reasons", () => {
    setup(
      readyView({
        destination: destinationView({
          problems: [{ path: "DCIM/a:b.mov", reason: "the name contains \":\", which this drive doesn't allow" }],
          problemCount: 3,
        }),
      }),
    );
    screen.getByText("3 files will fail");
    screen.getByText("DCIM/a:b.mov");
    screen.getByText("and 2 more");
  });

  test("identical files, different files and leftovers are explained", async () => {
    const { api } = setup(
      readyView({ destination: destinationView({ identical: 284, differs: 12, stalePartials: 2 }) }),
    );
    screen.getByText("284 identical files will be skipped (not checked).");
    screen.getByText("2 unfinished files from an interrupted copy will be replaced.");
    expect(screen.getByLabelText("Keep both")).toHaveProperty("checked", true);
    await fireEvent.click(screen.getByLabelText("Overwrite"));
    expect(api.setConflicts).toHaveBeenCalledWith("overwrite");
  });

  test("Copy mode changes the Start label", async () => {
    setup(readyView());
    await fireEvent.click(screen.getByLabelText("Copy"));
    screen.getByRole("button", { name: "Copy 1,284 files · 212.4 GB" });
  });

  test("a command error is shown where it happened", async () => {
    const { api } = setup(sessionView({ source: sourceView() }));
    api.setDestination.mockRejectedValueOnce(new Error("Can't write to the destination"));
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText("Can't write to the destination");
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && npm test`
Expected: `Setup.test.ts` fails: `Failed to resolve import "./Setup.svelte"`.

- [ ] **Step 3: Implement**

Create `ui/src/components/ExtensionChips.svelte`:

```svelte
<script lang="ts">
  // The file-type filter (FR-7..FR-9): one chip per extension, largest first, with All / None.
  import type { ExtensionView } from "../lib/bindings";
  import { formatBytes, formatCount } from "../lib/format";

  let {
    extensions,
    selected,
    onChange,
  }: {
    extensions: ExtensionView[];
    /** `null` = every extension. */
    selected: (string | null)[] | null;
    /** `null` when every extension ends up selected. */
    onChange: (selected: (string | null)[] | null) => void;
  } = $props();

  const isOn = (key: string | null) => selected === null || selected.includes(key);

  function toggle(key: string | null) {
    const current = selected ?? extensions.map((e) => e.key);
    const next = current.includes(key) ? current.filter((k) => k !== key) : [...current, key];
    onChange(next.length === extensions.length ? null : next);
  }
</script>

<div class="chips" role="group" aria-label="File types">
  {#each extensions as ext (ext.key)}
    <button
      type="button"
      class="chip"
      class:on={isOn(ext.key)}
      aria-pressed={isOn(ext.key)}
      onclick={() => toggle(ext.key)}
    >
      {ext.label} <span class="muted">{formatCount(ext.files)} · {formatBytes(ext.bytes)}</span>
    </button>
  {/each}
  <button type="button" class="link" onclick={() => onChange(null)}>All</button>
  <button type="button" class="link" onclick={() => onChange([])}>None</button>
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    padding: 3px 10px;
    border-radius: 999px;
    color: var(--text-muted);
  }

  .chip.on {
    color: var(--text);
    border-color: var(--accent);
  }

  .muted {
    color: var(--text-muted);
  }

  .link {
    background: none;
    border: none;
    color: var(--accent);
    padding: 3px 6px;
  }
</style>
```

Create `ui/src/components/PreflightPanel.svelte`:

```svelte
<script lang="ts">
  // What pre-flight found (FR-16..FR-18, RFD §5.2): what blocks Start, what will fail, what's
  // already there, and the choice for files that differ.
  import type { ConflictPolicy, DestinationView, PlanView } from "../lib/bindings";
  import { formatCount, plural } from "../lib/format";

  let {
    destination,
    plan,
    conflicts,
    onConflicts,
  }: {
    destination: DestinationView;
    plan: PlanView | null;
    conflicts: ConflictPolicy;
    onConflicts: (policy: ConflictPolicy) => void;
  } = $props();

  const blocker = $derived(destination.blocker ?? plan?.blocker ?? null);
  const choices: { value: ConflictPolicy; label: string }[] = [
    { value: "keepBoth", label: "Keep both" },
    { value: "overwrite", label: "Overwrite" },
    { value: "skip", label: "Skip" },
  ];
</script>

<div class="preflight" aria-live="polite">
  {#if blocker}
    <p class="danger" role="alert">{blocker}</p>
  {:else}
    {#if destination.existingItems}
      <p class="warning">
        <span class="mono">{destination.copyRoot}</span> already contains
        {plural(destination.existingItems, "item")}. Identical files will be skipped.
      </p>
    {/if}
    {#if destination.problemCount > 0}
      <details class="danger-box">
        <summary>{plural(destination.problemCount, "file")} will fail</summary>
        <ul>
          {#each destination.problems as p (p.path)}
            <li><span class="mono">{p.path}</span>: {p.reason}</li>
          {/each}
          {#if destination.problemCount > destination.problems.length}
            <li class="muted">
              and {formatCount(destination.problemCount - destination.problems.length)} more
            </li>
          {/if}
        </ul>
      </details>
    {/if}
    {#if destination.identical > 0}
      <p>
        {plural(destination.identical, "identical file")} will be skipped (not checked).
      </p>
    {/if}
    {#if destination.differs > 0}
      <fieldset>
        <legend>
          {plural(destination.differs, "file")}
          {destination.differs === 1 ? "differs" : "differ"} from what's there
        </legend>
        {#each choices as c (c.value)}
          <label>
            <input
              type="radio"
              name="conflicts"
              value={c.value}
              checked={conflicts === c.value}
              onchange={() => onConflicts(c.value)}
            />
            {c.label}
          </label>
        {/each}
      </fieldset>
    {/if}
    {#if destination.stalePartials > 0}
      <p class="muted">
        {plural(destination.stalePartials, "unfinished file")} from an interrupted copy will be replaced.
      </p>
    {/if}
  {/if}
</div>

<style>
  .preflight p {
    margin: 6px 0;
  }

  .danger {
    color: var(--danger);
  }

  .warning {
    color: var(--warning);
  }

  .muted {
    color: var(--text-muted);
  }

  .danger-box summary {
    color: var(--danger);
    cursor: pointer;
  }

  fieldset {
    border: none;
    padding: 0;
    margin: 6px 0;
    display: flex;
    gap: 16px;
    align-items: center;
  }

  legend {
    float: left;
    margin-right: 8px;
  }
</style>
```

Create `ui/src/components/Setup.svelte`:

```svelte
<script lang="ts">
  // The main window (RFD §5.2): FROM, TO, what pre-flight found, mode, Start.
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type { ConflictPolicy, SessionView } from "../lib/bindings";
  import { formatBytes, formatCount, plural } from "../lib/format";
  import ExtensionChips from "./ExtensionChips.svelte";
  import PreflightPanel from "./PreflightPanel.svelte";

  let {
    view = $bindable(),
    verify = $bindable(true),
    onStart,
  }: {
    view: SessionView;
    verify: boolean;
    onStart: () => void;
  } = $props();

  const api = useApi();

  /** What was picked last, to rescan when "folder itself / contents" changes. */
  let sourcePaths: string[] = $state([]);
  let busy = $state(false);
  let sourceError: string | null = $state(null);
  let destError: string | null = $state(null);

  const source = $derived(view.source);
  /** "1,284 files · 212.4 GB · 37 hidden items skipped" (FR-3, FR-13, FR-24) */
  const sourceSummary = $derived.by(() => {
    if (!source) return "";
    const parts = [plural(source.files, "file"), formatBytes(source.bytes)];
    if (source.skippedHidden > 0) parts.push(`${formatCount(source.skippedHidden)} hidden items skipped`);
    if (source.skippedSymlinks > 0) parts.push(`${plural(source.skippedSymlinks, "symlink")} skipped`);
    return parts.join(" · ");
  });
  const destination = $derived(view.destination);
  const canStart = $derived(
    !busy &&
      !!view.plan &&
      !view.plan.blocker &&
      !destination?.blocker &&
      view.plan.filesToWrite > 0,
  );

  /** Runs a session command; a newer scan's result replaces this one (`stale`). */
  async function update(
    call: () => Promise<SessionView>,
    setError: (e: string | null) => void,
  ): Promise<void> {
    busy = true;
    try {
      const next = await call();
      if (!next.stale) view = next;
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      busy = false;
    }
  }

  function scan(paths: string[], contentsOnly = false) {
    sourcePaths = paths;
    return update(() => api.scanSource(paths, contentsOnly), (e) => (sourceError = e));
  }

  async function chooseFolder() {
    const paths = await api.pickFolder();
    if (paths) await scan(paths);
  }

  async function chooseFiles() {
    const paths = await api.pickFiles();
    if (paths) await scan(paths);
  }

  function setDestination(path: string) {
    return update(() => api.setDestination(path), (e) => (destError = e));
  }

  async function chooseDestination() {
    const path = await api.pickDestination();
    if (path) await setDestination(path);
  }

  function setFilter(selected: (string | null)[] | null) {
    return update(() => api.setFilter(selected), (e) => (sourceError = e));
  }

  function setConflicts(policy: ConflictPolicy) {
    return update(() => api.setConflicts(policy), (e) => (destError = e));
  }

  onMount(() => {
    // Finder drops land on FROM or TO, whichever is under the pointer (FR-1, FR-2, FR-15).
    const unlisten = api.onDrop((paths, target) => {
      const zone = target?.closest("[data-drop]")?.getAttribute("data-drop");
      if (zone === "from") scan(paths);
      else if (zone === "to" && paths.length > 0) setDestination(paths[0]);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<section class="card" data-drop="from" aria-labelledby="from-title">
  <h2 id="from-title">From</h2>
  {#if source}
    <p class="path mono">{source.label}</p>
    <p class="muted">{sourceSummary}</p>
    {#if source.problemCount > 0}
      <details class="warning">
        <summary>{plural(source.problemCount, "item")} couldn't be read</summary>
        <ul>
          {#each source.problems as p (p)}<li class="mono">{p}</li>{/each}
        </ul>
      </details>
    {/if}
  {:else}
    <p class="muted">Drop a folder or files here, or choose them.</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={chooseFolder}>Choose folder…</button>
    <button type="button" onclick={chooseFiles}>Choose files…</button>
  </div>
  {#if sourceError}<p class="danger" role="alert">{sourceError}</p>{/if}

  {#if source?.isFolder}
    <fieldset class="mode">
      <legend class="sr-only">What to copy</legend>
      <label>
        <input
          type="radio"
          name="contents"
          checked={!source.contentsOnly}
          onchange={() => scan(sourcePaths, false)}
        />
        Copy the folder “{source.rootDir ?? source.label}” itself
      </label>
      <label>
        <input
          type="radio"
          name="contents"
          checked={source.contentsOnly}
          onchange={() => scan(sourcePaths, true)}
        />
        Copy only what's inside
      </label>
    </fieldset>
    {#if source.extensions.length > 0}
      <ExtensionChips
        extensions={source.extensions}
        selected={source.selectedExtensions}
        onChange={setFilter}
      />
    {/if}
  {/if}
</section>

<section class="card" data-drop="to" aria-labelledby="to-title">
  <h2 id="to-title">To</h2>
  {#if destination}
    <p class="path mono">{destination.path}</p>
    {#if !destination.blocker}
      <p class="muted">{formatBytes(destination.freeBytes)} free · {destination.fsKind}</p>
    {/if}
  {:else}
    <p class="muted">Drop the destination folder here, or choose it.</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={chooseDestination}>Choose…</button>
  </div>
  {#if destError}<p class="danger" role="alert">{destError}</p>{/if}
  {#if destination && source}
    <p>Files will go to: <span class="mono">{destination.copyRoot}</span></p>
    <PreflightPanel
      {destination}
      plan={view.plan}
      conflicts={view.conflicts}
      onConflicts={setConflicts}
    />
  {:else if destination?.blocker}
    <p class="danger" role="alert">{destination.blocker}</p>
  {/if}
</section>

<footer class="start">
  <div class="segmented" role="radiogroup" aria-label="Mode">
    <label class:on={!verify}>
      <input type="radio" name="mode" checked={!verify} onchange={() => (verify = false)} />
      Copy
    </label>
    <label class:on={verify}>
      <input type="radio" name="mode" checked={verify} onchange={() => (verify = true)} />
      Copy & Verify
    </label>
  </div>
  <button type="button" class="primary" disabled={!canStart} onclick={onStart}>
    {#if view.plan && view.plan.filesToWrite > 0}
      {verify ? "Copy & verify" : "Copy"}
      {plural(view.plan.filesToWrite, "file")} · {formatBytes(view.plan.bytesToWrite)}
    {:else}
      Start copy
    {/if}
  </button>
</footer>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  h2 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  p {
    margin: 4px 0;
  }

  .path {
    word-break: break-all;
  }

  .muted {
    color: var(--text-muted);
  }

  .danger {
    color: var(--danger);
  }

  .warning {
    color: var(--warning);
  }

  .actions {
    display: flex;
    gap: 8px;
    margin: 8px 0;
  }

  fieldset.mode {
    border: none;
    padding: 0;
    margin: 8px 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .start {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .segmented {
    display: flex;
    gap: 4px;
  }

  .segmented label {
    position: relative;
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .segmented label.on {
    border-color: var(--accent);
  }

  .segmented input {
    position: absolute;
    opacity: 0;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
```

Change `ui/src/lib/api.ts`:

```diff
diff --git a/ui/src/lib/api.ts b/ui/src/lib/api.ts
index d538a8b..5b92665 100644
--- a/ui/src/lib/api.ts
+++ b/ui/src/lib/api.ts
@@ -93,3 +93,8 @@ export function provideApi(api: Api): void {
 export function useApi(): Api {
   return getContext<Api>(KEY);
 }
+
+/** The context to render a component with a given `Api` (tests). */
+export function apiContext(api: Api): Map<symbol, Api> {
+  return new Map([[KEY, api]]);
+}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(ui): add the main window

FROM and TO with pickers and Finder drop, folder-or-contents, the
extension chips, "Files will go to", the non-empty warning, pre-flight
messages and the conflict choice, the mode, and a Start button that
says what it will do (RFD §5.2).

Refs: #15
EOF
```


---

### Task 7: The progress view

`JobProgress.svelte` is the view of RFD §5.3:
- the phase ("Copying & verifying", "Verifying", "Paused", "Done") and elapsed time
- a `ProgressBar` per phase: bytes, percent, current and average speed, ETA. The fill
  animates for 0.5 s, the gap between updates.
- files done, skipped and failed
- active files, with small files grouped
- Pause / Resume, and Cancel after a confirmation

`FinishedList.svelte` is virtualized. Only the visible rows (plus 5 each side) exist, and
rows are fetched in pages of 100 as they come into view. Incomplete pages are fetched again
as more files finish, and "Failed only" switches the source.

**Files:**
- Create: `ui/src/components/FinishedList.svelte`, `ui/src/components/JobProgress.svelte`, `ui/src/components/JobProgress.test.ts`, `ui/src/components/ProgressBar.svelte`

**Interfaces:**
- Consumes: Task 5.
- Produces: `JobProgress.svelte` (`progress: ProgressView`); `ProgressBar.svelte` (`label,
  done, total, speed, average, eta`); `FinishedList.svelte` (`total, failedTotal`).

- [ ] **Step 1: Write the failing tests**

Create `ui/src/components/JobProgress.test.ts`:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { FinishedRow, ProgressView } from "../lib/bindings";
import { fakeApi, progressView } from "../test/fake-api";
import JobProgress from "./JobProgress.svelte";

function show(progress: ProgressView) {
  const { api } = fakeApi();
  const result = render(JobProgress, { props: { progress }, context: apiContext(api) });
  return { api, ...result };
}

function row(i: number, over: Partial<FinishedRow> = {}): FinishedRow {
  return {
    id: i,
    path: `DCIM/C${i}.mov`,
    finalPath: `DCIM/C${i}.mov`,
    size: 8_100_000_000,
    millis: 6_900,
    hash: "9f3a07c1d4e2c21e",
    status: "verified",
    reason: null,
    ...over,
  };
}

describe("JobProgress", () => {
  test("phase, bars and file counts", () => {
    show(progressView({ copiedBytes: 148_200_000_000, verifiedBytes: 141_000_000_000, filesDone: 902 }));
    screen.getByRole("heading", { name: "Copying & verifying" });
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    screen.getByText(/148\.2 GB \/ 212\.4 GB · 69\.8 %/);
    screen.getByText("902 / 1,284 files");
  });

  test("speed and ETA need two updates, then follow the progress", async () => {
    const { rerender } = show(progressView());
    expect(screen.getAllByText(/ETA —/)).toHaveLength(2);
    await rerender({ progress: progressView({ elapsedMs: 1000, copiedBytes: 1_000_000_000 }) });
    screen.getByText(/1\.0 GB\/s \(avg 1\.0 GB\/s\) · ETA 3:31/);
  });

  test("plain Copy shows one bar", () => {
    show(progressView({ verify: false }));
    screen.getByRole("heading", { name: "Copying" });
    expect(screen.getAllByRole("progressbar")).toHaveLength(1);
  });

  test("pause and resume", async () => {
    const { api, rerender } = show(progressView());
    await fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(api.pauseJob).toHaveBeenCalled();
    await rerender({ progress: progressView({ paused: true }) });
    screen.getByRole("heading", { name: "Paused" });
    await fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(api.resumeJob).toHaveBeenCalled();
  });

  test("cancel asks first and only stops when confirmed", async () => {
    const { api } = show(progressView());
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalledTimes(1));
    expect(api.cancelJob).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(api.cancelJob).toHaveBeenCalled());
    expect(api.confirm.mock.calls[0][0]).toContain("the file in progress is removed");
  });

  test("big files get a row, small ones are grouped", () => {
    show(
      progressView({
        active: [
          { id: 1, name: "A001C014.mov", path: "DCIM/A001C014.mov", verifying: false, size: 8_400_000_000, bytesDone: 5_100_000_000 },
        ],
        smallFiles: { count: 12, size: 41_000_000, bytesDone: 18_000_000 },
      }),
    );
    screen.getByText("A001C014.mov");
    screen.getByText("+ 12 small files");
  });

  test("a fatal error shows a banner", () => {
    show(progressView({ phase: "done", fatal: "The source is no longer available; was it disconnected?" }));
    expect(screen.getByRole("alert").textContent).toContain("Stopped: The source is no longer available");
  });

  test("the finished list only renders the visible rows and fetches their page", async () => {
    const { api, container } = show(progressView({ filesDone: 10_000 }));
    api.finishedPage.mockImplementation((offset: number, limit: number) =>
      Promise.resolve(Array.from({ length: limit }, (_, i) => row(offset + i))),
    );
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(0, 100, false));
    expect(screen.getAllByRole("listitem").length).toBeLessThan(25);
    const viewport = container.querySelector(".viewport") as HTMLElement;
    viewport.scrollTop = 5000 * 28;
    await fireEvent.scroll(viewport);
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(4900, 100, false));
  });

  test("Failed only asks for failed rows", async () => {
    const { api } = show(progressView({ filesDone: 10, filesFailed: 2 }));
    await fireEvent.click(screen.getByLabelText("Failed only"));
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(0, 100, true));
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && npm test`
Expected: `JobProgress.test.ts` fails: `Failed to resolve import "./JobProgress.svelte"`.

- [ ] **Step 3: Implement**

Create `ui/src/components/FinishedList.svelte`:

```svelte
<script lang="ts">
  // Finished files (RFD §5.3). Virtualized: only the visible rows exist, and rows are fetched
  // from the app a page at a time, so it stays smooth with a million files.
  import { useApi } from "../lib/api";
  import type { FinishedRow } from "../lib/bindings";
  import { formatBytes, formatDuration, formatSpeed } from "../lib/format";

  let {
    total,
    failedTotal,
  }: {
    /** Finished files so far. */
    total: number;
    /** Failed files so far. */
    failedTotal: number;
  } = $props();

  const api = useApi();
  const ROW = 28;
  const HEIGHT = 280;
  const PAGE = 100;
  const OVERSCAN = 5;

  let failedOnly = $state(false);
  let scrollTop = $state(0);
  /** Fetched pages by page number; a page is refetched until it is full. */
  let pages: Map<number, FinishedRow[]> = $state(new Map());

  const count = $derived(failedOnly ? failedTotal : total);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(count, Math.ceil((scrollTop + HEIGHT) / ROW) + OVERSCAN));
  const visible = $derived(
    Array.from({ length: Math.max(0, last - first) }, (_, i) => {
      const index = first + i;
      return { index, row: pages.get(Math.floor(index / PAGE))?.[index % PAGE] };
    }),
  );

  $effect(() => {
    // Fetch the pages the visible rows need; incomplete pages again as more files finish.
    const only = failedOnly;
    const wanted = new Set(visible.map((v) => Math.floor(v.index / PAGE)));
    for (const page of wanted) {
      const have = pages.get(page);
      const expected = Math.min(PAGE, count - page * PAGE);
      if (have && have.length >= expected) continue;
      api.finishedPage(page * PAGE, PAGE, only).then((rows) => {
        if (only !== failedOnly) return;
        pages = new Map(pages).set(page, rows);
      });
    }
  });

  function showFailedOnly(on: boolean) {
    failedOnly = on;
    pages = new Map();
    scrollTop = 0;
  }

  const statusText = (r: FinishedRow) =>
    ({ verified: "✓ Verified", copied: "✓ Copied", skipped: "Skipped", failed: "✗ Failed" })[r.status];
</script>

<div class="head">
  <h3>Finished</h3>
  <label>
    <input
      type="checkbox"
      checked={failedOnly}
      onchange={(e) => showFailedOnly(e.currentTarget.checked)}
    />
    Failed only
  </label>
</div>
<div
  class="viewport"
  style:height="{HEIGHT}px"
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  role="list"
  aria-label="Finished files"
>
  <div style:height="{count * ROW}px" class="spacer">
    {#each visible as { index, row } (index)}
      <div class="row" role="listitem" style:top="{index * ROW}px">
        {#if row}
          <span class="name" title={row.finalPath}>{row.finalPath}</span>
          <span>{formatBytes(row.size)}</span>
          <span>{formatDuration(row.millis)}</span>
          <span>{formatSpeed(row.millis > 0 ? (row.size * 1000) / row.millis : null)}</span>
          <span class="mono">{row.hash ?? "—"}</span>
          <span class={row.status} title={row.reason ?? ""}>
            {statusText(row)}{row.reason ? ` — ${row.reason}` : ""}
          </span>
        {:else}
          <span class="muted">…</span>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  h3 {
    margin: 12px 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .viewport {
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 28px;
    display: grid;
    grid-template-columns: 1fr 80px 60px 90px 150px 120px;
    gap: 8px;
    align-items: center;
    padding: 0 10px;
    font-size: 12px;
    white-space: nowrap;
  }

  .row span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .verified,
  .copied {
    color: var(--success);
  }

  .failed {
    color: var(--danger);
  }

  .muted,
  .skipped {
    color: var(--text-muted);
  }
</style>
```

Create `ui/src/components/JobProgress.svelte`:

```svelte
<script lang="ts">
  // The progress view (RFD §5.3): phase and time, one bar per phase, active files, finished
  // files, Pause / Resume and Cancel.
  import { useApi } from "../lib/api";
  import type { ProgressView } from "../lib/bindings";
  import { formatBytes, formatCount, formatDuration, formatPercent, plural } from "../lib/format";
  import { RateMeter } from "../lib/rate";
  import FinishedList from "./FinishedList.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  let { progress }: { progress: ProgressView } = $props();

  const api = useApi();
  // The meters aren't reactive; each update recomputes the figures from them.
  const copyMeter = new RateMeter();
  const verifyMeter = new RateMeter();
  type Figures = { speed: number | null; average: number | null; eta: number | null };
  const unknown: Figures = { speed: null, average: null, eta: null };
  let copy: Figures = $state(unknown);
  let verify: Figures = $state(unknown);

  $effect(() => {
    copyMeter.push(progress.elapsedMs, progress.copiedBytes);
    verifyMeter.push(progress.elapsedMs, progress.verifiedBytes);
    const left = (done: number) => progress.totalBytes - done;
    copy = {
      speed: copyMeter.current(),
      average: copyMeter.average(),
      eta: copyMeter.eta(left(progress.copiedBytes)),
    };
    verify = {
      speed: verifyMeter.current(),
      average: verifyMeter.average(),
      eta: verifyMeter.eta(left(progress.verifiedBytes)),
    };
  });

  const phase = $derived(
    progress.phase === "done"
      ? "Done"
      : progress.paused
        ? "Paused"
        : progress.phase === "verifying"
          ? "Verifying"
          : progress.verify
            ? "Copying & verifying"
            : "Copying",
  );
  const files = $derived.by(() => {
    const parts = [`${formatCount(progress.filesDone)} / ${plural(progress.totalFiles, "file")}`];
    if (progress.filesSkipped > 0) parts.push(`${formatCount(progress.filesSkipped)} skipped`);
    if (progress.filesFailed > 0) parts.push(`${formatCount(progress.filesFailed)} failed`);
    return parts.join(" · ");
  });

  async function cancel() {
    const stop = await api.confirm(
      "Files already copied stay and are listed in the checksum file; the file in progress is removed.",
      "Stop copying?",
    );
    if (stop) await api.cancelJob();
  }
</script>

<section class="card" aria-labelledby="phase">
  <header>
    <h2 id="phase">{phase}</h2>
    <span class="muted">{formatDuration(progress.elapsedMs)} elapsed</span>
  </header>

  {#if progress.fatal}
    <p class="banner" role="alert">Stopped: {progress.fatal}</p>
  {/if}

  <ProgressBar label="Copied" done={progress.copiedBytes} total={progress.totalBytes} {...copy} />
  {#if progress.verify}
    <ProgressBar label="Verified" done={progress.verifiedBytes} total={progress.totalBytes} {...verify} />
  {/if}
  <p class="muted files">{files}</p>

  <div class="controls">
    {#if progress.paused}
      <button type="button" onclick={() => api.resumeJob()}>Resume</button>
    {:else}
      <button type="button" onclick={() => api.pauseJob()} disabled={progress.phase === "done"}>Pause</button>
    {/if}
    <button type="button" onclick={cancel} disabled={progress.phase === "done"}>Cancel</button>
  </div>
</section>

<section class="card" aria-labelledby="active-title">
  <h3 id="active-title">Active</h3>
  {#if progress.active.length === 0 && !progress.smallFiles}
    <p class="muted">—</p>
  {/if}
  <table>
    <tbody>
      {#each progress.active as f (f.id)}
        <tr>
          <td class="name" title={f.path}>{f.name}</td>
          <td>{f.verifying ? "Verifying" : "Copying"}</td>
          <td>{formatBytes(f.size)}</td>
          <td>{formatBytes(f.bytesDone)}</td>
          <td>{formatPercent(f.bytesDone, f.size)}</td>
        </tr>
      {/each}
      {#if progress.smallFiles}
        <tr>
          <td class="name">+ {plural(progress.smallFiles.count, "small file")}</td>
          <td></td>
          <td>{formatBytes(progress.smallFiles.size)}</td>
          <td>{formatBytes(progress.smallFiles.bytesDone)}</td>
          <td>{formatPercent(progress.smallFiles.bytesDone, progress.smallFiles.size)}</td>
        </tr>
      {/if}
    </tbody>
  </table>
  <FinishedList total={progress.filesDone} failedTotal={progress.filesFailed} />
</section>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
  }

  .banner {
    color: var(--danger);
    border: 1px solid var(--danger);
    border-radius: var(--radius);
    padding: 8px 12px;
  }

  .files {
    margin: 4px 0 0 82px;
  }

  .controls {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  td {
    padding: 3px 6px;
    white-space: nowrap;
  }

  td.name {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
```

Create `ui/src/components/ProgressBar.svelte`:

```svelte
<script lang="ts">
  // One phase's bar (RFD §5.3): bytes, percent, current and average speed, ETA. The fill
  // animates for as long as the gap between updates, so two updates a second look smooth.
  import { formatBytes, formatDuration, formatPercent, formatSpeed } from "../lib/format";

  let {
    label,
    done,
    total,
    speed,
    average,
    eta,
  }: {
    label: string;
    done: number;
    total: number;
    speed: number | null;
    average: number | null;
    eta: number | null;
  } = $props();

  const fraction = $derived(total === 0 ? 1 : Math.min(1, done / total));
</script>

<div class="bar-row">
  <span class="label">{label}</span>
  <div
    class="track"
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(fraction * 100)}
  >
    <div class="fill" style:width="{fraction * 100}%"></div>
  </div>
  <span class="figures">
    {formatBytes(done)} / {formatBytes(total)} · {formatPercent(done, total)} ·
    {formatSpeed(speed)} (avg {formatSpeed(average)}) · ETA {formatDuration(eta)}
  </span>
</div>

<style>
  .bar-row {
    display: grid;
    grid-template-columns: 70px 1fr;
    gap: 4px 12px;
    align-items: center;
    margin: 8px 0;
  }

  .label {
    color: var(--text-muted);
  }

  .track {
    height: 8px;
    border-radius: 4px;
    background: var(--surface-raised);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.5s linear;
  }

  .figures {
    grid-column: 2;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(ui): add the progress view

Phase and time, a bar per phase with speed and ETA, active files with
small files grouped, a virtualized finished list fetched in pages, and
Pause / Resume / Cancel (RFD §5.3).

Refs: #15
EOF
```


---

### Task 8: The summary, and the screens connected

`Summary.svelte` is the view of RFD §5.4:
- the status line (`headline.ts`)
- the figures, including files already at the destination "not checked"
- the failures with their reasons
- Reveal in Finder, Open checksum file, Save report…, Retry failed and New copy

`App.svelte` moves between the three screens:
- **Start** switches to progress. If the job can't start, it goes back to setup with the
  reason.
- **The `done` message** fetches the summary.
- **Retry failed** returns to setup with only the failed files. **New copy** clears the
  source and keeps the destination for this session.
- **Closing the window during a copy** asks first. If it closes anyway, Task 4's exit
  handler stops the copy cleanly.

**Files:**
- Create: `ui/src/components/Summary.svelte`, `ui/src/components/Summary.test.ts`, `ui/src/lib/headline.test.ts`, `ui/src/lib/headline.ts`
- Modify: `ui/src/App.svelte`, `ui/src/App.test.ts`

**Interfaces:**
- Consumes: Tasks 4–7.
- Produces: `Summary.svelte` (`summary, onRetry, onNewCopy`); `headline(SummaryView)`;
  `App.svelte` (optional prop `api`, default `tauriApi`).

- [ ] **Step 1: Write the failing tests**

Replace `ui/src/App.test.ts` with:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import App from "./App.svelte";
import { fakeApi, progressView, readyView, sessionView, summaryView } from "./test/fake-api";

function app(view = readyView()) {
  const { api, state } = fakeApi(view);
  render(App, { props: { api } });
  return { api, state };
}

const startButton = () => screen.findByRole("button", { name: /^Copy & verify 1,284 files/ });

describe("App", () => {
  test("shows the app name", () => {
    app(sessionView());
    expect(screen.getByRole("heading", { name: "Secopy" })).toBeTruthy();
  });

  test("a job goes from setup to progress to the summary", async () => {
    const { api, state } = app();
    await fireEvent.click(await startButton());
    expect(api.startJob).toHaveBeenCalledWith(true, expect.any(Function));
    await screen.findByRole("heading", { name: "Copying & verifying" });
    state.progress!(progressView({ copiedBytes: 1000, elapsedMs: 500 }));
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText(/All 1,284 files copied and verified/);
  });

  test("a job that can't start goes back to setup with the reason", async () => {
    const { api } = app();
    api.startJob.mockRejectedValueOnce(new Error("Nothing to copy, or something blocks the copy."));
    await fireEvent.click(await startButton());
    await screen.findByText("Nothing to copy, or something blocks the copy.");
    await startButton();
  });

  test("Retry failed goes back to setup with the failed files", async () => {
    const { api, state } = app();
    api.jobSummary.mockResolvedValue(summaryView({ outcome: "failures", failed: 2, verified: 1282 }));
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Retry failed" }));
    await waitFor(() => expect(api.retryFailed).toHaveBeenCalled());
    await startButton();
  });

  test("New copy clears the source and keeps the destination", async () => {
    const { api, state } = app();
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await fireEvent.click(await screen.findByRole("button", { name: "New copy" }));
    await waitFor(() => expect(api.clearSource).toHaveBeenCalled());
  });

  test("closing during a copy asks, and stays open unless confirmed", async () => {
    const { api, state } = app();
    await waitFor(() => expect(state.close).not.toBeNull());
    api.jobRunning.mockResolvedValue(true);
    api.confirm.mockResolvedValueOnce(false);
    let prevented = false;
    await state.close!(() => (prevented = true));
    expect(prevented).toBe(true);
    api.confirm.mockResolvedValueOnce(true);
    prevented = false;
    await state.close!(() => (prevented = true));
    expect(prevented).toBe(false);
  });

  test("closing when idle doesn't ask", async () => {
    const { api, state } = app();
    await waitFor(() => expect(state.close).not.toBeNull());
    await state.close!(() => {});
    expect(api.confirm).not.toHaveBeenCalled();
  });
});
```

Create `ui/src/components/Summary.test.ts`:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { SummaryView } from "../lib/bindings";
import { fakeApi, summaryView } from "../test/fake-api";
import Summary from "./Summary.svelte";

function show(summary: SummaryView) {
  const { api } = fakeApi();
  const calls: string[] = [];
  render(Summary, {
    props: { summary, onRetry: () => calls.push("retry"), onNewCopy: () => calls.push("new") },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("Summary", () => {
  test("a complete job: status, figures and no Retry", () => {
    show(summaryView({ skippedIdentical: 284 }));
    expect(screen.getByRole("status").textContent).toContain("All 1,284 files copied and verified");
    screen.getByText("212.4 GB written");
    screen.getByText("4:12");
    screen.getByText("284 already at the destination, not checked");
    expect(screen.queryByRole("button", { name: "Retry failed" })).toBeNull();
  });

  test("failures are listed with reasons and can be retried", async () => {
    const { calls } = show(
      summaryView({
        outcome: "failures",
        failed: 1,
        verified: 1283,
        failures: [
          {
            id: 7,
            path: "DCIM/A001.mov",
            finalPath: "DCIM/A001.mov",
            size: 10,
            millis: 1,
            hash: null,
            status: "failed",
            reason: "Hash mismatch (source 0000000000000001, copy 0000000000000002)",
          },
        ],
      }),
    );
    screen.getByText(/Hash mismatch/);
    await fireEvent.click(screen.getByRole("button", { name: "Retry failed" }));
    expect(calls).toEqual(["retry"]);
  });

  test("Reveal, Open checksum file and Save report", async () => {
    const { api } = show(summaryView());
    await fireEvent.click(screen.getByRole("button", { name: "Reveal in Finder" }));
    expect(api.reveal).toHaveBeenCalledWith("/Volumes/RAID/Day01/DCIM");
    await fireEvent.click(screen.getByRole("button", { name: "Open checksum file" }));
    expect(api.openFile).toHaveBeenCalledWith("/Volumes/RAID/Day01/secopy_2026-09-27_140302.xxh64");
    await fireEvent.click(screen.getByRole("button", { name: "Save report…" }));
    await waitFor(() => expect(api.saveReport).toHaveBeenCalledWith("/tmp/report.txt"));
    expect(api.pickReportPath).toHaveBeenCalledWith("r.txt");
  });

  test("a stopped job says why", () => {
    show(summaryView({ outcome: "stopped", stoppedBecause: "The destination drive is full", notStarted: 40 }));
    expect(screen.getByRole("status").textContent).toContain("Stopped: The destination drive is full");
    screen.getByText("40 not started");
  });

  test("a checksum file that couldn't be written is shown", () => {
    show(summaryView({ checksumFile: null, checksumError: "Permission denied" }));
    screen.getByText(/checksum file could not be written: Permission denied/);
    expect(screen.queryByRole("button", { name: "Open checksum file" })).toBeNull();
  });
});
```

Create `ui/src/lib/headline.test.ts`:

```ts
import { expect, test } from "vitest";
import { summaryView } from "../test/fake-api";
import { headline } from "./headline";

test("each outcome has a clear status line", () => {
  expect(headline(summaryView())).toBe("All 1,284 files copied and verified");
  expect(headline(summaryView({ verify: false, verified: 0, copied: 1 }))).toBe("All 1 file copied");
  expect(headline(summaryView({ outcome: "failures", failed: 3 }))).toBe("3 files failed");
  expect(headline(summaryView({ outcome: "cancelled" }))).toBe("Cancelled");
  expect(
    headline(summaryView({ outcome: "stopped", stoppedBecause: "The destination drive is full" })),
  ).toBe("Stopped: The destination drive is full");
  expect(headline(summaryView({ verified: 0, skippedIdentical: 12 }))).toBe(
    "Nothing to copy: everything was already there",
  );
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && npm test`
Expected: `Summary.test.ts`, `headline.test.ts` and `App.test.ts` fail: the modules don't exist.

- [ ] **Step 3: Implement**

Replace `ui/src/App.svelte` with:

```svelte
<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4).
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import type { ProgressView, SessionView, SummaryView } from "./lib/bindings";
  import JobProgress from "./components/JobProgress.svelte";
  import Setup from "./components/Setup.svelte";
  import Summary from "./components/Summary.svelte";

  let { api = tauriApi }: { api?: Api } = $props();
  // The app talks to one Api for its whole life; tests pass a fake one.
  // svelte-ignore state_referenced_locally
  provideApi(api);

  let screen: "setup" | "progress" | "summary" = $state("setup");
  let view: SessionView = $state({
    source: null,
    selectedFiles: 0,
    selectedBytes: 0,
    destination: null,
    conflicts: "keepBoth",
    plan: null,
    stale: false,
  });
  let verify = $state(true);
  let progress: ProgressView | null = $state(null);
  let summary: SummaryView | null = $state(null);
  let error: string | null = $state(null);

  /** What the progress view shows before the first update arrives. */
  const waiting = (): ProgressView => ({
    phase: "copying",
    elapsedMs: 0,
    paused: false,
    verify,
    totalFiles: view.selectedFiles,
    totalBytes: view.plan?.bytesToWrite ?? 0,
    copiedBytes: 0,
    verifiedBytes: 0,
    filesDone: 0,
    filesSkipped: 0,
    filesFailed: 0,
    active: [],
    smallFiles: null,
    fatal: null,
  });

  async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await action();
      error = null;
      return result;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return undefined;
    }
  }

  async function start() {
    progress = waiting();
    screen = "progress";
    const started = await run(() =>
      api.startJob(verify, (p) => {
        progress = p;
        if (p.phase === "done") void finish();
      }),
    );
    if (started === undefined) screen = "setup";
  }

  async function finish() {
    summary = (await run(() => api.jobSummary())) ?? null;
    if (summary) screen = "summary";
  }

  async function retry() {
    const next = await run(() => api.retryFailed());
    if (next) {
      view = next;
      screen = "setup";
    }
  }

  async function newCopy() {
    const next = await run(() => api.clearSource());
    if (next) {
      view = next;
      screen = "setup";
    }
  }

  onMount(() => {
    void run(() => api.sessionView()).then((v) => v && (view = v));
    // Closing during a copy asks first; if closed anyway, the app stops the copy cleanly.
    const unlisten = api.onCloseRequested(async (prevent) => {
      if (!(await api.jobRunning())) return;
      const stop = await api.confirm(
        "Files already copied stay and are listed in the checksum file; the file in progress is removed.",
        "Stop copying and quit?",
      );
      if (!stop) prevent();
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<main>
  <h1>Secopy</h1>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if screen === "setup"}
    <Setup bind:view bind:verify onStart={start} />
  {:else if screen === "progress" && progress}
    <JobProgress {progress} />
  {:else if screen === "summary" && summary}
    <Summary {summary} onRetry={retry} onNewCopy={newCopy} />
  {/if}
</main>

<style>
  main {
    max-width: 960px;
    margin: 0 auto;
    padding: 20px 24px;
  }

  h1 {
    margin: 0 0 14px;
    font-size: 18px;
  }

  .error {
    color: var(--danger);
  }
</style>
```

Create `ui/src/components/Summary.svelte`:

```svelte
<script lang="ts">
  // The summary (RFD §5.4): what happened, the figures, what failed and why, and what to do next.
  import { useApi } from "../lib/api";
  import type { SummaryView } from "../lib/bindings";
  import { formatBytes, formatCount, formatDuration, formatSpeed, plural } from "../lib/format";
  import { headline } from "../lib/headline";

  let {
    summary,
    onRetry,
    onNewCopy,
  }: {
    summary: SummaryView;
    onRetry: () => void;
    onNewCopy: () => void;
  } = $props();

  const api = useApi();
  let actionError: string | null = $state(null);
  const ok = $derived(summary.outcome === "complete");

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
      actionError = null;
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
    }
  }

  async function saveReport() {
    const name = summary.reportFile?.split("/").pop() ?? "secopy_report.txt";
    const path = await api.pickReportPath(name);
    if (path) await act(() => api.saveReport(path));
  }
</script>

<section class="card">
  <h2 class:ok class:bad={!ok} role="status">{ok ? "✓" : "✗"} {headline(summary)}</h2>
  <ul class="stats">
    <li>{plural(summary.files, "file")}</li>
    <li>{formatBytes(summary.bytesWritten)} written</li>
    <li>{formatDuration(summary.millis)}</li>
    <li>
      {formatSpeed(summary.millis > 0 ? (summary.bytesWritten * 1000) / summary.millis : null)} average
    </li>
    {#if summary.skippedIdentical > 0}
      <li>{formatCount(summary.skippedIdentical)} already at the destination, not checked</li>
    {/if}
    {#if summary.skippedDifferent > 0}
      <li>{plural(summary.skippedDifferent, "different file")} left as they were</li>
    {/if}
    {#if summary.notStarted > 0}
      <li>{formatCount(summary.notStarted)} not started</li>
    {/if}
  </ul>
  {#if summary.checksumError}
    <p class="danger" role="alert">The checksum file could not be written: {summary.checksumError}</p>
  {/if}

  <div class="actions">
    <button type="button" onclick={() => act(() => api.reveal(summary.copyRoot))}>Reveal in Finder</button>
    {#if summary.checksumFile}
      <button type="button" onclick={() => act(() => api.openFile(summary.checksumFile!))}>
        Open checksum file
      </button>
    {/if}
    <button type="button" onclick={saveReport}>Save report…</button>
    {#if summary.failed > 0}
      <button type="button" onclick={onRetry}>Retry failed</button>
    {/if}
    <button type="button" class="primary" onclick={onNewCopy}>New copy</button>
  </div>
  {#if actionError}<p class="danger" role="alert">{actionError}</p>{/if}
</section>

{#if summary.failures.length > 0}
  <section class="card" aria-labelledby="failures-title">
    <h3 id="failures-title">Failed</h3>
    <ul class="failures">
      {#each summary.failures as f (f.id)}
        <li><span class="mono">{f.path}</span>: {f.reason}</li>
      {/each}
      {#if summary.failed > summary.failures.length}
        <li class="muted">and {formatCount(summary.failed - summary.failures.length)} more (see the report)</li>
      {/if}
    </ul>
  </section>
{/if}

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  h2 {
    margin: 0 0 10px;
    font-size: 20px;
  }

  h2.ok {
    color: var(--success);
  }

  h2.bad {
    color: var(--danger);
  }

  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .stats {
    list-style: none;
    padding: 0;
    margin: 0 0 12px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .failures {
    margin: 0;
    padding-left: 18px;
    max-height: 280px;
    overflow-y: auto;
  }

  .danger {
    color: var(--danger);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
```

Create `ui/src/lib/headline.ts`:

```ts
// The summary's status line (RFD §5.4): unambiguous about what happened.

import type { SummaryView } from "./bindings";
import { plural } from "./format";

export function headline(s: SummaryView): string {
  switch (s.outcome) {
    case "stopped":
      return `Stopped: ${s.stoppedBecause ?? "the copy could not continue"}`;
    case "cancelled":
      return "Cancelled";
    case "failures":
      return `${plural(s.failed, "file")} failed`;
    case "complete": {
      const done = s.copied + s.verified;
      if (done === 0) return "Nothing to copy: everything was already there";
      return s.verify
        ? `All ${plural(done, "file")} copied and verified`
        : `All ${plural(done, "file")} copied`;
    }
  }
}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(ui): add the summary and connect the screens

A clear status line, the figures, the failures with reasons, and
Reveal / Open checksum file / Save report / Retry failed / New copy
(RFD §5.4). The app moves from setup to progress to the summary, and
asks before a window is closed during a copy.

Refs: #15
EOF
```


---

### Task 9: Release the app, and document it

`release-build.yml` now runs the UI checks with the Rust suite, builds the app with
`tauri build --target aarch64-apple-darwin --bundles dmg`, and attaches the unsigned
`Secopy_<version>_aarch64.dmg` and its SHA-256 next to the CLI archive.

The docs:
- **README:** how to install from a release, including the first-launch right-click → Open
  for an unsigned app, plus the UI's development commands.
- **AGENTS.md:** the new layout, the UI checks and the bindings rule. The rule to lint for
  Linux and Windows is now limited to the engine and CLI: the app's GTK dependencies can't
  build for Linux on a Mac.
- **`docs/testing/macos-app-checklist.md`:** the manual checklist for a real card, since
  Tauri's WebDriver doesn't support macOS.

**Files:**
- Create: `docs/testing/macos-app-checklist.md`
- Modify: `.github/workflows/release-build.yml`, `AGENTS.md`, `README.md`

**Interfaces:**
- Consumes: everything above.
- Produces: `.dmg` release assets; `docs/testing/macos-app-checklist.md`.

- [ ] **Step 1: Update the release workflow**

Change `.github/workflows/release-build.yml`:

```diff
diff --git a/.github/workflows/release-build.yml b/.github/workflows/release-build.yml
index a10e3fc..14c316e 100644
--- a/.github/workflows/release-build.yml
+++ b/.github/workflows/release-build.yml
@@ -1,6 +1,6 @@
 name: release-build
 
-# Tests, builds and packages the Apple Silicon binary (RFD NFR-9). release-please.yml
+# Tests, builds and packages the Apple Silicon app (.dmg) and CLI (RFD NFR-9). release-please.yml
 # calls it when a release is created and the files are attached to that release. Run it
 # by hand without a tag to check the build: the files are then kept as a run artifact.
 on:
@@ -30,14 +30,26 @@ jobs:
         with:
           ref: ${{ inputs.tag || github.ref }}
       - uses: Swatinem/rust-cache@v2
+      - uses: actions/setup-node@v7
+        with:
+          node-version: 24
+          cache: npm
+          cache-dependency-path: ui/package-lock.json
       - run: brew install xxhash
+      - run: npm ci
+        working-directory: ui
       # The same suite as ci.yml: nothing is released without it.
+      - run: npm run check && npm test
+        working-directory: ui
       - run: cargo test --workspace --locked
         env:
           SECOPY_REQUIRE_XXHSUM: "1"
           SECOPY_DEVICE_TESTS: "1"
       - run: rustup target add "$TARGET"
       - run: cargo build --release --locked -p secopy-cli --target "$TARGET"
+      # Unsigned for now: first launch is right-click → Open (see README).
+      - run: npm run tauri -- build --target "$TARGET" --bundles dmg
+        working-directory: ui
       - name: Package
         id: package
         run: |
@@ -47,15 +59,18 @@ jobs:
           cp "target/$TARGET/release/secopy-cli" README.md "dist/$name/"
           tar -C dist -czf "dist/$name.tar.gz" "$name"
           (cd dist && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
-          echo "name=$name" >> "$GITHUB_OUTPUT"
+          cp target/"$TARGET"/release/bundle/dmg/*.dmg dist/
+          (cd dist && for f in *.dmg; do shasum -a 256 "$f" > "$f.sha256"; done)
+          echo "artifact=secopy-${version}-macos-arm64" >> "$GITHUB_OUTPUT"
       - uses: actions/upload-artifact@v7
         with:
-          name: ${{ steps.package.outputs.name }}
+          name: ${{ steps.package.outputs.artifact }}
           path: |
             dist/*.tar.gz
+            dist/*.dmg
             dist/*.sha256
       - name: Attach to the release
         if: inputs.tag != ''
         env:
           GH_TOKEN: ${{ github.token }}
-        run: gh release upload "${{ inputs.tag }}" dist/*.tar.gz dist/*.sha256 --clobber
+        run: gh release upload "${{ inputs.tag }}" dist/*.tar.gz dist/*.dmg dist/*.sha256 --clobber
```

- [ ] **Step 2: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Rehearse the packaging step locally after a `tauri build`: run the `Package` step's
script with `TARGET=aarch64-apple-darwin GITHUB_OUTPUT=/tmp/out` (and a built `secopy-cli`);
`dist/` must hold the `.dmg`, the CLI archive and a `.sha256` for each, and
`(cd dist && shasum -c *.sha256)` must pass.

- [ ] **Step 3: Commit, then document the app**

```bash
git add -A
git commit -F - <<'EOF'
ci: attach the macos app to each release

release-build.yml runs the UI checks, builds the app for Apple Silicon
and attaches the unsigned .dmg and its SHA-256 to each release.

Refs: #15
EOF
```


Change `AGENTS.md`:

```diff
diff --git a/AGENTS.md b/AGENTS.md
index b688913..19ac0e8 100644
--- a/AGENTS.md
+++ b/AGENTS.md
@@ -122,14 +122,22 @@ format exactly.
 
 ## Development
 
-- Layout: `crates/secopy-core` (engine library, no UI dependencies) and `crates/secopy-cli`
-  (developer CLI and benchmark driver). The Tauri app and Svelte UI come in plan 3.
+- Layout: `crates/secopy-core` (engine library, no UI dependencies), `crates/secopy-cli`
+  (developer CLI and benchmark driver), `crates/secopy-app` (the Tauri 2 shell: session,
+  jobs, commands) and `ui/` (Svelte 5 + TypeScript, Vite). Run the app with
+  `npm run tauri dev` from `ui/`; build the `.dmg` with `npm run tauri build`.
 - Engine flow: `scan → select → preflight → Plan::resolve → run_job`, then `Report` for
   the job report. Design notes per plan are in `docs/superpowers/specs/`.
 - Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
   and `cargo test --workspace`. All must pass. When a change touches Linux- or Windows-only
-  `cfg` code, also run clippy with `--target x86_64-unknown-linux-gnu` and
-  `--target x86_64-pc-windows-msvc` (add them once with `rustup target add`).
+  `cfg` code in the engine, also run `cargo clippy -p secopy-core -p secopy-cli --all-targets`
+  with `--target x86_64-unknown-linux-gnu` and `--target x86_64-pc-windows-msvc` (add them
+  once with `rustup target add`). The app crate is macOS only and isn't linted for them.
+- UI checks, from `ui/` (`npm ci` once): `npm run check` (svelte-check) and `npm test`
+  (Vitest). Both must pass before every commit that touches `ui/` or `crates/secopy-app`.
+- `ui/src/lib/bindings.ts` is generated from the app's commands and DTOs by tauri-specta.
+  After changing either, run `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app`; a test
+  fails while the committed file is out of date. Never edit it by hand.
 - `xxhsum` must be installed locally (`brew install xxhash`). CI runs its compatibility
   test with `SECOPY_REQUIRE_XXHSUM=1` so it can't silently skip.
 - CI (`.github/workflows/ci.yml`) is one macOS job that runs after each merge to `main` and
```

Change `README.md`:

````diff
diff --git a/README.md b/README.md
index 21a3d0b..25fd43f 100644
--- a/README.md
+++ b/README.md
@@ -4,19 +4,34 @@ Fast, verified file copies for macOS. Copy a folder or a set of
 files, optionally verify every copy with xxHash64, and get an `xxhsum`-compatible
 checksum file in the destination.
 
-> **Status:** early development. The engine and a developer CLI come first; the desktop
-> app follows. Design: [RFD 0001](docs/rfd/0001-secopy.md).
+> **Status:** early development, Apple Silicon Macs only. Design:
+> [RFD 0001](docs/rfd/0001-secopy.md).
+
+## Install
+
+Download `Secopy_<version>_aarch64.dmg` from the
+[latest release](https://github.com/AbelCS/secopy/releases/latest) and drag Secopy to
+Applications. The app isn't signed yet, so the first time macOS refuses to open it:
+right-click Secopy in Applications, choose **Open**, then **Open** again. After that it
+starts normally.
 
 ## Development
 
 Requirements: Rust via [rustup](https://rustup.rs) (the toolchain is pinned in
-`rust-toolchain.toml`), and `xxhsum` for the checksum compatibility test
-(`brew install xxhash` or `apt install xxhash`).
+`rust-toolchain.toml`), Node 24 for the UI, and `xxhsum` for the checksum compatibility
+test (`brew install xxhash`).
 
 ```sh
 cargo test --workspace
 cargo clippy --workspace --all-targets -- -D warnings
 cargo fmt --all
+cd ui && npm ci && npm run check && npm test
+```
+
+Run the app:
+
+```sh
+cd ui && npm run tauri dev
 ```
 
 Try the CLI:
````

Create `docs/testing/macos-app-checklist.md`:

```markdown
# macOS app: manual checklist

Tauri's WebDriver doesn't support macOS, so the app gets this check by hand on a real Mac
with a real SD card (or any USB stick) before each release. Build it with
`npm run tauri build` from `ui/` (or install the release `.dmg`), then:

1. **Normal offload.** Insert a card with some video files. Drop its folder from Finder on
   FROM, choose an empty destination on TO. Expect: file count, size and hidden items
   skipped; "Files will go to" ends in the card folder's name. Start with Copy & Verify.
   Expect: both bars move, active files show, the finished list fills, the summary says
   "All N files copied and verified".
2. **Checksum file.** Summary → Open checksum file opens it; in Terminal,
   `cd <destination> && xxhsum -c secopy_*.xxh64` prints `OK` for every file.
3. **Run it again.** New copy, same card, same destination. Expect: the non-empty warning
   and "N identical files will be skipped (not checked)"; the job finishes at once and the
   summary says nothing had to be copied.
4. **Different files with the same name.** Change one file on the card (or copy another
   file over it), run again with Keep both. Expect: "1 file differs", and the copy lands as
   `name (1).ext`.
5. **Only what's inside, and the filter.** Choose "Copy only what's inside" and turn off an
   extension chip. Expect: the counts and "Files will go to" follow.
6. **Pause and Cancel.** Start a large copy, Pause (the bars stop), Resume, then Cancel and
   confirm. Expect: the summary says Cancelled, and no `.secopy-partial` file is left in
   the destination (`ls -la`).
7. **Pull the card.** Start a large copy and eject/pull the card mid-way. Expect: a red
   "Stopped: the source is no longer available" and a summary that lists what finished.
8. **Retry failed.** Make a file unreadable (`chmod 000` on a copy of a card folder),
   copy, then Retry failed. Expect: only that file is offered again.
9. **Closing during a copy.** Start a copy, close the window: it asks first; Keep copying
   keeps it open, Stop copying quits and leaves no partial file.
10. **Save report.** Summary → Save report… writes a `.txt` and a `.json` next to it.

Record the macOS version, the card reader and anything odd in the release PR.
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -F - <<'EOF'
docs: document the app, its checks and a manual release checklist

README: installing from a release and the first launch of an unsigned
app. AGENTS.md: the layout, the UI checks and the bindings rule; the
Linux/Windows lint rule now covers the engine and CLI only.
docs/testing/macos-app-checklist.md: the manual check on a real card.

Refs: #15
EOF
```


---

### Task 10: Manual check, roadmap, pull request

**Files:**
- Modify: `docs/superpowers/plans/2026-09-26-v1-roadmap.md`

**Interfaces:**
- Consumes: everything above.
- Produces: docs only.

- [ ] **Step 1: Run the manual checklist (the user)**

Ask the user to build the app (`cd ui && npm run tauri build`, or `npm run tauri dev`) and
go through `docs/testing/macos-app-checklist.md` with a real card. Fix what they find, each
with a failing test first, in `fix(app)` or `fix(ui)` commits.

- [ ] **Step 2: Mark plan 3a done in the roadmap**

In `docs/superpowers/plans/2026-09-26-v1-roadmap.md`, change the 3a row to
`| 3a | [Desktop app core](2026-09-27-macos-app-core.md) | M2 | 0.2.0 | Done (#15) |`.

```bash
git add docs/superpowers/plans/2026-09-26-v1-roadmap.md
git commit -F - <<'EOF'
docs: mark plan 3a done in the roadmap

Refs: #15
EOF
```

- [ ] **Step 3: Push and open the pull request (ask the user first)**

```bash
git push -u origin feat/15-macos-app
gh pr create --base main --title "feat(app): macOS app core (plan 3a)" --body "Closes #15 …"
```

CI doesn't run on PRs. Merge with `gh pr merge --rebase` (never squash) once the user
agrees. Check `gh pr list` for an open release PR first and ask whether it should go first.
The `ci` run on `main` after the merge is the clean-machine check; if it fails, open an
issue and fix it before anything else. Then confirm #15 is closed.
