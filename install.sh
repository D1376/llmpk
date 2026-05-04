#!/usr/bin/env bash
set -euo pipefail

repo="${LLMPK_REPO:-D1376/llmpk}"
bin="${LLMPK_BIN:-llmpk}"
version="${LLMPK_VERSION:-latest}"

log() {
  printf 'llmpk installer: %s\n' "$*"
}

fail() {
  printf 'llmpk installer: error: %s\n' "$*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

download() {
  local url="$1"
  local dest="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$dest"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
  else
    fail "curl or wget is required to download release assets"
  fi
}

target_triple() {
  local os
  local arch

  case "$(uname -s)" in
    Darwin) os="apple-darwin" ;;
    Linux) os="unknown-linux-gnu" ;;
    *) fail "unsupported OS: $(uname -s)" ;;
  esac

  case "$(uname -m)" in
    x86_64 | amd64) arch="x86_64" ;;
    arm64 | aarch64) arch="aarch64" ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
  esac

  case "${arch}-${os}" in
    x86_64-unknown-linux-gnu | aarch64-apple-darwin)
      printf '%s-%s' "$arch" "$os"
      ;;
    x86_64-apple-darwin)
      fail "macOS Intel release binaries are not published yet; install from source with cargo install --git https://github.com/${repo}.git"
      ;;
    aarch64-unknown-linux-gnu)
      fail "Linux arm64 release binaries are not published yet; install from source with cargo install --git https://github.com/${repo}.git"
      ;;
    *)
      fail "unsupported platform: ${arch}-${os}"
      ;;
  esac
}

release_base_url() {
  if [ "$version" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download' "$repo"
  else
    printf 'https://github.com/%s/releases/download/%s' "$repo" "$version"
  fi
}

install_dir() {
  if [ -n "${LLMPK_INSTALL_DIR:-}" ]; then
    printf '%s' "$LLMPK_INSTALL_DIR"
  elif [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
    printf '/usr/local/bin'
  else
    [ -n "${HOME:-}" ] || fail "HOME is not set; set LLMPK_INSTALL_DIR"
    printf '%s/.local/bin' "$HOME"
  fi
}

verify_checksum() {
  local dir="$1"
  local checksum="$2"

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dir" && sha256sum -c "$checksum")
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$dir" && shasum -a 256 -c "$checksum")
  else
    log "sha256sum/shasum not found; skipping checksum verification"
  fi
}

need uname
need tar
need install
need mktemp

target="$(target_triple)"
asset="${bin}-${target}.tar.gz"
base_url="$(release_base_url)"
tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t llmpk)"
trap 'rm -rf "$tmp_dir"' EXIT

log "downloading ${asset}"
download "${base_url}/${asset}" "${tmp_dir}/${asset}"
download "${base_url}/${asset}.sha256" "${tmp_dir}/${asset}.sha256"
verify_checksum "$tmp_dir" "${asset}.sha256"

tar -xzf "${tmp_dir}/${asset}" -C "$tmp_dir"
[ -f "${tmp_dir}/${bin}" ] || fail "release archive did not contain ${bin}"

dest_dir="$(install_dir)"
mkdir -p "$dest_dir"
install -m 755 "${tmp_dir}/${bin}" "${dest_dir}/${bin}"

log "installed ${bin} to ${dest_dir}/${bin}"

case ":${PATH:-}:" in
  *":${dest_dir}:"*) ;;
  *) log "add ${dest_dir} to PATH if ${bin} is not found by your shell" ;;
esac
