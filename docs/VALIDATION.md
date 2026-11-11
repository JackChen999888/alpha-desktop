# 发布前验证

日期：2026-11-11。平台：Windows x64 / Rust MSVC stable。

- `cargo check --locked --all-targets`：通过。
- `cargo test --locked --lib pro:: -- --test-threads=1`：20 项通过，3 项按需测试忽略。
- 设置路由回归：1 项通过，覆盖已删除板块和旧页面编号重定向。
- `cargo build --locked --release --bin daozai --bin desktop-island-event`：通过。
- PowerShell 安装脚本语法检查：通过。
- 在独立临时目录执行首次安装和重复安装（不创建快捷方式、不启动应用）：文件哈希校验和旧文件备份通过。
- 公开文件检查未发现真实凭据或个人目录；源码中的合成凭据测试值已经核对。

CI 执行编译检查、上述模块/设置测试和脚本语法检查。没有把私人账号、运行数据或 GUI 会话上传到 CI。

以上结果不代表所有邮箱、外设、媒体播放器或 AI 客户端已经验收。真实账号验证、Hook 信任及设备兼容性由使用者按自己的环境完成。开源版未在此次验证中安装到维护者的日常程序目录。
