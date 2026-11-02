# OpenConv

A premium native desktop wrapper frontend studio for FFmpeg, built with Tauri and web technologies.

## Prerequisites

Before running or building OpenConv, you must ensure that you have the following prerequisites installed on your system:

### 1. FFmpeg
FFmpeg is the core engine that powers the conversions and processing under the hood.
- **Windows**: Download the latest release from the [official site](https://ffmpeg.org/download.html#build-windows) or use a package manager like winget:
  ```bash
  winget install ffmpeg
  ```
- **macOS**: Install via Homebrew:
  ```bash
  brew install ffmpeg
  ```
- **Linux**: Use your distribution's package manager:
  ```bash
  sudo apt install ffmpeg
  ```
  
> [!IMPORTANT]  
> After installation, you **must** ensure that `ffmpeg` is added to your system's PATH. You can verify this by opening a terminal and running `ffmpeg -version`. If it prints the version information, you are good to go!

### 2. Node.js & npm
Required for managing the frontend dependencies.
- Download from [Node.js official website](https://nodejs.org/)

### 3. Rust & Cargo
Required for compiling the Tauri backend.
- Install from [rustup.rs](https://rustup.rs/)

## Getting Started

1. **Install Dependencies:**
   ```bash
   npm install
   ```

2. **Run in Development Mode:**
   ```bash
   npm run tauri dev
   ```

3. **Build for Production:**
   ```bash
   npm run tauri build
   ```

## Project Layout

- `src/` – the UI: `index.html` (markup and styles), `app.js` (behaviour) and `formats.js` (the output format list).
- `src-tauri/src/main.rs` – the Tauri app: commands, process handling and events.
- `src-tauri/core/` – `openconv-core`, all logic that doesn't need a window: building FFmpeg arguments, choosing output paths, parsing progress and turning FFmpeg errors into readable messages.

> [!NOTE]
> To add an output format, add it to `formats.js` **and** to `FORMAT_PRESETS` in `app.js`. Formats without a preset are hidden from the picker, and a backend `Container` must exist for the preset's `container` value (the tests check this).

## Testing

```bash
npm run lint        # ESLint for the UI scripts
npm test            # UI tests (Vitest + jsdom)
npm run test:rust   # Rust tests for openconv-core
npm run lint:rust   # rustfmt + clippy (needs the Tauri system libraries)
npm run check       # lint + UI tests + Rust tests
```

The Rust tests include end-to-end conversions with the real `ffmpeg` binary for every output format. They are skipped when `ffmpeg` is not on your PATH; set `OPENCONV_REQUIRE_FFMPEG=1` to make that an error instead (CI does this).

`openconv-core` has no Tauri dependency, so `npm run test:rust` works without the WebKit/GTK libraries. CI (`.github/workflows/ci.yml`) runs everything on every pull request, and also type-checks the full app on Linux, Windows and macOS.
