#!/usr/bin/env bash
set -euo pipefail

repo="${AGYLINE_REPO:-CPbianma/AGYCometixLine}"
version="${AGYLINE_VERSION:-latest}"

case "$(uname -s)" in
  Linux) os="linux" ;;
  Darwin) os="macos" ;;
  *)
    printf 'agyline installer: unsupported operating system: %s\n' "$(uname -s)" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *)
    printf 'agyline installer: unsupported CPU architecture: %s\n' "$(uname -m)" >&2
    exit 1
    ;;
esac

if [[ "$os" == "linux" && "$arch" == "x86_64" ]]; then
  target="x86_64-unknown-linux-gnu"
elif [[ "$os" == "macos" && "$arch" == "x86_64" ]]; then
  target="x86_64-apple-darwin"
elif [[ "$os" == "macos" && "$arch" == "aarch64" ]]; then
  target="aarch64-apple-darwin"
else
  printf 'agyline installer: no prebuilt binary for %s %s; build from source instead.\n' "$os" "$arch" >&2
  exit 1
fi

for dependency in curl tar awk; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    printf 'agyline installer: required command not found: %s\n' "$dependency" >&2
    exit 1
  fi
done

asset="agyline-${target}.tar.gz"
if [[ "$version" == "latest" ]]; then
  base_url="https://github.com/${repo}/releases/latest/download"
else
  base_url="https://github.com/${repo}/releases/download/${version}"
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/agyline-install.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

printf 'Downloading %s (%s)...\n' "$asset" "$version"
curl --fail --location --silent --show-error --retry 3 \
  "$base_url/$asset" --output "$tmp_dir/$asset"
curl --fail --location --silent --show-error --retry 3 \
  "$base_url/SHA256SUMS" --output "$tmp_dir/SHA256SUMS"

expected="$(awk -v file="$asset" '$2 == file { print $1; exit }' "$tmp_dir/SHA256SUMS")"
if [[ -z "$expected" ]]; then
  printf 'agyline installer: no checksum published for %s.\n' "$asset" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp_dir/$asset" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$tmp_dir/$asset" | awk '{ print $1 }')"
else
  printf 'agyline installer: sha256sum or shasum is required to verify the download.\n' >&2
  exit 1
fi

if [[ "$actual" != "$expected" ]]; then
  printf 'agyline installer: checksum mismatch for %s.\n' "$asset" >&2
  exit 1
fi

tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
chmod +x "$tmp_dir/agyline"
"$tmp_dir/agyline" install

printf '\nConfigure agyline with:\n  %s --config\n' \
  "${AGYLINE_HOME:-$HOME/.gemini/antigravity-cli/agyline}/agyline"
