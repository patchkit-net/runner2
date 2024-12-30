# PatchKit Runner 2

A modern, Rust-based launcher application that manages downloading, updating, and launching patcher applications for PatchKit.

## Features

- 🚀 Automatic updates checking and downloading
- 🔒 Secure app secret handling
- 🌐 Network connectivity verification
- 📦 Version management
- 🎯 Manifest-based execution
- 🖥️ Modern dark-themed UI using egui
- 💨 Asynchronous operations with tokio
- 📊 Download progress tracking

## Prerequisites

- Rust 1.82 or higher
- Cargo package manager

## Installation

1. Clone the repository:
```bash
git clone [repository-url]
cd runner2
```

2. Build the project:
```bash
cargo build
```

3. Run the application:
```bash
cargo run
```

## Configuration

The application requires a `launcher.dat` file in the root directory containing the necessary launcher configuration data. This file should include:
- App secret
- Patcher secret
- Other launcher-specific configuration

## Project Structure

- `src/`
  - `config/` - Configuration handling
  - `file/` - File management operations
  - `launcher/` - Core launcher functionality
  - `network/` - Network operations and downloads
  - `ui/` - User interface components
  - `manifest/` - Manifest parsing and handling

## Development

To run tests:
```bash
cargo test
```

To build in release mode:
```bash
cargo build --release
```

## License

BSD - See LICENSE file for details 