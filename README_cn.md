<p align="center">
  <a href="https://github.com/yuc1013/rsdish-cli" target="_blank"><img src="assets/parfait_gpt.png" width="100" /></a>
  <h1 align="center">rsdish</h1>
  <div align="center">
    <a href="LICENSE" target="_blank">
      <img alt="GNU General Public License v3.0" src="https://img.shields.io/badge/License-GPLv3-blue.svg">
    </a>
  </div>
  <div align="center">针对家用存储的多功能同步工具。</div>
</p>

针对家用存储的多功能同步工具。

## 亮点
- ✅ 备份私人数据，一次设置，永久同步；
- 🛡️ 针对可能离线的家用硬盘设计；
- 🔗 通过符号链接整合散落在各种介质上的数据；
- 🖥️ Linux, Windows, MacOS支持；
- 🌐 可选的本地网页界面 —— 在浏览器里扫描、同步、编辑 cabinet；

## 安装

将 `rsdish` 添加到 `PATH`；

## 原理
[<img src="assets/how_it_works.png" width="40%" alt="How_it_works">](#)

## Web UI（网页界面）

同一个库也自带一个本地网页界面，扫描、组操作和 cabinet 编辑都可以在浏览器里完成：

```bash
rsdish web                        # http://127.0.0.1:8787
rsdish web --port 9000 --open     # 指定端口并自动打开浏览器

# 也可以直接使用独立二进制
rsdish-webui --host 127.0.0.1 --port 9000
```

前端资源已编译进二进制：不需要 node，不需要构建步骤，也不需要额外安装文件。

| 页面 | 作用 |
| --- | --- |
| Dashboard | 展示扫描到的存储、其中的 cabinet 与 membership，以及扫描问题 |
| Groups | 每个组的成员，支持 预览 / sync / link / clean / exec |
| Cabinets | 初始化配置、加入组、编辑 membership、应用 main/mirror/gate 预设 |
| Settings | 配置文件路径与 custom_storages 列表 |
| Logs | 实时查看与 CLI 相同的事件日志 |

所有破坏性操作都会先给出只读预览：每个成员将要复制、链接或删除的文件，
确认之后才真正操作磁盘。预览与 CLI 使用同一套函数，二者不会出现行为分歧。

服务默认只监听 `127.0.0.1`，且没有鉴权：请保持在环回地址上使用，
如需对外暴露请在前面加一层反向代理。

## 配置

```toml
# rsdish.config.toml

# macOS: ~/Library/Application Support/<app>/<config_name>.toml
# Linux: ~/.config/<app>/<config_name>.toml
# Windows: %APPDATA%\<app>\<config_name>.toml

# Tip: Run `rsdish config` to print current config path

custom_storages = ["<STG_ABS_PATH>(s)"]
```

```toml
# rsdish.cabinet.toml

# For example:
# Storage_SSD/
# ├── Cabinet_Book/
# │   ├── book1.epub
# │   ├── book2.pdf
# │   ├── .srcignore
# │   └── rsdish.cabinet.toml
# └── Cabinet_Movie/
#     ├── movie1.mp4
#     └── rsdish.cabinet.toml

# Tip: Run `rsdish cabinet init` to generate an empty config file;
# Run `rsdish cabinet join` to generate a random membership.

[[memberships]]
group_uuid = "0199ebad-44ad-78a2-baad-c56a052e33ac"
priority = 0   # Higher number = higher priority (higher can override lower)

[memberships.src_option]
enable = false

[memberships.dst_option]
enable = false
cover_level = 0  # Enum: 0=DontCover, 1=HigherCover
save_level  = 0  # Enum: 0=DontSave, 1=SaveHigher, 2=SaveHigherEqual, 3=SaveAll

[memberships.link_option]
enable = false
save_level = 0
```

```ignore
# .srcignore
# The syntax of .srcignore is largely the same as that of .gitignore.
```

## 注意

⚠️ Windows平台下， `rsdish link` 需要管理员权限，或者在Win10中开启开发者模式才能正常运行。

## License

This project is licensed under the [GNU General Public License v3.0 (GPLv3)](LICENSE).
