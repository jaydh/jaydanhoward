# Mildly unhinged "Monorepo" for self-hosting personal website at jaydanhoward.com

### Technologies included

 - Rust
 - Foster (server-owned state machines + generic WASM client, https://github.com/jaydh/foster)
 - Mutli-architecture OCI images
 - CICD integrated with self-hosted mult-cpu-architecture Kubernetes cluster utilizing https://github.com/actions/actions-runner-controller with both DinD and k8s mode runners

### Startup

 1. `cd foster-server && cargo run`
 2. localhost:8000
