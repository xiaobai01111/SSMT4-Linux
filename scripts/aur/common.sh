#!/usr/bin/env bash

resolve_aur_source_dir() {
  local script_dir="$1"
  local root_dir="$2"
  local primary="$script_dir/ssmt4-linux"
  local legacy="$root_dir/aur/ssmt4-linux"

  if [[ -d "$primary" ]]; then
    printf '%s\n' "$primary"
    return 0
  fi

  if [[ -d "$legacy" ]]; then
    printf '%s\n' "$legacy"
    return 0
  fi

  printf '%s\n' "$primary"
}

ensure_aur_srcinfo() {
  local aur_dir="$1"

  if ! command -v makepkg >/dev/null 2>&1; then
    echo "错误: makepkg 未安装，无法生成 .SRCINFO" >&2
    return 1
  fi

  (
    cd "$aur_dir"
    makepkg --printsrcinfo > .SRCINFO
  )
}

require_release_tag_at_head() {
  local root_dir="$1"
  local tag_name="$2"

  if ! git -C "$root_dir" rev-parse -q --verify "refs/tags/${tag_name}" >/dev/null 2>&1; then
    echo "错误: 当前版本缺少本地 Git tag: ${tag_name}" >&2
    echo "请先提交版本变更并创建 tag，例如：" >&2
    echo "  git add version version-log package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml" >&2
    echo "  git commit -m \"release: ${tag_name}\"" >&2
    echo "  git tag ${tag_name}" >&2
    return 1
  fi

  local head_commit
  local tag_commit
  head_commit="$(git -C "$root_dir" rev-parse HEAD)"
  tag_commit="$(git -C "$root_dir" rev-list -n1 "${tag_name}")"

  if [[ "$head_commit" != "$tag_commit" ]]; then
    echo "错误: tag ${tag_name} 没有指向当前 HEAD" >&2
    echo "  HEAD: ${head_commit}" >&2
    echo "  TAG : ${tag_commit}" >&2
    echo "请确认版本提交已打到正确的 release commit 上。" >&2
    return 1
  fi
}

require_remote_release_tag() {
  local root_dir="$1"
  local tag_name="$2"

  if ! git -C "$root_dir" remote get-url origin >/dev/null 2>&1; then
    echo "错误: 未找到 origin 远程仓库，无法校验 release tag 是否已发布" >&2
    return 1
  fi

  if ! env \
    GIT_TERMINAL_PROMPT=0 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_NOSYSTEM=1 \
    git \
      -C "$root_dir" \
      -c credential.helper= \
      -c core.askPass=true \
      ls-remote --exit-code --refs origin "refs/tags/${tag_name}" >/dev/null 2>&1; then
    echo "错误: origin 上不存在 release tag: ${tag_name}" >&2
    echo "AUR 的 GitHub 源会直接 checkout 这个 tag；未推送时构建一定失败。" >&2
    echo "请先执行：" >&2
    echo "  git push origin HEAD" >&2
    echo "  git push origin ${tag_name}" >&2
    return 1
  fi
}
