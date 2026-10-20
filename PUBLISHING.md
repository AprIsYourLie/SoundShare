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

2. `build -Release` 继续使用现有 `dist` profile、文件名和 ZIP 布局；构建成功后自动生成 `release\SHA256SUMS.txt`，并验证：
   - GUI、CLI、README、LICENSE、ZIP 与校验文件存在且非空；
   - SHA256SUMS.txt 恰好包含独立 EXE 和 ZIP 两项，格式正确且实际 SHA-256 一致；
   - ZIP 根目录恰好包含 `SoundShare.exe`、`SoundShare-CLI.exe`、`README.md`、`LICENSE`，无重复、子目录或额外文件；
   - ZIP 内每个文件与打包目录中的对应文件长度、SHA-256 一致。

3. 上传前可独立复核已有产物（只验证，不重写校验值）：

```powershell
.\scripts\verify-release.ps1
# 或指定完整打包目录（含两个 EXE、README、LICENSE、ZIP 和 SHA256SUMS.txt）：
.\scripts\verify-release.ps1 -ReleaseDirectory C:\path\to\release
```

4. 完成下方人工确认后，在 GitHub 创建对应版本的 Release，确认 tag 指向已验证的提交。
5. 上传 `release\SoundShare.exe`（主下载项）、`release\SoundShare-Windows-x64.zip`（含 CLI 的完整包）和 `release\SHA256SUMS.txt`。继续明确写出“普通用户只需下载 SoundShare.exe”。ZIP 仍可作为可选下载项。
6. 上传完成后重新下载 EXE、ZIP 和 SHA256SUMS.txt，在下载目录运行下列命令，将输出与校验文件中的两行逐一对比，不要重新生成校验文件来掩盖不一致：

```powershell
Get-Content .\SHA256SUMS.txt
Get-FileHash .\SoundShare.exe -Algorithm SHA256
Get-FileHash .\SoundShare-Windows-x64.zip -Algorithm SHA256
```

7. 不要把 `%LOCALAPPDATA%\SoundShare\identity.json` 或个人 `settings.json` 放入压缩包。SHA-256 用于发现文件变化或传输损坏，不等同于代码签名或可信来源认证。

## CI 自动验证范围

Windows CI 保留原有格式检查、Clippy 和 `.\build.ps1 test`，随后运行校验脚本回归测试、`.\build.ps1 build -Release` 和独立产物复核；任一步失败都会阻止候选包上传。回归测试使用临时样本，覆盖缺失/空文件、内容损坏、非法或重复校验项，以及 ZIP 漏项、重复项和额外文件等情况，可单独运行：

```powershell
.\scripts\test-release.ps1
```

成功运行会上传以提交 SHA 命名的 Actions artifact，内含 EXE、ZIP 和 SHA256SUMS.txt。下载 Actions artifact 后先解开外层 artifact ZIP，里面的 `SoundShare-Windows-x64.zip` 才是发布包。它是待人工验收的候选产物，CI 不创建 Release、不上传 Release 附件，也不自动打 tag。选择与发布 tag 相同提交且检查通过的候选包，并对实际上传的同一份文件完成人工确认；如果重新构建，则重新验证并重新人工验收。

CI 可验证编译、打包及文件一致性，但托管 Windows runner 不能替代干净 Windows 电脑、真实播放器和 TeamSpeak 的端到端验收。

## 发布前人工确认

- 在一台未配置过 SoundShare 的 Windows 电脑上，分别验证独立 EXE 启动和完整 ZIP 解压后 GUI、CLI 启动；
- 确认 EXE、标题栏和任务栏图标正确；
- 确认未安装 Visual C++ Redistributable 的干净电脑也能直接运行单个 EXE；
- 确认服务器地址默认为空；
- 用真实音乐播放器验证应用音频捕获，确认选中播放器的声音能被捕获；
- 使用测试账号连接真实 TeamSpeak 服务器并加入目标频道，用另一个客户端确认收到声音且音质正常；
- 验证开始共享、停止共享和再次开始：停止后频道不再收到共享音频，再次开始能够恢复；
- 记录测试的提交/tag、产物 SHA-256、Windows 版本、播放器、TeamSpeak 客户端/服务器版本及结果；不要记录凭据；
- 明确说明程序未签名，避免用户误以为 SmartScreen 提示代表安装包损坏；
- 更新 `CHANGELOG.md`，将 Unreleased 内容移动到对应版本。
