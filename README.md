<p align="center">
  <img src="assets/soundshare-icon.png" width="120" alt="SoundShare icon">
</p>

<h1 align="center">SoundShare</h1>

<p align="center">
  轻量、开源的 Windows TeamSpeak 共享伴奏工具
</p>

SoundShare 捕获 QQ 音乐、网易云音乐、浏览器等指定程序正在播放的声音，并以一个独立的 TeamSpeak 客户端加入频道。房间成员可以一起收听，也可以单独调节伴奏客户端的音量，而你的正常语音客户端不受影响。

项目使用 Rust 和 Windows 原生界面，不包含浏览器内核、不读取音乐账号或歌单，也不依赖点歌机器人。许可证为 [MIT](LICENSE)。

## 主要功能

- 选择一个正在发声的 Windows 程序，而不是混入麦克风或所有系统声音；
- 使用持久化独立身份连接 TeamSpeak 6；
- 48 kHz 双声道、20 ms 音频帧、96 kbps Opus Music；
- 支持默认频道或数字频道 ID；
- 自动记住最近连接成功的服务器，可从下拉框快速选择；
- 按服务器保存频道历史，并在连接后自动读取真实频道名称；
- 支持为频道填写本地备注，备注优先用于历史列表显示；
- 原生轻量 GUI，并保留完整 CLI 排错工具；
- 服务器地址、频道 ID 和显示名称保存在本机；
- 真正的单文件 EXE，已静态嵌入运行库，无需安装或额外 DLL。

```text
QQMusic.exe / cloudmusic.exe / 浏览器
                ↓ Windows 按进程回环捕获
            SoundShare
                ↓ 48 kHz 双声道、Opus Music
          TeamSpeak 服务器
                ↓
            当前语音频道
```

## 下载与使用

从 GitHub Releases 直接下载 `SoundShare.exe`，双击即可使用，不需要安装或解压：

1. 先在音乐播放器或浏览器中播放内容；
2. 点击“刷新列表”，选择有声音的程序；
3. 填写 TeamSpeak 服务器地址，或从下拉框选择以前成功连接过的服务器；
4. 频道可以留空进入默认频道、手动输入数字 ID，或从该服务器的频道历史中选择；
5. 可填写“频道备注”，例如“音乐房”；不填写时会自动使用服务器返回的真实频道名；
6. 点击“开始共享”，完成后点击“停止”。

第一次运行时，Windows 可能因为程序尚未购买代码签名证书而显示 SmartScreen 提示。请只从项目 GitHub Releases 或你信任的构建者处下载，并核对发布页提供的 SHA-256。

普通用户只需要这一个 EXE。`SoundShare-Windows-x64.zip` 是面向排错和开发者的可选包，其中额外包含命令行工具、README 与许可证。

使用耳机可以避免麦克风再次收录扬声器中的伴奏。SoundShare 是第二个独立客户端，因此原来的 TeamSpeak 客户端可以继续正常说话。

## CLI 与本地排错

发布包同时包含 `SoundShare-CLI.exe`。列出活动音频程序：

```powershell
.\SoundShare-CLI.exe list
```

记下目标程序 PID 后开始共享。省略频道 ID 时进入默认频道：

```powershell
.\SoundShare-CLI.exe share <PID> ts.example.com:9987
```

进入指定频道并设置显示名称：

```powershell
.\SoundShare-CLI.exe share <PID> ts.example.com:9987 <频道ID> "我的伴奏"
```

只测试连接而不发送声音：

```powershell
.\SoundShare-CLI.exe connect-test ts.example.com:9987
```

录制 10 秒 WAV，检查本地捕获是否正确：

```powershell
.\SoundShare-CLI.exe record <PID> 10 soundshare-capture.wav
```

## 从源码构建

开发环境需要 Windows 10/11 x64、Rust stable、Visual Studio Build Tools 的“使用 C++ 的桌面开发”、Windows SDK 和 Visual Studio CMake 组件。

```powershell
.\build.ps1 test
.\build.ps1 build -Release
```

完成后会生成：

```text
release\SoundShare.exe
release\SoundShare-CLI.exe
release\SoundShare-Windows-x64.zip
```

其中 `SoundShare.exe` 是面向普通用户的主要发布文件，可脱离本项目目录单独复制和运行。

`build.ps1` 会自动寻找 Visual Studio 自带的 CMake，并处理新版 CMake 构建旧版 Opus 时的兼容参数。首次构建需要从 crates.io 下载 Rust 依赖，之后使用本地缓存增量构建。

仓库中的 `vendor` 固定保存了经过 TS6 验证的 `tsclientlib` 和 `tsproto` 兼容代码，来源与许可证见 [vendor/README.md](vendor/README.md)。

## 本地数据与隐私

SoundShare 的数据位于：

```text
%LOCALAPPDATA%\SoundShare\identity.json
%LOCALAPPDATA%\SoundShare\settings.json
```

`identity.json` 相当于该客户端在 TeamSpeak 服务器上的长期身份，绝不能公开或提交到 Git。删除它会生成一个全新身份，原身份关联的服务器组或权限也会丢失。

服务器历史、频道 ID、真实频道名和本地备注保存在 `settings.json` 中。首次安装时服务器地址为空；项目源码和公开安装包不会预填维护者的私人服务器。历史只会在连接成功后加入，最多保存最近 8 个服务器和 32 个频道。

正常共享只在内存中处理实时音频，不会写入 WAV；只有主动使用 CLI 的 `record` 命令才会创建录音文件。程序不会读取或上传 QQ 音乐、网易云音乐等平台的账号、Cookie、歌单或登录信息。

## 网络带宽与限制

SoundShare 发送一条约 96 kbps 的上行音频流。TeamSpeak 服务器需要分别转发给每位听众，因此服务器出口会随人数近似线性增加：

```text
服务器伴奏出口 ≈ 96 kbps × 听众人数 + 协议开销
```

这不是 KOOK、OOPZ 一类平台内置的 P2P 共享伴奏，也不绕过 TeamSpeak 的频道权限。当前还存在以下限制：

- 只支持 Windows；
- 暂无自动重连、托盘、应用内音量控制和频道列表；
- 播放器必须已经创建活动音频会话，才能出现在列表中；
- 独占模式、受 DRM 保护或采用特殊音频路径的程序可能无法捕获；
- 服务器或频道必须允许 SoundShare 身份连接和发送语音；
- 未进行代码签名，发布者显示为未知。

## 常见问题

**列表中没有播放器**：先真正播放一首歌，再点“刷新列表”。一些播放器会由子进程输出声音，应选择列表中有明显音量的项目。

**进入了错误频道**：填写目标频道的数字 ID；留空会进入服务器默认频道。

**频道下拉框为什么一开始是空的**：SoundShare 必须先成功连接一次，才能从 TeamSpeak 状态中读取真实频道名。此后该服务器的频道会出现在历史下拉框中。当前版本不会在连接前浏览服务器的完整频道树。

**别人听不到**：先用 CLI 的 `record` 检查 WAV。录音正常时，再检查 TeamSpeak 中伴奏客户端是否被静音、频道 Talk Power 和客户端音量。

**声音卡顿**：检查发送电脑的上行网络、服务器出口带宽与丢包。不要同时运行多个 SoundShare 实例捕获同一个播放器。

**关掉共享后本机声音变大或变小**：SoundShare 不主动修改播放器或系统音量。Windows 通信活动自动衰减、TeamSpeak 设置或播放器音量标准化可能造成听感变化，可先在 Windows“声音 → 通信”中检查相关设置。

## 项目状态与参与

核心链路已经验证：活动会话枚举、指定进程捕获、本地 WAV、独立身份、TS6 连接、实时 Opus 发送、开始/停止以及 Windows 发布包均可用。

后续方向包括自动重连、托盘、发送音量、频道列表和更完整的长期网络测试。提交代码前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)；安全问题请参考 [SECURITY.md](SECURITY.md)。维护者上传和发布步骤见 [PUBLISHING.md](PUBLISHING.md)。

TeamSpeak 是其各自权利人的商标。本项目为独立开源项目，不隶属于 TeamSpeak Systems GmbH，也不隶属于任何音乐平台。
