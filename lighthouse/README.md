# Lighthouse CI Image

This directory contains the Lighthouse CI image that runs performance audits on jaydanhoward.com.

## Architecture

The image uses a **two-stage build approach** to work with Bazel OCI rules:

1. **Base Image** (`lighthouse-base`): Built with Docker, contains all dependencies
2. **Final Image** (`lighthouse`): Built with Bazel OCI, adds entrypoint script

### Why This Approach?

Bazel's OCI rules are excellent for layering pre-built artifacts but cannot execute build commands like `apt-get install` or `npm install`. The base image handles all complex installation steps, then Bazel OCI adds your application logic on top.

## Files

- `Dockerfile.base` - Builds the base image with Chromium, Node.js, and Lighthouse
- `Dockerfile` - Legacy Docker build (kept for reference)
- `entrypoint.sh` - Script that runs health checks and Lighthouse
- `BUILD` - Bazel OCI build configuration
- `build_base.sh` - Helper script to build multi-arch base image

## Building

### One-Time Setup: Build Base Image

The base image needs to be built once and pushed to Harbor before using Bazel:

```bash
cd lighthouse
./build_base.sh
```

This builds the base image for both `linux/amd64` and `linux/arm64` and pushes to:
```
harbor.home.local/library/lighthouse-base:latest
```

**When to rebuild the base:**
- Updating Node.js version
- Updating Lighthouse version
- Adding new system dependencies
- Updating Chromium

### Building Final Image with Bazel

Once the base image exists:

```bash
# Build the image
bazel build //lighthouse:lighthouse_image

# Push to Harbor
bazel run //lighthouse:lighthouse_image_push
```

## CI/CD

The GitHub Actions workflow automatically:
1. Pulls the base image from Harbor
2. Builds the final image with Bazel OCI (adds entrypoint.sh)
3. Pushes to `harbor.home.local/library/lighthouse:latest`

The base image is **not** rebuilt in CI - it's a pre-requisite.

## Environment Variables

Set these when running the container:

- `LEPTOS_SITE_HEALTH_ADDR` - Health check endpoint (default: https://jaydanhoward.com/health_check)
- `LEPTOS_SITE_API_ADDR` - Lighthouse upload endpoint (default: https://jaydanhoward.com/api/lighthouse)
- `LEPTOS_SITE_TARGET_ADDR` - Page to audit (default: https://jaydanhoward.com/about)
- `LIGHTHOUSE_UPDATE_TOKEN` - Authentication token for uploading results

## Manual Testing

```bash
docker run \
  -e "LIGHTHOUSE_UPDATE_TOKEN=your-token" \
  harbor.home.local/library/lighthouse:latest
```

## Troubleshooting

### Base image not found

If Bazel fails with `repository 'lighthouse_base' could not be resolved`:

1. Build and push the base image: `cd lighthouse && ./build_base.sh`
2. Verify it exists: `docker pull harbor.home.local/library/lighthouse-base:latest`
3. Try building again

### Entrypoint not executable

If the entrypoint fails to execute, check that it has correct permissions in the BUILD file (mode = "0755").

### Chromium crashes

The entrypoint uses `--no-sandbox --disable-dev-shm-usage` flags for Chromium. These are required when running in containers.
