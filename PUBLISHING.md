# GitHub 上传与发布清单

这份清单供仓库维护者使用。SoundShare 不会自动上传代码或创建 Release。

## 第一次上传源码

1. 在 GitHub 新建空仓库，不要额外生成 README、许可证或 `.gitignore`。
2. 推荐仓库说明：`A lightweight Windows app that shares per-application audio to a TeamSpeak channel.`
3. 推荐 Topics：`teamspeak`、`audio`、`windows`、`rust`、`wasapi`、`opus`、`music-bot`。
4. 在本目录初始化并上传：

```powershell
git init
git branch -M main
git add .
git status
git commit -m "Initial open-source release"
git remote add origin https://github.com/<你的用户名>/<仓库名>.git
git push -u origin main
```

执行 `git add` 后务必查看 `git status`，确认没有真实服务器地址、日志、测试 WAV、`identity.json`、`settings.json`、EXE 或 ZIP。`target` 与 `release` 已由 `.gitignore` 排除。

## 发布 Windows 成品

1. 构建并测试：

```powershell
.\build.ps1 test
.\build.ps1 build -Release
```

2. 分别计算单文件程序和可选压缩包的校验值：

```powershell
Get-FileHash .\release\SoundShare.exe -Algorithm SHA256
Get-FileHash .\release\SoundShare-Windows-x64.zip -Algorithm SHA256
```

3. 在 GitHub 创建例如 `v0.1.0` 的 Release。
4. 把 `release\SoundShare.exe` 作为普通用户的主下载项直接上传。
5. 可同时上传 `release\SoundShare-Windows-x64.zip`，供需要 CLI 和说明文件的高级用户使用。
6. 在 Release 说明中附上两个文件各自的 SHA-256，并明确写出“普通用户只需下载 SoundShare.exe”。
7. 不要把 `%LOCALAPPDATA%\SoundShare\identity.json` 或个人 `settings.json` 放入压缩包。

## 发布前人工确认

- 在一台未配置过 SoundShare 的 Windows 电脑上解压并启动；
- 确认 EXE、标题栏和任务栏图标正确；
- 确认未安装 Visual C++ Redistributable 的干净电脑也能直接运行单个 EXE；
- 确认服务器地址默认为空；
- 用真实音乐播放器验证开始、停止和再次开始；
- 明确说明程序未签名，避免用户误以为 SmartScreen 提示代表安装包损坏；
- 更新 `CHANGELOG.md`，将 Unreleased 内容移动到对应版本。
