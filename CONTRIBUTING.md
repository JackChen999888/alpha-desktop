# 参与开发

Alpha 是 Daozai 的衍生项目。较大的改动请先提交 Issue 说明场景，贡献按 MIT 许可提交，并保留上游和第三方版权。

```powershell
cargo +stable-x86_64-pc-windows-msvc check --locked --all-targets
cargo +stable-x86_64-pc-windows-msvc test --locked --lib pro:: -- --test-threads=1
cargo +stable-x86_64-pc-windows-msvc test --locked --lib alpha_unified_settings_routes_and_renders_all_sections -- --test-threads=1
```

测试和开发请使用独立数据目录，避免覆盖日常配置：

```powershell
$env:DESKTOP_ISLAND_DATA_DIR = Join-Path $env:TEMP 'alpha-development'
cargo +stable-x86_64-pc-windows-msvc run --locked --bin daozai -- --pro
```

`src/pro` 包含翻译、笔记、图文剪贴板和 AI 任务；`src/extras.rs` 包含设置界面；`src/dashboard.rs` 包含系统监控、额度和电量面板；`vendor` 包含必要的依赖补丁。部分 Rust crate 和内部标识保留 Daozai 名称以兼容现有实现。

CI 运行编译检查和有限的离线回归测试，不代表所有邮箱、外设、AI 客户端和 Windows 桌面环境已验收。真实账号或 GUI 检查应由维护者在独立环境中按需执行，禁止在公共 CI 中配置私人账号。

提交前检查不包含用户数据、密钥、Cookie、真实账号截图、个人路径或生成文件。截图应使用合成内容。不要将已有个人仓库的整个 Git 历史合并进公开仓库。
