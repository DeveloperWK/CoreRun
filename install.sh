#!/bin/bash
set -e
VERSION="v1.0.0"
URL="https://github.com/DeveloperWK/corerun/releases/download/$VERSION/corerun"

echo "⬇️  Downloading CoreRun $VERSION..."
curl -L $URL -o corerun
chmod +x corerun
sudo mv corerun /usr/local/bin/
echo "✅ CoreRun installed successfully!"
