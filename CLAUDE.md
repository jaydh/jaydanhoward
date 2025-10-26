# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Personal website monorepo for jaydanhoward.com built with:
- **Leptos** (Rust full-stack web framework with SSR and hydration)
- **Bazel** (build system with bzlmod enabled)
- **WASM-Bindgen** for client-side WebAssembly
- **Actix-web** for the server runtime
- **TailwindCSS** for styling

The application uses Leptos's isomorphic architecture: server-side rendering (SSR) with the `ssr` feature, and client-side hydration with the `hydrate` feature. The same Rust codebase compiles to both native binary (server) and WASM (client).

## Build Commands

### Development
```bash
# Run local development server (localhost:8000)
bazel run :jaydanhoward_bin

# Run clippy checks
bazel build clippy
```

### Production Builds
```bash
# Build multi-architecture OCI images
bazel build jaydanhoward_image_amd64
bazel build jaydanhoward_image_arm64

# Push images to Harbor registry (requires authentication)
bazel run jaydanhoward_image_amd64_push
bazel run jaydanhoward_image_arm64_push
```

### Dependencies
```bash
# After modifying dependencies in WORKSPACE.bazel, repin lock files:
# Update crates in crates_repository definitions in WORKSPACE.bazel
# Then run:
bazel sync --only=server_crates
bazel sync --only=wasm_crates
```

## Architecture

### Build System Architecture

**Dual Crate System**: The project maintains separate dependency trees for server and WASM targets:
- `server_crates`: Native dependencies (Cargo.server.lock, Cargo.Bazel.server.lock)
- `wasm_crates`: WASM-compatible dependencies (Cargo.wasm.lock, Cargo.Bazel.wasm.lock)

This separation is necessary because WASM targets have different requirements (e.g., `getrandom` needs the `js` feature for WASM).

**Platform-Specific Tooling**: The build system selects platform-specific binaries using Bazel's `select()`:
- TailwindCSS binaries for linux_x86_64, linux_arm64, macos_arm64
- wasm-bindgen toolchain for each platform (defined in wasm_bindgen/BUILD)

**Custom TailwindCSS Rule** (`bzl/tailwindcss.bzl`): A Bazel aspect that:
1. Collects all source files from the target and its dependencies via `_srcs_aspect`
2. Runs platform-specific TailwindCSS binary with minification (`-m`) on the collected sources
3. Outputs processed CSS as a build artifact

### Application Architecture

**Entry Points**:
- `src/main.rs`: Server entry point, runs Actix-web server (feature `ssr`)
- `src/lib.rs`: WASM entry point, exports `hydrate()` function (feature `hydrate`)

**Structure**:
- `src/components/`: Leptos components (app.rs is the root App component)
- `src/routes/`: HTTP route handlers (health_check, robots, lighthouse metrics)
- `src/startup.rs`: Server initialization and configuration
- `src/telemtry.rs`: Tracing and logging setup
- `src/prometheus_client.rs`: Prometheus metrics client

**Asset Handling**: Static assets in `assets/` are managed through Bazel filegroups:
- Fonts, favicons, images organized in subdirectories
- Each asset directory has its own BUILD file
- Assets are included in the binary via the `data` attribute in BUILD

**Rust Configuration**:
- Uses nightly Rust toolchain (configured in .bazelrc)
- Clippy aspects run on all builds for linting
- Custom rustc flags: `-C opt-level=3 -C codegen-units=1` for optimization

## CI/CD

GitHub Actions workflow (`.github/workflows/general.yml`) runs on self-hosted Kubernetes runners:
- Clippy checks on all PRs and pushes
- Multi-architecture image builds (x86_64 and arm64 on separate runner sets)
- Image push to Harbor registry (harbor.home.local) only on push to main
- Manifest creation for multi-arch support
- Lighthouse Docker image build (separate from main Bazel build)

The workflow uses custom runner images and supports both DinD and Kubernetes mode via actions-runner-controller.

## Development Notes

**Feature Flags**: Code must be conditionally compiled based on target:
- Use `#[cfg(feature = "ssr")]` for server-only code
- Use `#[cfg(feature = "hydrate")]` for WASM/client code
- Leptos components are typically shared between both

**Environment Variable**: Both server and WASM builds set `SERVER_FN_OVERRIDE_KEY=bazel` to handle Leptos server functions in the Bazel build environment.

**Runfiles**: The server binary expects runfiles (WASM artifacts, assets) in the `jaydanhoward_bin.runfiles` directory. OCI images set `workdir` accordingly.
