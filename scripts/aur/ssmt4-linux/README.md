# ssmt4-linux AUR Metadata

这个目录保存 `ssmt4-linux` AUR 包的元数据源文件。

包含：

- `PKGBUILD`
- `.SRCINFO`（由脚本自动生成）

相关脚本：

- `scripts/aur/update-aur-metadata.sh`
- `scripts/aur/sync-to-aur-repo.sh`
- `scripts/aur/init-aur-repo.sh`
- `scripts/aur/publish-aur.sh`

本地 `makepkg`/`yay` 生成的工作目录不会进入版本控制：

- `src/`
- `pkg/`
- `ssmt4-linux/`

发布顺序通常是：

1. 先提交版本变更，并创建与 `version` 一致的 Git tag
2. 推送 release commit 和 tag 到 GitHub
3. 构建并上传 GitHub Release 里的 pacman 包产物
4. 执行 `scripts/aur/update-aur-metadata.sh`
5. 执行 `scripts/aur/publish-aur.sh`

当前 AUR `PKGBUILD` 固定使用 GitHub tag 源，不再自动切换到 Gitee 镜像。
