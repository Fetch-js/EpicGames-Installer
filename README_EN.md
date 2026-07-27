# Epic Games Launcher Setup

Desktop installer UI built with Tauri 2, vanilla JavaScript and a native C++20 copy engine.

## Development

Requirements: Node.js, Rust, Visual Studio Build Tools with the Desktop development with C++ workload, and WebView2.

```powershell
npm install
npm run tauri dev
```

## Downloaded payload

Set `PAYLOAD_URL` near the top of `src-tauri/src/lib.rs` to a direct HTTPS URL of a ZIP archive. The installer downloads the archive, safely extracts it to a temporary directory, and passes the extracted files to the native C++ copy engine. Temporary files are deleted automatically.

The expected main executable name for the desktop shortcut is `EpicGamesLauncher.exe`.

## Release build

```powershell
npm run tauri build
```
