# NAS Lighthouse

<p align="center"><img src="public/logo.svg" width="96" alt="NAS Lighthouse"></p>

把 NAS 状态、常用服务和文件搜索放在桌面上。

NAS Lighthouse is a Windows desktop companion for Linux NAS systems: live metrics over SSH, a self-hosted service launcher, and optional Everything / AList search. Built with Tauri 2, Rust, React and TypeScript.

## 功能

- **总览**：CPU、内存、网络、磁盘、温度、Linux MD RAID 和 Docker 容器。
- **服务导航**：从容器端口及本机连接发现服务，收藏、分组、TCP 连通性检查。
- **桌面悬浮窗**：贴边收起、托盘、快捷键 `Ctrl+Alt+N` 和可选开机自启。
- **搜索**：本机 Everything 与 NAS 的 AList 联合搜索；未配置 AList 时跳过 NAS 搜索。
- **网盘凭据维护**：更新已有夸克或百度存储的凭据，保留后端其他设置。每类支持一个可配置的挂载路径。

## 支持范围

| 组件 | 当前要求 |
| --- | --- |
| 桌面客户端 | Windows 10/11、WebView2、OpenSSH；macOS/Linux 客户端尚未完成验证 |
| NAS 采集 | Linux、Bash、可读的 `/proc`；GNU 风格的 `df -B1 -P`、`date +%s%N`、awk、tr |
| 可选指标 | Docker CLI 与访问权限；hwmon 温度；`/proc/mdstat` 阵列状态 |
| 网络探测 | `nc -z -w`，或 `timeout` + Bash `/dev/tcp`；工具缺失时不进行无界等待 |
| 本机搜索 | 运行中的 [Everything](https://www.voidtools.com/) 与官方 `es.exe` |
| NAS 搜索 | AList 的 HTTP API 和 Docker 部署；OpenList 提供显式兼容模式，未承诺所有版本和镜像布局 |

网卡默认跟随默认路由；磁盘自动发现为尽力而为，可在设置中指定挂载点。ZFS、FUSE、BusyBox 及厂商定制系统的完整兼容性需要实机验证。缺失指标不等于设备正常或故障。

## 从源码运行

安装 Node.js **22.12+**、Rust stable（MSVC 工具链）、Visual Studio C++ Build Tools、Windows SDK 和 WebView2。Tauri 系统依赖参见 [官方指南](https://v2.tauri.app/start/prerequisites/)。

```powershell
git clone https://github.com/teawalklam-ux/nas-lighthouse.git
cd nas-lighthouse
npm ci
npm run tauri -- dev
```

第一次打开时 SSH 别名和 NAS 地址都为空，采集不会自动连接。先在 Windows OpenSSH 配置中建立自己的 Host 条目，例如：

```sshconfig
Host my-nas
    HostName 192.0.2.10
    User nas-monitor
    IdentityFile ~/.ssh/id_ed25519
```

这里使用文档地址，请换成自己的设备。先在终端运行 `ssh my-nas`，核实主机指纹并确认密钥登录成功，再在应用设置中填写别名和地址。应用使用 `BatchMode=yes`，不接收密码，也不关闭主机密钥校验。Docker 权限是高权限能力，只在需要容器或 AList 功能时授予。

## 可选搜索与网盘

Everything 优先通过 `es.exe` 访问。可在设置中指定完整路径，或放在 Everything 安装目录、应用配置目录的 `bin/` 或 PATH。Everything HTTP 是可选后备，不需要为了应用开启对外 HTTP 服务。

启用 NAS 搜索时填写后端地址、容器名和类型：

- AList 预设执行容器内 `/opt/alist/alist admin token`。
- OpenList 预设执行容器内 `/opt/openlist/openlist admin token`。
- 管理员令牌仅缓存在内存中，按地址、SSH 别名、容器和后端隔离；连接设置变化时清空缓存。
- 地址不允许内嵌用户名、密码或查询参数，带凭据的请求不跟随重定向。HTTPS 应使用系统信任的证书，也可通过自己配置的 SSH 隧道访问后端。

在 AList / OpenList 后台先创建夸克或百度存储，再在本应用填写对应挂载路径。应用只更新已有存储的 Cookie 或 refresh token，不创建、启用或删除存储，不覆盖代理、签名、根目录等设置。更新期间避免其他客户端同时修改该存储。

**云盘凭据会传送给后端，并由后端保存。** 它们不写入本客户端配置文件。HTTP 地址以明文传输，建议使用 HTTPS 或受信任的本地隧道。云盘读写权限由后端账号及配置决定，应用不承诺把后端强制变成只读。

## 配置与隐私

Windows 配置目录为 `%APPDATA%/nas-lighthouse/`。开发和演示可用 `NAS_LIGHTHOUSE_CONFIG_DIR` 指定独立的私人目录，不要提交其内容。

新版不自动读取旧版配置或事件，也不修改旧版启动项。如需迁移，在设置页手动导入旧配置；开机自启需单独设置。旧版 WebView 的告警偏好不自动迁移。

配置备份含主机地址、导航链接和服务名称，不能直接作为公开 Issue 附件。事件文件保留最近 300 条状态变化。图标仅从服务同源地址抓取，不使用第三方图标服务、不跟随重定向，也不绕过 TLS 校验。出站探测从 NAS 连接设置中的目标，可以关闭。

更多数据流、权限和安全报告方式见 [SECURITY.md](SECURITY.md)。合成配置见 [examples/demo-config.json](examples/demo-config.json)；导入后需换成自己的连接设置。

## 构建与验证

```powershell
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
python scripts/check_repository.py
npm run tauri -- build --bundles nsis
```

普通测试不需要 NAS。写注册表的自启集成测试默认忽略，CI 在一次性 Windows runner 上单独运行。`npm run probe` 会连接你配置的 NAS，不是无网络测试。

CI 验证构建和测试，不自动发布安装包。自行分发时，请从对应源码提交重新构建，并保留第三方声明。

## 贡献与许可证

欢迎提交可复现的问题、NAS 兼容性信息和 Pull Request，参见 [CONTRIBUTING.md](CONTRIBUTING.md)。

项目原创代码采用 [MIT](LICENSE)。第三方组件保持各自许可；GSAP 和 `@gsap/react` 使用其 Standard “no charge” license，不被本项目重新许可为 MIT。bloub 吉祥物引擎来自 Jérémy Perret，详见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。
