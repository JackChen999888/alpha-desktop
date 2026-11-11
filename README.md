<p align="center"><img src="assets/alpha-logo.png" width="100" alt="Alpha"></p>
<h1 align="center">Alpha Desktop</h1>
<p align="center">Windows 桌面信息岛 · 音乐 · 日程 · 翻译 · 系统监控</p>

Alpha 是基于 [Daozai](https://github.com/Renly1994/xiaotang-daozai) 二次开发的 Windows 桌面工具，部分通知功能参考 [Fly Island](https://github.com/flyl1u/fly-island)。使用 Rust、egui / eframe 和 Windows 原生接口实现。

**当前为 0.1.0 开源预览版。普通用户直接下载安装包或免安装 ZIP，无需安装 Rust、Git、Visual Studio 或 Windows SDK。程序尚未进行商业代码签名。**

[部署教程](docs/DEPLOYMENT.md) · [隐私说明](docs/PRIVACY.md) · [参与开发](CONTRIBUTING.md) · [MIT 许可](LICENSE)

## 功能

| 模块 | 能力与边界 |
| --- | --- |
| 桌面岛 | 液态玻璃外观、主题色、紧凑/详细页面、圆点导航、拖动排序与尺寸调整、按应用显示规则 |
| 音乐 | 播放控制、进度、歌词、音频波形；实际能力取决于播放器兼容性 |
| 日程 | 日历、重复待办、专注、饮水提醒 |
| 每日笔记 | 当天记录、未完成任务结转、Markdown 导出 |
| 剪贴板 | 文字与图片、搜索、备注、固定/收藏；可以关闭历史记录 |
| 翻译 | 岛内输入、复制后快捷键、图片 OCR 后翻译；需要用户自己的 DeepSeek API 密钥 |
| 监控 | CPU、内存、GPU、网速、进程原生图标、趋势与颜色配置；部分指标依赖驱动 |
| 设备电量 | Windows 实际上报的电脑及外设电量；不支持通用读取所有 2.4 GHz 接收器 |
| 邮箱 | Gmail、QQ 等 IMAP；Outlook / Hotmail 使用 OAuth；Proton 使用 Bridge |
| AI 额度 | 显示服务端返回的周期/余额；需要相应账号授权，不伪造无数据的额度 |
| AI 任务 | Codex / Claude Code 生命周期事件、项目与会话区分；需用户安装并审核 Hooks |

## 下载与安装

| 适合谁 | 下载 | 使用方式 |
| --- | --- | --- |
| 普通用户（推荐） | [Windows x64 安装包](https://github.com/chennengjian/alpha-desktop/releases/download/v0.1.0/Alpha-Setup-v0.1.0-x64.exe) | 双击，按向导安装，使用桌面 Alpha 快捷方式 |
| 不想安装的用户 | [Windows x64 免安装 ZIP](https://github.com/chennengjian/alpha-desktop/releases/download/v0.1.0/Alpha-v0.1.0-windows-x64.zip) | 完整解压后双击 Alpha.exe；不要直接在压缩包中运行 |

[全部版本与 SHA-256](https://github.com/chennengjian/alpha-desktop/releases) · **[详细部署教程](docs/DEPLOYMENT.md)**

支持 Windows 10/11 x64。安装版默认放在 `%LOCALAPPDATA%\Programs\Alpha`，无需管理员权限。两种版本都默认将个人数据保存在 `%APPDATA%\Alpha`，并在本机单独配置密钥和账号。更新、卸载保留用户数据。

GitHub 自动附带的 **Source code (zip/tar.gz)** 是源码；普通用户请选择上表的安装包或 `Alpha-v…-windows-x64.zip`。

开发者可按[源码构建说明](docs/DEPLOYMENT.md#开发者从源码构建)安装 Rust 并自行编译。维护者运行 `scripts/package.ps1` 可以生成同样的安装包和 ZIP；仓库提供手动触发的 **Build Windows packages** 工作流。

## 状态与限制

- 这是独立的衍生项目，不代表上游作者或各 AI/邮箱服务商。
- Outlook 需要用户注册/提供自己的 OAuth 公共客户端 ID 并完成授权；Proton Bridge 需要相应订阅及本机 Bridge。
- AI 任务依赖真实客户端事件。没有收到事件会明确显示未接入；完整真实任务兼容性仍需按客户端版本验证。
- 已禁用继承自上游的自动更新，避免安装回 Daozai。请从本仓库 Releases 下载新版，退出 Alpha 后安装或替换程序。
- 图形驱动、显示器缩放、媒体软件和外设会影响兼容性；内存占用会随页面和缓存变化，不承诺固定上限。
- 快捷翻译不会在点击任意屏幕文字时自动触发；可选中复制后按 `Ctrl+Alt+T`。

## 隐私

仓库不包含 API 密钥、登录凭据、个人邮件、聊天记录、笔记、剪贴板历史或日常使用配置。请在自己电脑上配置账号。不要把数据目录、凭据文件或未脱敏日志提交到 Issue / Pull Request。

DeepSeek 密钥和邮箱凭据由 Windows 凭据管理器保存。使用翻译时会把输入文字发送到 DeepSeek；图片先在 Windows 本机 OCR，再发送识别文字。详见[隐私说明](docs/PRIVACY.md)。

## 开源与致谢

- [Daozai / 岛仔](https://github.com/Renly1994/xiaotang-daozai)：主体项目，MIT；原作者版权保留于 [LICENSE](LICENSE)。
- [Fly Island](https://github.com/flyl1u/fly-island)：通知相关实现参考，MIT；见 [LICENSE-Fly-Island](LICENSE-Fly-Island)。
- egui / eframe、imap 等 vendored 依赖保留各自许可证和版权声明。
- 图标及其他第三方来源见 [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt) 和资源目录中的许可证。产品名称和商标属于各自权利人。

本仓库使用经过整理的源码快照作为首次公开提交，不包含个人开发历史、真实运行数据和机器专用验证记录。

## Code signing policy

正在准备 SignPath Foundation 免费开源签名申请，尚未获批；当前 v0.1.0 下载文件仍未签名。角色、构建来源与后续签名流程见 [Code signing policy](docs/CODE_SIGNING.md)。
