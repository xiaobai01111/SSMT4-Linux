# Proton 下载、管理与使用

最后更新：2026-03-08

本文解释 SSMT4-Linux 中 Proton/Wine 的真实工作方式，而不是只告诉你“去哪里点下载”。

如果你理解这条链路，就会知道：

- 为什么 `设置 -> Proton 管理` 和 `游戏设置 -> 运行环境` 是两层不同功能；
- 为什么有些版本能在运行环境页里选到，但不一定能在 Proton 管理页里被删除；
- 为什么切换 Proton 后，问题有时是版本问题，有时其实是 Prefix、DXVK 或启动链问题；
- 为什么某些开关只是“偏好”，实际启动时可能还会回退。

## 1. 先理解两层职责

当前项目中的 Proton 相关能力分成两层：

### 1.1 全局层：`设置 -> Proton 管理`

这一层负责：

- 扫描本机已存在的 Wine/Proton 版本；
- 获取远程可下载的 Proton/Wine 版本；
- 下载并安装 SSMT4 自己管理的 Proton/Wine；
- 维护“家族”和“来源”目录，也就是 Proton Catalog。

这层不直接决定某个游戏实际使用哪个版本。

### 1.2 单游戏层：`游戏设置 -> 运行环境`

这一层负责：

- 给当前游戏绑定实际使用的 Wine/Proton 版本；
- 维护当前游戏的 Prefix；
- 配置 `umu-run`、Pressure Vessel、Wayland、GStreamer、DXVK HUD、自定义环境变量等运行参数；
- 决定启动时如何构造运行命令和环境变量。

所以一句话概括：

- `Proton 管理` 解决“系统里有哪些版本可用”；
- `运行环境` 解决“当前游戏到底用哪个版本、怎么跑”。

## 2. SSMT4-Linux 会从哪里扫描 Proton/Wine

项目扫描本地 Wine/Proton 时，不只看自己下载的目录，而是按多来源汇总：

### 2.1 系统 Wine

- `/usr/bin/wine`
- `/usr/bin/wine64`
- 以及 `which wine` / `which wine64` 能找到的系统版本

### 2.2 Steam 官方 Proton

- Steam `steamapps/common` 下的官方 Proton
- 包括普通 Proton 和 Proton Experimental

### 2.3 `compatibilitytools.d`

项目会扫描多个 `compatibilitytools.d` 目录，包括：

- SSMT4 自己数据目录下的 `proton/`
- Steam 的 `compatibilitytools.d`
- XDG/Flatpak 相关 Steam 目录

这里通常能识别到：

- GE-Proton
- DW-Proton
- Proton-TKG
- 其他自定义兼容工具

### 2.4 Lutris Wine

- `~/.local/share/lutris/runners/wine`

### 2.5 SSMT4 自己下载的 Wine runners

- `<dataDir>/runners/wine/`

### 2.6 用户自定义搜索路径

- 来自全局设置中的 `custom_search_paths`
- 会递归扫描，但有最大深度限制，避免无限下钻或符号链接环路

这意味着：

- 运行环境页里能看到的“本地已安装版本”，可能来自系统、Steam、Lutris、SSMT4 自己的下载目录，或者你的自定义路径。
- 所以“能选到”不代表“是 SSMT4 下载并管理的版本”。

## 3. SSMT4 自己下载的 Proton/Wine 放在哪里

项目自己下载的版本，目录是分开的：

### 3.1 Proton

- 安装到 `<dataDir>/proton/`

### 3.2 Wine runners

- 安装到 `<dataDir>/runners/wine/`

这个区分很重要，因为当前下载逻辑会根据远程条目的 `variant` 决定落到哪一边：

- `GE-Proton` / `DW-Proton` 之类的 Proton，进 `proton/`
- `Wine-GE` / `Wine-Builds` 之类的 Wine runner，进 `runners/wine/`

## 4. Proton 管理页真正管理的是什么

`设置 -> Proton 管理` 并不是一个单纯的“GitHub Releases 列表页”，它管理的是一套目录系统：

### 4.1 家族（Family）

每个家族通常对应一类版本来源或一类命名规则，例如：

- GE-Proton
- DW-Proton
- Official
- Experimental
- Lutris
- System Wine
- Custom

家族有这些作用：

- 决定 UI 上如何分组显示；
- 决定扫描到的本地版本归到哪里；
- 决定远程版本该挂到哪个分类下面；
- 决定排序和展示名称。

### 4.2 来源（Source）

来源定义的是远程获取规则，通常包含：

- 使用哪个 provider
- 对应哪个仓库或 endpoint
- 资产匹配规则
- tag 匹配规则
- 取第几个资源
- 是否包含预发布
- 最多拉多少条

当前支持的 provider 至少包括：

- `github_releases`
- `forgejo_releases`
- `github_actions`

这就是为什么 Proton 管理页里会有“目录可视化维护”：

- 你不是只能被动接受固定下载源；
- 项目允许你在 UI 中直接维护 Proton Catalog。

## 5. 本地版本和远程版本不是同一个集合

这点非常容易误解。

### 5.1 本地版本列表

本地版本来自“扫描结果”，是一个广义集合，可能包含：

- 系统 Wine
- Steam 官方 Proton
- `compatibilitytools.d` 中的第三方 Proton
- Lutris runners
- SSMT4 自己下载的版本
- 自定义路径里的版本

### 5.2 远程版本列表

远程版本来自 Proton Catalog 定义的“来源”，是一个更受控的集合。

因此：

- 你可以在运行环境页选择系统 Wine 或 Steam 官方 Proton；
- 但在 `Proton 管理` 页里，只有项目识别并归入家族/来源体系的版本，才会以“远程可下载”或“本地已安装分组”的方式出现。

## 6. 下载一个 Proton 时，项目具体做了什么

当你在 `设置 -> Proton 管理` 点击下载某个版本时，流程大致是：

1. 根据条目里的 `download_url`、`tag`、`variant` 调用后端。
2. 判断这是 Proton 还是 Wine runner。
3. 选择安装目录：`<dataDir>/proton/` 或 `<dataDir>/runners/wine/`。
4. 依次尝试原始 GitHub 地址和镜像地址。
5. 流式下载到临时归档文件。
6. 做基础完整性检查：
   - 文件大小阈值
   - 归档魔数检查
7. 用 `tar` 或 `unzip` 解压到目标目录。
8. 删除临时归档。
9. 刷新本地版本缓存。

因此当前下载并不是“把压缩包下下来就算完成”，而是已经包含了：

- 镜像回退
- 流式写盘
- 归档校验
- 自动解压

## 7. 哪些本地版本允许删除

项目对“删除本地 Proton”是有边界控制的。

当前只允许删除 SSMT4 自己下载并管理的版本，也就是路径必须位于：

- `<dataDir>/proton/`
- `<dataDir>/runners/wine/`

这意味着：

- Steam 官方 Proton 不能通过这个入口删除；
- 系统 Wine 不能通过这个入口删除；
- 你在其他路径下手工放的自定义版本，默认也不能通过这个入口删除。

这样做是为了避免 UI 误删系统级或 Steam 管理的运行环境。

## 8. 单游戏是如何绑定 Proton 的

当你在 `游戏设置 -> 运行环境` 里为某个游戏选择 Wine/Proton 版本时，项目会把配置写进该游戏的 Prefix 配置。

如果 Prefix 已存在：

- 更新 `wine_version_id`
- 更新 `proton_settings`

如果 Prefix 还不存在：

- 会先创建 Prefix 目录
- 再写入对应配置

所以从数据结构上看，单游戏运行环境并不是单独漂浮在 UI 状态里，而是和 Prefix 配置绑定在一起的。

## 9. Prefix 路径是如何确定的

当前项目优先使用“游戏目录内的 prefix”：

- 如果能解析出当前游戏根目录，Prefix 会优先放在游戏目录下的 `prefix/`

并且还有一个迁移逻辑：

- 如果发现旧的 legacy prefix 存在，而新位置还没有，会尝试自动迁移

只有在无法解析游戏根目录时，项目才会回退到旧的统一 prefixes 目录。

这意味着当前设计倾向于：

- 让 Prefix 更贴近具体游戏目录；
- 而不是继续长期把所有 Prefix 混放在一个中心目录里。

## 10. 打开运行环境页时，项目会默认给你什么

`get_game_wine_config` 返回当前游戏运行环境信息时，会做几件事：

- 读取当前游戏的 Prefix 配置；
- 返回当前绑定的 `wine_version_id`；
- 返回 Prefix 路径（如果已经存在）；
- 返回 `proton_settings`；
- 某些游戏如果预设要求默认启用 `umu-run`，且旧配置里还没有这个字段，会自动给出默认值。

也就是说：

- `umu-run` 的默认值不一定完全来自前端默认状态；
- 某些游戏预设可以在首次使用时自动把它打开。

## 11. 运行环境页里这些开关，实际是什么意思

运行环境页里的 Proton 相关设置不是装饰，它们会直接影响启动环境变量和命令构造。

### 11.1 `use_umu_run`

- 表示优先使用 `umu-run` 启动
- 如果系统找不到 `umu-run`，项目会回退到 Proton 启动链，不会硬崩

### 11.2 `use_pressure_vessel`

- 表示优先尝试 Steam Linux Runtime / Pressure Vessel 容器
- 如果找不到运行时入口，项目会自动回退到直连 Proton

### 11.3 `proton_media_use_gst`

- 会设置 `PROTON_MEDIA_USE_GST=1`
- 主要影响媒体、视频、网页相关行为

### 11.4 `proton_enable_wayland`

- 会设置 `PROTON_ENABLE_WAYLAND=1`

### 11.5 `proton_no_d3d12`

- 会设置 `PROTON_NO_D3D12=1`

### 11.6 `mangohud`

- 会设置 `MANGOHUD=1`

### 11.7 `steam_deck_compat` / `steamos_compat`

- 会注入一组 Steam Deck / SteamOS 相关环境变量
- 用于兼容某些依赖这些环境标识的脚本或游戏逻辑

### 11.8 `dxvk_hud` / `dxvk_async`

- 影响 DXVK 的显示和异步编译相关环境

### 11.9 `disable_gpu_filter`

- 用于避免自动 GPU 过滤逻辑干预某些环境

### 11.10 `custom_env`

- 最终会直接并入启动环境变量
- 风险最高，也最容易引入不可预期行为

## 12. 项目实际如何决定用什么方式启动

当前启动并不是简单等于“选了某个 Proton 路径就直接跑它”。

项目内部会综合这些信息：

- 选中的 Proton/Wine 路径
- `proton_settings`
- 预设元数据
- LaunchProfile patch
- 是否存在 `umu-run`
- 是否存在 Steam Linux Runtime

然后在这些运行器之间决定实际命令：

- `umu-run`
- `Pressure Vessel`
- `Proton`
- `Wine`

并且这个决定是可以回退的：

- 开了 `umu-run` 但系统没找到 `umu-run`，回退
- 开了 Pressure Vessel 但没找到 Steam Linux Runtime，回退
- 某些强制直连逻辑触发时，也会从 `umu-run` 回退到直连 Proton

所以文档层面的正确理解是：

- 运行环境页里的设置是一组“偏好和约束”
- 实际启动命令仍要经过项目的启动解析器再决定

## 13. 为什么“换 Proton”经常不能单独解决问题

很多用户会把 Proton 当成万能旋钮，但项目的真实结构不是这样的。

一次启动至少还受这些因素影响：

- 主程序路径是否正确
- Prefix 是否干净
- DXVK / VKD3D 是否安装
- 防护状态是否满足
- 图形层是否正常
- 是否启用了 Mod、3DMigoto、Bridge、脚本、自定义环境变量

所以换 Proton 能解决的是“运行环境层的一部分问题”，不是所有问题。

## 14. 正确的排查顺序

如果你怀疑是 Proton 问题，建议按这个顺序排查：

1. 先确认主程序路径是对的。
2. 再确认当前游戏已绑定了有效的 Wine/Proton 版本。
3. 确认 Prefix 路径存在且不是混乱状态。
4. 确认 DXVK / VKD3D 安装状态。
5. 暂时关闭 3DMigoto、Mods、Bridge 和自定义环境变量。
6. 再尝试切换 Proton 版本。

如果你一开始就同时更换 Proton、DXVK、Prefix 和实验性功能，排查价值会非常低。

## 15. 推荐的使用策略

### 15.1 全局层

- 至少保留一个稳定版本和一个备用版本。
- 在 `Proton 管理` 里维护清晰的家族和来源，不要堆一堆不明来源 runner。
- 优先使用可追溯来源的版本。

### 15.2 单游戏层

- 一个游戏先在“基础启动链”跑通，再谈高级功能。
- 不同游戏尽量不要强行共用完全相同的假设。
- 记录“哪个游戏在什么 Proton 版本下可正常运行”。

### 15.3 Prefix 层

- 切换大版本前，最好保留一个可工作的 Prefix 基线。
- 出现严重异常时，先判断是版本问题还是 Prefix 污染问题。

## 16. 常见误区

- 误区一：只要 `Proton 管理` 页里下载了版本，游戏就会自动切过去。
  不是。单游戏仍需要在运行环境页绑定具体版本。

- 误区二：运行环境页里能看到的所有版本，都可以在 Proton 管理页里删除。
  不是。只有 SSMT4 自己下载并管理的版本允许从 UI 删除。

- 误区三：开了 `umu-run` 或 Pressure Vessel，就一定会按那个方式启动。
  不是。缺失依赖时项目会自动回退。

- 误区四：换一个 Proton 就等于重置了环境。
  不是。Prefix、DXVK、VKD3D、自定义环境变量仍然会持续影响结果。

- 误区五：Proton 管理页就是“单游戏运行环境页”。
  不是。一个是全局版本管理，一个是单游戏绑定与运行时配置。

## 17. 文档结论

你可以把当前项目里的 Proton 功能理解成三层系统：

1. 扫描层：从系统、Steam、compatibilitytools.d、Lutris、SSMT4 下载目录和自定义路径发现版本。
2. 管理层：通过 Proton Catalog 把本地和远程版本组织成“家族 + 来源”体系，并负责下载、安装和可控删除。
3. 运行层：把某个具体版本与某个具体游戏绑定，再结合 Prefix、环境变量和运行器策略生成最终启动命令。

所以“Proton 下载、管理与使用”的真正重点不是下载按钮本身，而是：

- 你要知道版本从哪里来；
- 哪些是项目托管的，哪些只是扫描到的；
- 哪些设置是全局管理，哪些是单游戏绑定；
- 以及这些选项在启动时究竟会如何生效和回退。

如果你还没有读过 [游戏下载与主程序配置](02-游戏下载与主程序配置.md) 和 [DXVK / VKD3D 下载、管理与使用](04-DXVK-下载管理与使用.md)，建议继续阅读，这两篇分别解释启动入口链和图形层管理。
