#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
cargo build --release --locked
bin_root="$HOME/.local/bin"
data_root="${XDG_DATA_HOME:-$HOME/.local/share}"
config_root="${XDG_CONFIG_HOME:-$HOME/.config}"
install -d "$bin_root" "$data_root/applications" "$data_root/icons/hicolor/scalable/apps" "$config_root/systemd/user"
bin_temporary=$(mktemp "$bin_root/codex-micro.XXXXXX")
trap 'rm -f -- "$bin_temporary"' EXIT
install -m755 target/release/codex-micro "$bin_temporary"
mv "$bin_temporary" "$bin_root/codex-micro"
install -m644 assets/codex-micro.svg "$data_root/icons/hicolor/scalable/apps/codex-micro.svg"
cat > "$data_root/applications/codex-micro.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Codex Micro
Comment=Configure Codex Micro buttons and dial actions
Exec="$bin_root/codex-micro"
Icon=codex-micro
Terminal=false
Categories=Settings;HardwareSettings;
StartupWMClass=codex-micro
DESKTOP
if [[ -f "$config_root/systemd/user/work-louder.service" ]]; then
  systemctl --user disable --now work-louder.service
  rm -- "$config_root/systemd/user/work-louder.service"
fi
install -m644 systemd/codex-micro.service "$config_root/systemd/user/codex-micro.service"
systemctl --user daemon-reload
systemctl --user enable codex-micro.service
systemctl --user restart codex-micro.service
rm -f -- "$bin_root/work-louder" "$data_root/applications/work-louder.desktop" "$data_root/icons/hicolor/scalable/apps/work-louder.svg"
if command -v update-desktop-database >/dev/null; then update-desktop-database "$data_root/applications"; fi
printf 'Installed Codex Micro. Open it from the application launcher.\n'
