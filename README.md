# Heimdall

Connect Facebook ad accounts, analyze performance with an LLM agent, and deliver statistics where your team works.

Heimdall is a desktop app built with Tauri 2. The UI is React and lives in `client/`. The backend is a Rust core under `src-tauri/` that runs inside the app and talks to the UI through a single IPC command.

## Requirements

- Node.js 22
- Rust stable (install with [rustup](https://rustup.rs))
- Linux only: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev patchelf`

## Run in development

```
npm install
npm run dev
```

This starts Vite on port 5173, compiles the Rust core and opens the app. The first build downloads and compiles the Rust dependencies, so it takes a few minutes.

## Build an installer

```
npm run build
```

The installer for the current operating system is written to `src-tauri/target/release/bundle/`. To build one format only, pass it through, for example `npm run build -- --bundles nsis`.

Pushing a tag that starts with `v` builds installers for Windows, macOS and Linux in GitHub Actions and attaches them to a release.

## Where data lives

The database and encryption key are stored in the same folder the Electron version used:

- Windows: `%APPDATA%\Heimdall`
- macOS: `~/Library/Application Support/Heimdall`
- Linux: `~/.config/Heimdall`

Files: `heimdall.db` and `secret.key`. Keys and tokens saved in Settings are encrypted with `secret.key`, so keep both files together. Data from the Electron version is read as is.

## Move data from the old Docker version

Close Heimdall, copy `heimdall.db` and `secret.key` from the old data volume into the folder above, then start the app.

## Tray

Closing the window hides Heimdall to the system tray so scheduled syncs, reports and alerts keep running. Use Quit in the tray menu to exit.
