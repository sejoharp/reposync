#!/usr/bin/env bash
set -x
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
if [ "${OS}" != "linux" ] && [ "${OS}" != "darwin" ]; then
  echo "Unsupported OS: ${OS}"
  exit 1
fi
if [ "${OS}" == "linux" ]; then
  OS="unknown-linux"
elif [ "${OS}" == "darwin" ]; then
  OS="apple-darwin"
fi

ARCH=$(uname -m)
if [ "${ARCH}" == "x86_64" ]; then
  ARCH="amd64"
elif [ "${ARCH}" == "arm64" ]; then
  ARCH="aarch64"
else
  echo "Unsupported architecture: ${ARCH}"
  exit 1
fi

echo "detected os: ${OS}"
echo "detected arch: ${ARCH}"
RELEASE_META_DATA_URL="https://api.github.com/repos/sejoharp/reposync/releases/latest"
BINARY_URL=$(curl -s ${RELEASE_META_DATA_URL} | jq -r ".assets[] | select(.name | contains(\"${ARCH}-${OS}\")) | .browser_download_url")
echo "downloading ${BINARY_URL}"
curl -sLo reposync "${BINARY_URL}"
echo "make it executable"
chmod +x reposync
echo "installing to ${HOME}/bin/reposync"
mv reposync "${HOME}/bin/"
