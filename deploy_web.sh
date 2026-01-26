#!/bin/bash
set -e

echo "Building Vortex Client..."
cd ../vortex-client
npm run build

echo "Deploying to Vortex Server..."
rm -rf ../vortex-server/static/*
mkdir -p ../vortex-server/static
cp -r dist/* ../vortex-server/static/

echo "Web app deployed successfully!"
