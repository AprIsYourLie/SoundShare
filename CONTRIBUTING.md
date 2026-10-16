# 参与 SoundShare 开发

感谢你愿意改进 SoundShare。提交问题前，请先搜索是否已有相同报告；提交代码前，建议先用 Issue 说明较大的功能改动。

## 开发环境

- Windows 10/11 x64；
- Rust stable（MSVC 工具链）；
- Visual Studio Build Tools 的“使用 C++ 的桌面开发”；
- Windows SDK 与 Visual Studio CMake 组件。

构建脚本会寻找 Visual Studio 自带的 CMake，并为旧版 Opus 构建配置兼容参数：

```powershell
.\build.ps1 test
.\build.ps1 build -Release
```

## 提交要求

- 保持原生、轻量、单文件发布，不引入浏览器内核；
- 不提交 `%LOCALAPPDATA%\SoundShare` 中的身份或设置；
- 不在源码、截图和日志中写入真实服务器地址、密码或 Token；
- 涉及音频捕获、重采样或分帧时，请补充相应测试；
- 提交前运行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 和 `build.ps1 test`。

本项目采用 MIT 许可证。提交代码即表示你同意按该许可证发布你的贡献。
