# Mildly unhinged "Monorepo" for self-hosting personal website at jaydanhoward.com

### Technologies included

 - Rust
 - Bazel
 - WASM-Bindgen
 - Leptos
 - Mutli-architecture OCI images
 - TailwindCSS
 - CICD integrated with self-hosted mult-cpu-architecture Kubernetes cluster utilizing https://github.com/actions/actions-runner-controller with both DinD and k8s mode runners

### Startup

 1. Install https://github.com/bazelbuild/bazelisk
 2. bazel run :jaydanhoward_bin
 3. localhost:8000
