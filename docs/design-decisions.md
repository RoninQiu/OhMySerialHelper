# 关键设计决策记录

> 本文档记录 OhMySerial 开发过程中的关键设计决策与教训。从 AI 会话记忆文件移出以保持其精简。
> 每次有新决策时在此追加，不做裁剪。

---

1. **RingBuffer Arc<Mutex<>> 跨线程共享** + AtomicBool stop_flag

2. **分级错误处理** 防止 CH340 短接松动误判断线

3. **乐观更新 + 失败回滚**：sendData 先增 txBytes，invoke 失败再减

4. **try_lock + 2ms 重试** 避免 reader/poller/precise 三线程死锁

5. **Tauri 2.x capabilities 必须显式声明**，否则 listen 静默失败

6. **Zustand persist version 字段 + migrate** 兼容旧用户 localStorage

7. **bench 用 #[cfg(feature = "bench")] gate 的 reset_for_bench()**，避免污染公开 API

8. **主题语义集中在 DARK_CLASSES / LIGHT_CLASSES**：组件用 `${t.bg.secondary}` 而非 `bg-gray-800`，便于切肤

9. **text.inverse 仅限品牌色按钮**：select / textarea 文字必须用 `t.text.primary`，否则白字叠白底（v0.4.1 修复的坑）

10. **图标生成走 PIL 不走 SVG 转换器**：`docs/gen-icons.py` 用 Pillow 直接画矢量风格图标，零外部依赖（避免 cairosvg/rsvg-convert 安装门槛）

11. **rAF 节流 + selector 订阅（v0.6.0）**：60Hz 数据流不拖垮 React 渲染 — StatusBar 显值节流到 15Hz，useConfigSync 只在字段真正变化时同步

12. **Tauri Channel 零拷贝（v0.6.0）**：`Channel<Vec<u8>>` 替代 `app.emit("serial-data", Vec<u8>)`，跨进程 IPC 不再走 JSON 序列化；reader 线程 clone 一份 channel 持有，close 时自然 drop

13. **chunked memcpy（v0.6.0）**：RingBuffer 写读用 `copy_from_slice` 替代字节循环，环形 wrap 用 ≤2 段连续拷贝，~600× 提升

14. **每行方向 + 时间戳（v0.6.0）**：Xterm ANSI 256 色着色（RX 蓝/TX 绿），HEX 紧凑格式 `AA CC 12 34`；每 emit 一行（16ms 节奏），用户视觉上能区分 TX/RX 流

15. **配置原子写（v0.5.0）**：写 `config.json.tmp` 再 `rename` 替换，避免半写文件导致下次启动 panic

16. **自动重连复用同 channel（v0.5.0）**：`schedule_reconnect` 把持有的 `Channel<Vec<u8>>` 透传给新 spawn 的 reader 线程，重连成功后数据流不中断

17. **VID/PID 单一来源（v1.0.1）**：rust 端 `chip_from_vid_pid(vid, pid)` 查表，前端下拉拼 `chip + manufacturer`（形如 `COM3 (CH340 · wch.cn)`），识别不到时空串 → UI 不显示后缀，告别 `(Unknown)`

18. **UI 版本号单一来源（v1.0.2）**：`src/utils/version.ts` 编译时 import `package.json.version`（Vite 内联常量，零运行时开销）；三处 version 字段（Cargo.toml / tauri.conf.json / package.json）由 `scripts/release.sh` 同步，杜绝"binary 1.0.1 / UI v0.4.0"撕裂

19. **发布流程自动化（v1.0.2）**：`./scripts/release.sh <version> --build` 一键 8 步；AI-config 文件（`.agents/` / `scripts/`）不入 Git（`.gitignore` 屏蔽）；GitHub asset 上传走 `curl` 直传 uploads API（`gh release upload` 在 Windows 上偶尔 hang）

20. **字号只影响终端不动 <html>（v1.1.1）**：用户明确诉求"字号调节只改变终端内的字符大小，不改变原有 UI 结构大小布局"——v1.1.0 初版的 `useFontSize` / `applyFontSizeSync` 改 `<html>` fontSize 让所有 Tailwind rem 联动，**与用户期望相反**。撤掉这两个机制后字号只走 `xterm.options.fontSize`，UI 永远保持 Tailwind 默认 14px。

21. **xterm canvas 撑大 flex 子项（v1.1.1）**：xterm 内部 `.xterm-screen` 的 canvas 用 inline `width`/`height` 属性设置 `cols × cellWidth` 实际尺寸，会**撑大**所有 flex 祖先（Terminal 根 div → Terminal 父容器 → flex 容器）。**4 层连锁防御**：中间区外层/中间区-左/终端外层/Terminal 根 div **都**必须加 `min-w-0`，**任一层缺失**都会让 flexbox `min-width: auto` 把 Terminal 父容器撑大，挤压右侧栏（即使右侧栏 `w-80 flex-shrink-0` 也无效——CSS 规范 `min-width` 优先级 > `flex-shrink`）。`overflow-hidden` 仅裁掉 Terminal 根 div 多余的 canvas/viewport 延伸，xterm 内部 `.xterm-viewport` 自带 `overflow: auto` 仍能滚动。

22. **bug 修复不联动原则（v1.1.1 教训）**：df389dd 撤 `useFontSize` 时**错误地把 52e132b 的 App.tsx `overflow-hidden` 也撤了**——把"字号联动 <html>"和"整页 overflow 防御"两个独立问题错误耦合。**正确做法**：每个 fix 提交**只解决一个具体问题**，撤 PR / 撤 fix 时**只撤**与该问题直接相关的代码，不要顺手回滚无关的防御代码。

23. **README 同步纳入 release 流程（v1.1.1）**：v1.0.2 / v1.1.0 / v1.1.1 连续 3 次 release 都忘了改 README，导致 shields.io badge 和 installer 文件名引用旧版本号。**自动化分层**：
    - **机械的 2 处**（badge URL `version-X.Y.Z-blue.svg` + installer 文件名 `OhMySerial_X.Y.Z_x64-setup.exe`）：用 sed 自动化，纳入 `scripts/release.sh` step 2.5 和 `.agents/skills/release/SKILL.md` AI 工作流
    - **语义化的 2 处**（"💡 v1.X.Y 完整功能" 描述段 + 路线图列表）：**不**自动改，AI 必须人工写——版本号的差异会让"上一版"和"这版"的语境丢失，强行 sed 替换会产生误导用户的文档
    - **教训**：文档化 checklist 100% 会被遗忘（连续 3 次就是证据），**必须**用脚本/AI 工具自动化"机械的部分"

24. **预设命令去 name 字段（v1.1.2）**：工业控制场景里用户记的是字节序列本身（`AA 55 03 01`），不是"重启设备"这种抽象标签。**name 是多余的中间层**——列表只显示 name，content 隐藏在 hover title 里，导致用户看不到"实际发出去什么"；表单里 name 和 content 是两个长得一样的文本框，用户容易填错位置。
    - 砍掉 name 后：表单字段从 3 个降到 2 个（type + content），表单天然少一个错填项
    - 列表直接展示 content 预览 + type 徽章，**所见即所发**
    - 旧 localStorage 数据：migrate 不主动 strip 旧 name（避免遍历大数组），UI/接口层不读 name，残留无害

25. **所有发送面板都接 onSent 写终端（v1.1.2 教训）**：v0.6.0 给 SendPanel 加了 `onSent` 回调写终端 TX 行，v1.1.2 给 PresetPanel 同步接线——这是**所有"调 sendData 的 UI"的统一契约**。如果某个发送入口漏接 onSent，用户点击后"没反应"（数据发出去但终端没回显）。**守卫**：未来新增发送面板，必须接受 onSent prop 并在 sendData 成功后调 `onSent?.(bytes)`。

26. **migrate 函数提取为可独立测试的纯函数（v1.1.2）**：v1.1.2 起 persist 的 `migrate` 提取为 `export function migratePreset(state, version): PresetState`，**便于单测**——zustand v4 persist 闭包不暴露 migrate（`store.persist.options` 路径不存在），直接调 migrate 验证行为比"塞 localStorage 触发 rehydrate"简洁一个量级。

27. **Recorder 是纯字符串 sink（v1.2.0）**：Rust 端不持 HEX/TEXT/编码知识，只接收已格式化好的字符串行（来自前端的 `formatLine`），写盘 `BufWriter` 不做解析。**设计收益**：(1) 加新格式（如 CSV/JSON）只需改前端 formatLine + 新 IPC type，Rust 不动；(2) Rust 单元测试不需要 mock xterm，纯文件 I/O 可测；(3) 前端 Terminal 和 Recorder 共用同一行格式 → 所见即所录。

28. **重连不切文件（v1.2.0）**：Recorder 是 `Arc<Mutex<Option<>>>` 挂在 SerialState 上，跨 reader 线程生命周期（断线/重连都不释放）；断线/重连事件通过 `mark_event` 写 `# 注释行`（含 `gap X.XXXs`），便于离线分析时识别数据缺口。**与 v0.5.0 schedule_reconnect 复用同 channel 是同款设计**（共享状态透传给新线程）。

29. **closePort = 录制结束（v1.2.0 Q13A）**：用户主动 closePort 时自动 stop 录制 + `console.info` 提示 summary（KB / 耗时）。**反面选择**：不保留跨 session 的"暂停录制"语义——工业抓包场景都是连续 session，跨 session 录制极少需要；如需手动跨 session 录制，行为类似录视频的 start/stop，比"暂停/继续"语义清晰。

30. **formatLine 提取为可测试纯函数（v1.2.0）**：从 Terminal.tsx 提取 `formatTimestamp` / `byteHex` + 新增 `formatLine` 到 `src/utils/terminalFormat.ts`，Terminal.tsx re-export 保持 backward compat（`tests/frontend/terminal.test.ts` 直接 import formatTimestamp 测试）。**便于单测**——jsx 组件在 jsdom 下挂载 xterm 是已知坑（v1.1.0 TerminalFont.test.ts 注释），纯函数零依赖可放心覆盖。

31. **CustomEvent 通信解耦（v1.2.0 SettingsPanel）**：⚙ 按钮在 SerialToolbar 里，触发 SettingsPanel 在 App 根级渲染。两者层级差 3 层，prop drilling 累赘；用 `window.dispatchEvent('oh-my-serial:toggle-settings')` + App.tsx `useEffect addEventListener` 解耦。**未来加更多 Modal（如 About / Help）也可走同一事件名 + 不同 custom event**，无需引入新的状态管理库。

32. **集成测试放单元测试模块（v1.2.0 教训）**：`tests/recorder_integration.rs` 原本想测"同路径 resume / drop flush / 并发写入"，但 `use oh_my_serial::*` 会拉入 Tauri runtime 触发 Windows `STATUS_ENTRYPOINT_NOT_FOUND`（`config_json_shape.rs` 注释已记录）。**改方案**：3 个集成场景直接放 `recorder/mod.rs` 的 `#[cfg(test)] mod tests`，作为单元测试运行——绕开 Windows 入口点问题，测试逻辑不变。**教训**：Windows 上 Rust 集成测试 import 主 crate 是地雷，能放单元测试就放单元。

33. **核心层抽离为 `oh-my-serial-core`（v1.3.0 起步）**：原 `src-tauri/` 把串口业务 + Tauri IPC 耦合在一个 crate，为了未来 `egui-app` / `tauri-app` 双宿主实现 A/B 对比测试，拆出纯 Rust 的 `core/` crate。**关键约束**：
    - `core/` 不能依赖 `tauri` / `egui` 任何 UI 框架
    - 数据通道用 `tokio::sync::mpsc::Sender<Vec<u8>>`（替代 `tauri::ipc::Channel<Vec<u8>>`），Tauri 主机侧在 `cmd_open_port` 里 spawn forwarder task 桥接 mpsc → Tauri Channel，保持 v0.6.0 的零拷贝契约
    - 事件用 `tokio::sync::broadcast::Sender<BackendEvent>`（替代 `app.emit()`），lib.rs setup 里订阅一次后转发到 `app.emit("...", ...)`，前端事件名契约（`port-disconnected` / `reconnect-status` / `send-poller-error` / `send-precise-error`）保持不变
    - `Backend::open_port` 用 `self: &Arc<Self>` 签名，reader 线程克隆 Arc 持有，关闭时自然 drop
    - `core/src/lib.rs` 重新导出全部公开类型（`RingBuffer` / `SendCommand` / `SendQueue` / `Recorder` / `AppConfig` / `PortInfo` / `Backend` / `BackendEvent` / `ReconnectPhase` / 等），bench 文件改 `use oh_my_serial_core::{...}` 即可复用
    - **测试改进**：核心层 58 个单测移到 `core/src/**/mod tests`，无 Tauri 依赖，**Windows 上稳定通过**（之前同一组测试在 src-tauri --lib 跑会偶发 STATUS_ENTRYPOINT_NOT_FOUND）
    - **未变**：前端（React + xterm.js）零改动，27 个 IPC 命令签名零改动，webview 通信契约零改动

34. **egui 启动 panic：`tokio::spawn` 找不到 reactor**（v1.3.0 经验）：egui-app 用 `Box::leak(Runtime)` 长生命周期 runtime，eframe 主线程没 `rt.enter()`，bridge task 里裸调 `tokio::spawn(...)` 会 panick `"there is no reactor running"`。**修法**：bridge 函数显式接受 `tokio::runtime::Handle` 参数，`runtime.handle().spawn(...)` 替代 `tokio::spawn(...)`。**诊断辅助**：`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` 在 release 模式下吞掉 stderr，panic 信息无痕。装 `std::panic::set_hook` 把 panic + backtrace 写到 `%TEMP%/oh-my-serial-egui/panic.log`，第一次启动崩溃就能定位。
