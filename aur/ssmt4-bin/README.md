# ssmt4-bin AUR Packaging

This directory contains AUR metadata for `ssmt4-bin`.

## Runtime dependencies

- `gtk3`
- `webkit2gtk-4.1`
- `libsoup3`
- `xdg-utils`

Optional dependencies:

- `xorg-xwayland` (XWayland support)
- `wine` (Windows game compatibility)
- `winetricks` (Wine helper scripts)
- `libayatana-appindicator` (tray icon support)

## Refresh .SRCINFO

```bash
cd aur/ssmt4-bin
makepkg --printsrcinfo > .SRCINFO
```

## Publish to AUR

1. Upload the release asset first:
`ssmt4-bin-${pkgver}-${pkgrel}-x86_64.pkg.tar.zst`
2. Push `PKGBUILD` + `.SRCINFO` to your AUR repo.

