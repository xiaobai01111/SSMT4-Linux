# AUR Packaging Assets

`tauri bundle` currently supports Linux targets `deb`, `rpm`, and `appimage` (no `pacman` target in Tauri config schema).

To support Arch Linux / `yay`, this repo now provides AUR assets:

- `packaging/aur/ssmt4-git/PKGBUILD`
- `packaging/aur/ssmt4-git/.SRCINFO`

The `ssmt4-git` package keeps runtime hard dependencies minimal and uses `optdepends` for
environment-specific integrations (XWayland, Wine, winetricks, Vulkan loader, tray indicator).

## Usage

1. Create a dedicated AUR repo (e.g. `ssmt4-git`).
2. Copy `PKGBUILD` and `.SRCINFO` from `packaging/aur/ssmt4-git/`.
3. Push to AUR.
4. Users can then install with:

```bash
yay -S ssmt4-git
```

## Update `.SRCINFO`

When `PKGBUILD` changes, regenerate `.SRCINFO` in the AUR repo:

```bash
makepkg --printsrcinfo > .SRCINFO
```
