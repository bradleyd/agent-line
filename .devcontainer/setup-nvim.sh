#!/usr/bin/env bash
# .devcontainer/setup-nvim.sh
set -euo pipefail

NVIM_VERSION="v0.12.3"
ARCH="$(uname -m)"
case "$ARCH" in
x86_64) PKG="nvim-linux-x86_64" ;;
aarch64) PKG="nvim-linux-arm64" ;;
*)
  echo "unsupported arch: $ARCH" >&2
  exit 1
  ;;
esac

curl -fsSL "https://github.com/neovim/neovim-releases/releases/download/${NVIM_VERSION}/${PKG}.tar.gz" |
  sudo tar xzf - -C /opt
sudo ln -sf "/opt/${PKG}/bin/nvim" /usr/local/bin/nvim

# LazyVim deps that the Rust/extras stack actually wants
sudo apt-get update && sudo apt-get install -y ripgrep fd-find unzip
# fd ships as fdfind on Debian/Ubuntu; LazyVim looks for `fd`
sudo ln -sf "$(which fdfind)" /usr/local/bin/fd 2>/dev/null || true

# Vanilla LazyVim starter
git clone https://github.com/LazyVim/starter ~/.config/nvim
rm -rf ~/.config/nvim/.git

# Enable the Rust extra (this is what :LazyExtras would toggle)
mkdir -p ~/.config/nvim/lua/plugins
cat >~/.config/nvim/lua/plugins/extras.lua <<'EOF'
return {
  { import = "lazyvim.plugins.extras.lang.rust" },
  { import = "lazyvim.plugins.extras.lang.toml" },
}
EOF

# jj -> escape in insert mode
cat >~/.config/nvim/lua/plugins/keymaps.lua <<'EOF'
return {
  {
    "LazyVim/LazyVim",
    keys = {
      { "jj", "<Esc>", mode = "i", desc = "Escape insert mode" },
    },
  },
}
EOF

# Install everything headless so first launch is ready
nvim --headless "+Lazy! sync" +qa 2>&1 | tail -5 || true
nvim --headless "+MasonInstall codelldb" +qa 2>&1 | tail -5 || true
