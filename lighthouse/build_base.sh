#!/bin/bash
# Build and push the lighthouse base image for multiple architectures
# This needs to be run once before using the Bazel OCI rules

set -e

REGISTRY="harbor.home.local/library"
IMAGE_NAME="lighthouse-base"
TAG="latest"

echo "Building multi-architecture lighthouse base image..."
echo "Registry: $REGISTRY"
echo "Image: $IMAGE_NAME:$TAG"
echo ""

# Check if buildx is available
if ! docker buildx version > /dev/null 2>&1; then
    echo "Error: Docker buildx is not available"
    echo "Please install Docker with buildx support"
    exit 1
fi

# Create builder if it doesn't exist
if ! docker buildx inspect multiarch-builder > /dev/null 2>&1; then
    echo "Creating buildx builder..."
    docker buildx create --name multiarch-builder --use
fi

# Use the builder
docker buildx use multiarch-builder

# Build and push multi-arch image
echo "Building and pushing for linux/amd64 and linux/arm64..."
docker buildx build \
    --platform linux/amd64,linux/arm64 \
    --file Dockerfile.base \
    --tag "$REGISTRY/$IMAGE_NAME:$TAG" \
    --push \
    .

echo ""
echo "✅ Successfully built and pushed $REGISTRY/$IMAGE_NAME:$TAG"
echo ""
echo "You can now build the final lighthouse image with Bazel:"
echo "  bazel build //lighthouse:lighthouse_image"
echo "  bazel run //lighthouse:lighthouse_image_push"
