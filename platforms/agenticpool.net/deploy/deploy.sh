#!/usr/bin/env bash
set -euo pipefail

# Deployment script for AgenticPool.net to k3s cluster
# Target Host: u2mes4@155.133.27.1
# Target Namespace: agenticpool

REMOTE_HOST="${1:-u2mes4@155.133.27.1}"
REMOTE_BUILD_DIR="/tmp/agora-build-$$"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

echo "========================================================"
echo "🚀 Deploying AgenticPool.net to k3s at ${REMOTE_HOST}"
echo "📁 Repository Root: ${REPO_ROOT}"
echo "🌐 Namespace: agenticpool"
echo "========================================================"

# Step 1: Create remote build directory
echo "📦 1/5 Creating remote temporary build workspace..."
ssh -o BatchMode=yes "$REMOTE_HOST" "mkdir -p $REMOTE_BUILD_DIR"

# Step 2: Transfer codebase & Dockerfile (excluding large build artifacts & node_modules)
echo "📤 2/5 Syncing codebase to remote host..."
tar -C "$REPO_ROOT" \
    --exclude='.git' \
    --exclude='target' \
    --exclude='node_modules' \
    --exclude='dist' \
    --exclude='.agenticpool' \
    -czf - . | ssh -o BatchMode=yes "$REMOTE_HOST" "tar -xzf - -C $REMOTE_BUILD_DIR"

# Step 3: Build & Push Docker image on remote host to local registry
echo "🔨 3/5 Building container image (localhost:5000/agora:latest) on remote host..."
ssh -o BatchMode=yes "$REMOTE_HOST" "
    cd $REMOTE_BUILD_DIR && \
    sudo docker build -t localhost:5000/agora:latest -f Dockerfile . && \
    sudo docker push localhost:5000/agora:latest && \
    rm -rf $REMOTE_BUILD_DIR
"

# Step 4: Apply Kubernetes Manifests
echo "☸️  4/5 Applying Kubernetes manifests to namespace 'agenticpool'..."
ssh -o BatchMode=yes "$REMOTE_HOST" "mkdir -p /tmp/k8s-manifests-$$"
tar -C "$SCRIPT_DIR/k8s" -czf - . | ssh -o BatchMode=yes "$REMOTE_HOST" "tar -xzf - -C /tmp/k8s-manifests-$$"

ssh -o BatchMode=yes "$REMOTE_HOST" "
    sudo kubectl apply -f /tmp/k8s-manifests-$$/00-namespace.yaml
    sudo kubectl apply -f /tmp/k8s-manifests-$$/01-secrets-and-config.yaml
    sudo kubectl apply -f /tmp/k8s-manifests-$$/02-postgres.yaml
    sudo kubectl apply -f /tmp/k8s-manifests-$$/03-llull.yaml
    sudo kubectl apply -f /tmp/k8s-manifests-$$/04-gateway.yaml
    sudo kubectl apply -f /tmp/k8s-manifests-$$/05-ingress.yaml
    rm -rf /tmp/k8s-manifests-$$
"

# Step 5: Wait for Rollout & Verify Health
echo "⏳ 5/5 Verifying pod rollouts and health..."
ssh -o BatchMode=yes "$REMOTE_HOST" "
    sudo kubectl rollout status statefulset/postgres -n agenticpool --timeout=120s
    sudo kubectl rollout status deployment/llull -n agenticpool --timeout=120s
    sudo kubectl rollout status deployment/gateway -n agenticpool --timeout=120s
"

echo ""
echo "🔍 Checking cluster status in namespace 'agenticpool':"
ssh -o BatchMode=yes "$REMOTE_HOST" "
    sudo kubectl get all,pvc,ingress -n agenticpool
"

echo ""
echo "🩺 Testing Gateway health check:"
ssh -o BatchMode=yes "$REMOTE_HOST" "
    GATEWAY_IP=\$(sudo kubectl get pod -n agenticpool -l app=gateway -o jsonpath='{.items[0].status.podIP}')
    echo 'Gateway Pod IP:' \$GATEWAY_IP
    curl -s http://\$GATEWAY_IP:7100/health
"

echo ""
echo "========================================================"
echo "✅ Deployment completed successfully!"
echo "🌐 API Endpoint: https://api.agenticpool.net"
echo "🌐 Alt Endpoint: https://agenticpool.2mes4.com"
echo "========================================================"
