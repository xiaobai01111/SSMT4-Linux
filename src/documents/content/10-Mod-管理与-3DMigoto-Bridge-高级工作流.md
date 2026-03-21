# Mod 管理与 3DMigoto / Bridge 高级工作流

最后更新：2026-03-08

这篇文档解释 SSMT4-Linux 中 `Mod 管理`、`3DMIGOTO 管理`、`Bridge` 和实际启动链之间的关系。

它不是单纯的“怎么开开关”教程，而是用来回答这些问题：

- 为什么顶栏的 `Mod 管理` 有时会消失；
- 为什么某个游戏明明开了 3DMigoto，但启动时仍然没有注入；
- 为什么我在 `Mod 管理` 里启用了 Mod，实际加载结果仍然不对；
- 为什么某些游戏里手动选择的 importer 会被系统自动校正；
- `Bridge` 到底做了什么，配置文件和输出日志又写到了哪里。

## 1. 先理解三层控制关系

当前项目里的 3DMigoto 不是“一个开关控制全部”，而是至少分成三层。

### 1.1 全局层

这一层在：

- `设置 -> 3DMIGOTO 管理`

它控制的是整套 3DMigoto 功能是否对整个应用开放。

当全局关闭时：

- 顶栏不会显示 `Mod 管理`；
- 游戏设置中不会显示 3DMigoto 相关入口；
- 即使某个游戏自己的 `config.other.migoto.enabled=true`，启动时也不会走 Bridge，也不会注入。

这意味着：

- 全局开关是最上层闸门；
- 游戏级配置不会越过这个总闸门。

### 1.2 游戏层

这一层是单游戏配置里的：

- `config.other.migoto.enabled`

它表示“该游戏是否请求启用 3DMigoto / Mod 支持”。

它的作用是：

- 决定当前游戏是否应当进入 Bridge 工作流；
- 决定 `Mod 管理` 页面里该游戏显示为“已启用”还是“未启用”。

但它不是最终结果。真正启动时，还要再和全局开关合并。

### 1.3 运行层

启动时真正使用的是：

- `migoto_enabled = 全局开关 && 游戏级开关`

只要其中任意一层为假：

- 就不会走 Bridge；
- 不会写入本轮 Bridge 配置；
- 不会进行 3DMigoto 注入。

另外，即使这两个条件都为真，如果 `Bridge` 可执行文件本身不存在，启动也会直接报错，而不是静默跳过。

## 2. 三个页面各自负责什么

很多误解都来自“把三个页面当成了同一个东西”。

### 2.1 顶栏 `Mod 管理`

这个页面负责的是：

- 选择游戏；
- 查看当前游戏有效的 `importer`；
- 查看实际生效的 `Mods` / `ShaderFixes` 目录；
- 按条目启用 / 禁用 Mod；
- 批量启用 / 禁用 Mod；
- 打开目录；
- 切换该游戏的 `config.other.migoto.enabled`。

这个页面不负责：

- 下载或部署 Bridge；
- 生成 Bridge 配置；
- 修改 `d3dx.ini` 的细项；
- 决定本轮启动一定成功注入。

它是“Mod 资产管理页”，不是“注入链调试台”。

### 2.2 `设置 -> 3DMIGOTO 管理`

这个页面负责的是高级配置。

主要包括：

- 全局开关；
- 3Dmigoto 数据路径；
- importer 目录、`Mods` 目录、`ShaderFixes` 目录、`d3dx.ini` 路径；
- importer 类型；
- Hook / Direct 注入方式；
- `process_timeout`、`enforce_rendering`、`xxmi_dll_init_delay` 等运行参数；
- `custom_launch`、`pre_launch`、`post_load`、`extra_libraries` 等高级链路。

你可以把它理解为：

- `Mod 管理` 管目录内容；
- `3DMIGOTO 管理` 管运行机制和桥接参数。

### 2.3 游戏设置页

游戏设置页里的 3DMigoto 入口只负责做轻量级联动：

- 给单游戏开关；
- 在全局禁用时隐藏；
- 把详细设置引导到 `设置 -> 3DMIGOTO 管理`。

它不是高级配置的主界面。

## 3. importer 和路径不是完全自由的

### 3.1 已知游戏会强制使用预设 importer

当前实现里，以下游戏会被预设强制映射到固定 importer：

- `WutheringWaves -> WWMI`
- `ZenlessZoneZero -> ZZMI`
- `HonkaiStarRail -> SRMI`
- `GenshinImpact / Genshin -> GIMI`
- `HonkaiImpact3rd / Honkai3rd -> HIMI`
- `ArknightsEndfield -> EFMI`

这意味着：

- 你在配置里手动写了别的 importer，启动时仍可能被按游戏预设校正；
- 日志里会记录这一点，而不是完全照抄你的配置。

对未知游戏，系统才会优先使用你手动填写的 importer；如果你什么都没填，则回退到 `WWMI`。

### 3.2 `migoto_path` 不一定就是最终 importer 目录

当前路径解析分成几层：

- `migoto_path`
- `importer_folder`
- `mod_folder`
- `shader_fixes_folder`
- `d3dx_ini_path`

默认规则大致是：

1. 先看你有没有显式填写自定义路径；
2. 没填时，再按 importer 和默认目录自动推导。

默认数据根通常在：

- `<app_data_dir>/3Dmigoto-data`

但 importer 目录不是总会变成 `<app_data_dir>/3Dmigoto-data/WWMI` 这种嵌套形式。当前实现会判断：

- 你配置的路径看起来是否已经像一个 importer 目录；
- 该 importer 子目录是否已经存在；
- 若不存在，则新部署可能直接写进当前配置的数据路径本身。

所以：

- 不要仅凭目录名字猜测最终生效路径；
- 以 `Mod 管理` 页或生成出的 `bridge-config.json` 为准。

### 3.3 `Mods` 和 `ShaderFixes` 默认是从 importer 目录继续推导

若未单独覆盖：

- `mod_folder = importer_folder / Mods`
- `shader_fixes_folder = importer_folder / ShaderFixes`
- `d3dx_ini_path = importer_folder / d3dx.ini`

这也是为什么“改了 importer 目录，其他三个路径看起来也会跟着变”。

## 4. `Mod 管理` 页真正做了什么

### 4.1 它扫描的是 `Mods` 目录条目

当前 `Mod 管理` 页的条目列表来自：

- 当前游戏有效的 `mod_folder`

扫描时：

- 只读取这个目录；
- 会忽略隐藏条目；
- 会记录条目名称、路径、大小、修改时间、类型和启用状态。

`ShaderFixes` 目录会显示路径并可直接打开，但不会像 `Mods` 一样在页面里逐条列出和逐条开关。

### 4.2 启用 / 禁用不是写配置，而是重命名

当前启用状态不是保存在数据库里，而是通过文件系统名称控制：

- 启用：保持原名；
- 禁用：在目录或文件名后追加 `.disabled`。

例如：

- `ExampleMod` 表示启用；
- `ExampleMod.disabled` 表示禁用。

这有几个直接后果：

- 关闭某个 Mod 不会删除它；
- 批量启用 / 禁用本质上是批量重命名；
- 如果目标名称发生冲突，批量切换时会出现“跳过某些条目”的情况。
- `.disabled` 只影响该包是否参与加载；只要游戏级 3DMigoto 已启用，EFMI / XXMI 本体仍会继续注入，以保留 F10/F12 等运行时控制。

### 4.3 打开目录时会尽量保证目录存在

从页面点击“打开 Mod 目录”或“打开 ShaderFixes”时，前端会先确保目录存在，再交给系统文件管理器打开。

所以：

- 目录不存在不一定是错误；
- 某些目录是首次打开时才被自动创建出来。

### 4.4 游戏级开关只是在改该游戏的请求状态

`Mod 管理` 页里的“本游戏启用 3DMigoto”只是在改：

- `config.other.migoto.enabled`

它不代表：

- 全局功能已经打开；
- Bridge 可执行文件已经存在；
- 当前目录中的 Mod 必然兼容；
- 本轮启动一定注入成功。

## 5. 启动时真正发生了什么

### 5.1 实际执行器有优先级

当前启动链的优先级是：

1. `3DMigoto Bridge`
2. `jadeite`
3. 游戏本体 exe

也就是说：

- 只要 3DMigoto 真正启用，本轮被 Proton 运行的就不再是游戏 exe，而是 `ssmt4-bridge.exe`；
- 如果没启用 3DMigoto，但 HoYoverse 游戏检测到 `jadeite.exe`，则由 `jadeite` 接管；
- 否则才是直接运行游戏主程序。

### 5.2 Bridge 会先生成本轮配置

启动前会根据当前游戏配置生成 `BridgeConfig`，然后写到：

- `<app_data_dir>/Cache/bridge/bridge-config.json`

这份配置里会包含：

- 实际 importer；
- 游戏目录和启动 exe；
- `Mods` / `ShaderFixes` / `d3dx.ini` 路径；
- Hook / Direct 注入方式；
- 脚本和额外库配置；
- Jadeite 配置；
- importer 对应的 `d3dx.ini` 默认段落。

因此：

- 你要确认“这轮到底按什么配置在跑”，最直接的证据就是这份文件；
- 不要只看 UI 当前展示值。

### 5.3 Bridge 必须和游戏运行在同一个 Proton 容器里

当前实现明确要求：

- Bridge 与游戏共用同一 Prefix；
- 共用同一 Proton 运行器和同一容器会话。

原因不是风格选择，而是技术约束：

- DLL 注入要求处于同一容器 / 会话上下文；
- 相关 Windows API 需要同一进程枚举环境；
- `Z:\...` 这类 Windows 路径转换也是按该容器内视角解析。

所以：

- Bridge 不是一个“外部旁路工具”；
- 它是启动链内部的一环。

### 5.4 启用 3DMigoto 时会自动补某些启动条件

当前实现会按 importer 自动补充必要启动参数：

- `WWMI` 默认补 `-dx11`
- `EFMI` 默认补 `-force-d3d11`

此外，在用户没有自行覆盖时，还会自动设置：

- `DXVK_ASYNC=1`

它的目的不是保证零卡顿，而是减少首次着色器编译造成的卡顿峰值。

### 5.5 特定游戏还有额外启动链修正

当前至少有两类特殊处理。

#### `ArknightsEndfield`

终末地不是简单地“直接启动 `Endfield.exe` 再注入”。

当前实现会在识别到官方启动链后：

- 用官方启动器 exe 作为启动入口；
- 把真正的注入目标进程修正为 `Endfield.exe`；
- 同时规范启动参数。

所以终末地的“启动 exe”和“注入目标 exe”可能不是同一个文件。

#### HoYoverse 游戏

若检测到已安装的 `jadeite.exe`，会把 Jadeite 配置写入 Bridge 配置，让 Bridge 通过 Jadeite 启动游戏。

这意味着：

- 3DMigoto 与 Jadeite 不是完全并列的两条链；
- 在某些游戏里，它们会组合成一条更长的启动链。

## 6. importer 默认值差异要单独理解

不同 importer 不是只换个名字，默认值也不同。

### 6.1 `WWMI`

当前默认特征包括：

- 默认强制补 `-dx11`
- `enforce_rendering=true`
- `xxmi_dll_init_delay=500`
- 默认允许使用 Hook 注入

这说明 WWMI 的默认配置更偏向“积极接管渲染链”。

### 6.2 `EFMI`

当前默认特征包括：

- 默认强制补 `-force-d3d11`
- `process_timeout=60`
- `use_hook=false`
- `enforce_rendering=false`
- `xxmi_dll_init_delay=0`
- 启动时会自动尝试修正旧式 `RabbitFX / SetTextures` 写法的 EFMI Mod；如需禁用，可显式设置 `SSMT4_ENABLE_EFMI_COMPAT_REWRITE=0`

并且 EFMI 还带有一层旧默认值纠偏逻辑：

- 旧配置里遗留的 `30 / true / 500` 不一定继续按原值使用；
- 生成 Bridge 配置时会自动修正到 EFMI 当前默认语义。

这也是为什么：

- 你在老版本里保存过的设置，升级后看到的实际行为可能发生变化；
- 文档和最终 `bridge-config.json` 必须一起看。

## 7. `设置 -> 3DMIGOTO 管理` 里的高级项如何理解

### 7.1 路径层

这类配置决定 Bridge 读什么、从哪里读：

- `migoto_path`
- `importer_folder`
- `mod_folder`
- `shader_fixes_folder`
- `d3dx_ini_path`
- `bridge_exe_path`

路径层问题常见表现是：

- 目录看起来没错，但桥接时根本没读到预期文件；
- 你改了一个路径，结果另外三个自动推导路径全部跟着变；
- 实际加载的是另一套 importer 目录。

### 7.2 注入层

这类配置决定注入策略：

- `importer`
- `use_hook`
- `process_start_method`
- `process_priority`
- `process_timeout`
- `xxmi_dll_init_delay`
- `enforce_rendering`

如果这部分配错，常见结果不是“完全没有反应”，而是：

- 游戏正常起了，但没有生效；
- 进程能起但窗口长期黑屏；
- 注入时机不对，导致首屏卡死或直接退出。

### 7.3 调试层

这类配置决定日志和调试噪音：

- `mute_warnings`
- `calls_logging`
- `debug_logging`
- `enable_hunting`
- `dump_shaders`

默认值通常偏保守，原因很简单：

- 打开这些选项不一定让兼容性更好；
- 但更容易带来额外性能开销、磁盘写入和日志噪音。

### 7.4 扩展链路

这类配置会让启动链进一步复杂化：

- `custom_launch_enabled`
- `custom_launch_cmd`
- `custom_launch_inject_mode`
- `pre_launch_*`
- `post_load_*`
- `extra_libraries_*`

这些能力很强，但也最容易把问题从“游戏兼容性问题”变成“你自己的启动编排问题”。

推荐做法是：

1. 先在最小配置下确认基础注入可用；
2. 再逐项增加扩展能力；
3. 每加一层，都重新验证一次 `bridge-config.json` 和实际日志。

## 8. 推荐的高级工作流

如果你要长期维护一套 3DMigoto / Mod 环境，建议按下面顺序做。

### 8.1 建立最小可用基线

1. 先只开全局开关，不改高级项。
2. 只对单个目标游戏开启 `config.other.migoto.enabled`。
3. 确认 `Bridge` 可执行文件存在。
4. 启动一次，确认 `bridge-config.json` 能正常生成。
5. 查看 `bridge-output.log`，确认没有基础报错。

### 8.2 再验证目录和 importer

1. 打开 `Mod 管理`，确认 importer 是否符合该游戏预设。
2. 确认 `Mods`、`ShaderFixes`、`d3dx.ini` 的路径是你预期的那一套。
3. 保持空 Mod 或只放一个最小 Mod 先跑通。

### 8.3 再逐层叠加内容

推荐顺序：

1. 先加普通 Mod；
2. 再加 `ShaderFixes`；
3. 再调整 `d3dx.ini`；
4. 最后才考虑 `custom_launch`、额外库、脚本钩子和调试开关。

不要一开始就把这些全部打开，否则你很难判断失败来自哪一层。

### 8.4 一次只改一个变量

例如一次只改：

- 一个 Mod；
- 一个 importer 参数；
- 一个路径覆盖；
- 一个 Hook / Direct 策略。

如果你同时改五个变量，日志就失去定位价值了。

## 9. 常见误区

### 9.1 “顶栏出现了 `Mod 管理`，说明这游戏一定会注入成功”

不对。

顶栏能显示只说明：

- 全局 3DMigoto 功能已开启。

它不代表：

- 某个具体游戏已经开启；
- Bridge 可执行文件存在；
- 本轮启动链没有被别的配置破坏。

### 9.2 “我在 `Mod 管理` 里启用了 Mod，说明已经被加载”

不对。

它只说明该条目从 `.disabled` 状态恢复成了正常名称。最终是否实际被加载，还取决于：

- 本轮是否真的走了 Bridge；
- importer 和路径是否正确；
- Mod 本身是否兼容当前游戏与当前图形层。

### 9.3 “我改了 importer，启动就一定按这个 importer 来”

不对。

对已知游戏，启动器会按预设强制校正 importer。

### 9.4 “没有 `d3d11_log.txt` 就说明 3DMigoto 完全没工作”

不一定。

这还要看：

- 对应 importer / `d3dx.ini` 是否开启了相关日志；
- `calls_logging`、`debug_logging` 是否启用；
- 当前版本是否把日志输出到了别的路径或只保留 Bridge 侧日志。

### 9.5 “全局关闭 3DMigoto 会把我的 Mod 配置删除掉”

不对。

全局关闭只是不让这些功能参与启动链：

- 目录仍在；
- 游戏级配置仍在；
- 只是本轮不会桥接和注入。

## 10. 出问题时先看哪些证据

排查 3DMigoto / Bridge 问题时，建议按这个顺序看。

### 10.1 先看本轮配置是否真的落盘

先确认是否生成了：

- `<app_data_dir>/Cache/bridge/bridge-config.json`

如果没有，通常说明问题还停留在“启动前判定阶段”，例如：

- 全局没开；
- 游戏级没开；
- Bridge 文件不存在；
- 启动链根本没有进入 3DMigoto 分支。

### 10.2 再看 Bridge 输出

然后看：

- `<app_data_dir>/Cache/bridge/bridge-output.log`

它是当前项目里判断 Bridge 是否真的起跑的直接证据之一。

### 10.3 再看游戏日志窗口和目标游戏日志

如果 Bridge 已经起跑，再去结合：

- 游戏日志窗口
- `Player.log`
- 启动器日志

判断是：

- Bridge 没部署成功；
- 游戏起了但没注入；
- 注入了但 Mod 不兼容；
- 还是图形层本身出了问题。

## 11. 推荐交叉阅读

- [《服务条款》与风险声明](00-服务条款与风险声明.md)
- [项目风险与要求](01-项目风险与要求.md)
- [Prefix 与模板管理](08-Prefix-与模板管理.md)
- [Proton 下载、管理与使用](03-Proton-下载管理与使用.md)
- [DXVK / VKD3D 下载、管理与使用](04-DXVK-下载管理与使用.md)
- [防护与防封禁管理](05-防护与防封禁管理.md)
- [日志分析与标准排查流程](07-日志分析与标准排查流程.md)

## 12. 一句话总结

`Mod 管理` 管的是目录和条目状态，`3DMIGOTO 管理` 管的是桥接与注入参数，真正决定本轮是否加载的是“全局开关 + 游戏级开关 + Bridge 启动链 + importer/路径是否正确”这四层共同结果。
