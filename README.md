<p align="center">
  <img src="assets/soundshare-icon.png" width="120" alt="SoundShare icon">
</p>

<h1 align="center">SoundShare</h1>

<p align="center">
  轻量、开源的 Windows TeamSpeak 共享伴奏工具
</p>

将 QQ 音乐、网易云音乐、浏览器等指定应用的声音分享至 TeamSpeak 频道，让房间成员一起收听，同时保留你原来的语音客户端。

## 主要功能

- 捕获指定应用的声音，不混入麦克风或其他应用；
- 以独立身份连接 TeamSpeak 6，听众可单独调节伴奏客户端的音量；
- 支持默认频道、数字频道 ID，以及服务器和频道历史；
- 连接后读取真实频道名，支持本地频道备注；
- Rust + Windows 原生界面。

## 运行原理

SoundShare 捕获你选定应用正在播放的声音，通过一个独立的 TeamSpeak 客户端发送到频道，再由服务器转发给房间成员。你原来的 TeamSpeak 客户端可以继续正常说话，听众也能单独调节伴奏音量。共享过程不保存录音，也不读取音乐账号或歌单。

## 下载与使用

从 [Releases](https://github.com/AprIsYourLie/SoundShare/releases) 下载 `SoundShare.exe`，双击运行。

1. 在播放器中播放内容，点击“刷新列表”并选择音频来源；
2. 填写 TeamSpeak 服务器地址，首次使用时地址为空；
3. 频道留空进入默认频道，也可输入数字 ID 或选择历史频道；
4. 按需填写频道备注和显示名称，点击“开始共享”，结束时点击“停止”。

共享期间连接输入栏锁定并显示灰底。建议使用耳机，避免麦克风重复收录伴奏。

## 常见问题

**列表中没有播放器？** 先播放声音再刷新；部分播放器由子进程输出声音，应选择有音量的项目。独占模式或受 DRM 保护的音频可能无法捕获。

**频道历史为什么是空的？** 成功连接后才会记录频道并读取真实名称，目前不支持浏览完整频道树。本地备注优先用于历史列表显示。

**别人听不到或声音卡顿？** 检查伴奏客户端是否被静音、频道发言权限和客户端音量；卡顿时检查网络及服务器带宽。构建产物中的 `SoundShare-CLI.exe --help` 提供录音和连接排错命令。

**设置和身份存在哪里？** 位于 `%LOCALAPPDATA%\SoundShare`。`settings.json` 保存设置与历史；`identity.json` 是长期身份，请勿公开或提交到 Git，删除后会丢失原身份关联的权限。

**Windows 提示未知发布者？** 程序尚未代码签名，可能触发 SmartScreen，请只运行可信来源的构建。

开源许可：[MIT](LICENSE) · 第三方协议库：[来源与许可证](vendor/README.md) · [参与开发](CONTRIBUTING.md) · [安全反馈](SECURITY.md)
