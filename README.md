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
