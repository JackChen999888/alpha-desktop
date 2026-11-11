# Alpha Desktop 部署教程

适用版本：0.1.0 开源预览版。项目地址：https://github.com/chennengjian/alpha-desktop

## 1. 安装前准备

支持 Windows 10/11 64 位。建议先在 Windows 11 上使用；并非所有外观、媒体和硬件接口都能在每台电脑上完整工作。

安装以下工具：

1. [Git for Windows](https://git-scm.com/download/win)。
2. [Rust](https://rustup.rs/)：选择 MSVC stable 工具链。
3. [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)：勾选“使用 C++ 的桌面开发”，安装 MSVC x64/x86 工具和 Windows 10/11 SDK。

完成后重新打开 PowerShell，检查 `git --version`、`rustup --version`。若报 `link.exe` 或 `rc.exe` 缺失，先补装 C++ 工具链与 SDK。

## 2. 获取源码并编译

在你准备存放源码的目录打开 PowerShell：

```powershell
git clone https://github.com/chennengjian/alpha-desktop.git
cd alpha-desktop
rustup toolchain install stable-x86_64-pc-windows-msvc --profile minimal
cargo +stable-x86_64-pc-windows-msvc build --locked --release --bin daozai --bin desktop-island-event
```

首次编译需要下载依赖，耗时和磁盘空间取决于机器与网络。源码中的包名保留为 `daozai`，因此构建得到 `target\release\daozai.exe`；安装脚本会将应用命名为 `Alpha.exe`。

如果设置了 `CARGO_TARGET_DIR` 或显式 `--target`，请把安装脚本的 `-BuildDirectory` 指向对应的 `release` 目录。

## 3. 安装与启动

默认安装，无需管理员权限：

```powershell
powershell -NoProfile -File .\scripts\install.ps1
```

也可以指定安装位置：

```powershell
powershell -NoProfile -File .\scripts\install.ps1 -Destination 'D:\Apps\Alpha'
```

脚本复制程序和必要文件，校验 SHA-256，创建桌面/开始菜单快捷方式；已有程序文件会先备份。若目标目录中的 Alpha 正在运行，脚本会提示先从托盘退出。

双击桌面 **Alpha** 打开设置。关闭设置窗口不会退出桌面岛；完全退出请使用托盘菜单。开机启动由用户在“常规”中自行开启。

若系统阻止执行脚本，可先阅读 `scripts/install.ps1`，或手动把 `daozai.exe` 和 `desktop-island-event.exe` 复制到同一目录，将前者重命名为 `Alpha.exe`，再运行 `Alpha.exe --pro`。不要覆盖正在使用的文件，也不要关闭系统安全软件。

## 4. 数据保存在哪里

| 内容 | 位置 |
| --- | --- |
| 默认程序目录 | `%LOCALAPPDATA%\Programs\Alpha` |
| 默认用户数据 | `%APPDATA%\Alpha` |
| DeepSeek / 邮箱等凭据 | Windows 凭据管理器（部分独立凭据使用当前用户 DPAPI） |
| 桌面快捷方式 | Windows 当前用户的桌面目录 |

自定义数据目录时，可设置当前 PowerShell 会话的环境变量再启动程序：

```powershell
$env:DESKTOP_ISLAND_DATA_DIR = 'D:\AlphaData'
& "$env:LOCALAPPDATA\Programs\Alpha\Alpha.exe" --pro
```

此设置只影响从当前会话启动的进程；从桌面快捷方式启动会使用默认目录。若需要持续使用自定义目录，请在 Windows 用户环境变量中设置同名变量，并重新登录。旧版 `DAOZAI_DATA_DIR` 仍可兼容，但优先使用 `DESKTOP_ISLAND_DATA_DIR`。

迁移数据时先退出 Alpha，备份并复制整个数据目录。Windows 凭据管理器中的密钥不会随目录自动迁移到其他电脑，需要在新电脑重新配置。

## 5. 配置 DeepSeek 翻译

1. 在 DeepSeek 官方平台创建自己的 API 密钥并确认余额。
2. 打开 Alpha → **AI 任务与密钥 → DeepSeek 密钥**。
3. 粘贴密钥，点击“保存密钥”或“保存并测试”，确认显示已保存和连接测试结果。输入框保存后清空属于正常行为。
4. 在岛内翻译栏输入文字，或者选中其他应用文字并复制后按 `Ctrl+Alt+T`。
5. 粘贴图片时由 Windows OCR 识别文字，再把识别结果发送给 DeepSeek 翻译。缺少语言识别能力时，请在 Windows 设置中安装对应语言的 OCR 组件。

**不要把真实密钥写进源码、README、截图或提交到 GitHub。** 使用 API 的费用由你的 DeepSeek 账号承担。项目没有内置可共享的密钥。

## 6. 邮箱和日历

- **QQ / 部分 IMAP 邮箱**：在邮箱官方设置中开启 IMAP，使用服务商提供的授权码或应用密码。
- **Gmail**：按照 Google 账号策略开启相应访问方式；需要时使用应用专用密码，普通登录密码可能无效。
- **Outlook / Hotmail**：采用 OAuth。需要自己的 Microsoft 应用客户端 ID，在 Alpha 提供的界面按照提示完成授权；不能简单用 QQ 式授权码替代。
- **Proton Mail**：先安装并登录官方 Proton Mail Bridge，复制 Bridge 显示的本地 IMAP 地址、端口、用户名和密码；Bridge 通常需要付费计划。
- **日历**：本机日程可直接使用，远程服务需要按供应商要求配置。

不要把邮箱授权码、OAuth Token 或完整邮箱配置提交到仓库。不同供应商、组织策略和账号套餐可能限制连接；本仓库不承诺所有服务已完成真实账号验收。

## 7. AI 额度与任务状态

额度页面读取已配置服务返回的真实周期、余额或限额；未授权、接口拒绝或未返回数据时会显示提示。不会因为界面有某个服务名称就代表已连接。

Codex / Claude Code 任务状态需要独立配置：

1. 先安装对应客户端，并确保 `desktop-island-event.exe` 与 Alpha 位于同一安装目录。
2. 在 **AI 任务与密钥 → AI 任务** 点击“安装 / 修复状态接入”。此操作会备份并合并客户端 Hook 配置。
3. 在 Codex / Claude Code 的客户端设置中审核命令、完成信任，再开启新会话运行任务。
4. 查看 Alpha 是否收到真实事件。没有收到事件就按提示检查客户端版本、路径和信任状态；不要把演示数据当成真实运行。

只记录会话、项目路径、状态和时间，不记录任务正文。完整生命周期仍需按你使用的客户端版本验证。Bot 数据接入与 AI 建议已移除。

## 8. 更新、备份与卸载

更新前从托盘退出 Alpha，并备份数据目录。在源码目录执行：

```powershell
git pull --ff-only
cargo +stable-x86_64-pc-windows-msvc build --locked --release --bin daozai --bin desktop-island-event
powershell -NoProfile -File .\scripts\install.ps1
```

自定义安装位置时再次传入相同的 `-Destination`。脚本只更新程序文件，不覆盖数据目录；旧程序在安装目录的 `backups` 中。

卸载时先在 Alpha 里关闭开机启动，再从托盘退出，移除安装目录和快捷方式。用户数据和 Windows 凭据需要由你自行决定是否删除。如果曾安装 Hooks，先在对应客户端移除指向 Alpha 的命令，保留其他 Hook。

## 9. 常见问题

- **编译找不到 linker / Windows SDK**：补装 Visual Studio C++ Build Tools，重新打开终端。
- **桌面只有字母/旧图标**：确保快捷方式目标是已安装的 Alpha.exe，图标指向同目录的 Alpha.ico。
- **电量为未知**：设备或驱动没有公开上报，尤其是 2.4 GHz 接收器，不能推算成真实电量。
- **任务状态为空**：先审核并安装 Hooks，再运行一个新任务；本项目不会扫描进程来猜测 AI 状态。
- **内存会上升**：展开页面会加载缓存；收起且空闲时会按冷却间隔回收可重建缓存，不能保证固定内存值。
- **出现 Windows 安全提示**：当前源码自行编译的应用未经过商业代码签名。核对源码和来源，按自己的判断处理，勿关闭系统安全防护。

反馈问题前先去除邮箱、密钥、真实路径和私人内容。请勿上传整个数据目录。
