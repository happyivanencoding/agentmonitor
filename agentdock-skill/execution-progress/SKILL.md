---
name: execution-progress
description: 所有需要真实执行工作的请求都使用：修改文件、开发、构建、测试、研究本地数据、运行命令、调用 AgentDock 工具或 ACP。把用户当前请求声明为可恢复任务，持续 checkpoint；遇到意外新增工作时实时扩展计划，不把查询类无执行请求强行任务化。
version: 1.0.1
---

# Execution Progress

用于让执行型工作在 Agent Monitor 中具有可恢复、实时更新的语义进度。

## 适用

只要当前用户消息要求真实执行动作，包括读写本地文件、改代码、构建、测试、调试、部署、处理数据、调用 AgentDock 工具或 ACP，就使用本 Skill。

纯解释、纯问答、状态查询且不需要执行动作时，不必创建 AgentDock Task；Agent Monitor 仍会把该用户消息记录为 Request。

## 执行协议

1. 每一条用户消息都是一个独立 Request。只要该消息会触发任何真实执行动作，就必须在开始执行前用 `task_manage create` 为**这一条 Request 新建**一个 Task；即使它是在继续上一条消息的工作，也不能把上一条 Request 的 Task 直接 resume/复用成当前 Request 的计划。
2. 把任务拆成 2–8 个“结果导向”的步骤。步骤描述应是用户能理解的阶段结果，不是机械工具动作。
3. 第一个步骤开始时设为 `in_progress`。
4. 每完成一个步骤，立即调用 `task_manage checkpoint`：完成项打勾，并把下一项设为 `in_progress`。不要等到最终回复前一次性补记。
5. 如果出现真实 blocker，使用 `task_manage block`；恢复时使用 `resume`。`resume` 只恢复**同一 Request 自己**的 Task，不用来替代为新用户消息创建新 Task。
6. 所有工作完成后先 `final_review`，通过后再 `complete`，最后才向用户交付完成结果。
7. 如果一条看似“测试一下 / 看看 / 好了吗”的短消息实际导致读取本地文件、调用工具、构建、验证或任何其他执行，它就是执行型 Request，必须有自己的 Task/Steps；不能因为文字很短就标成“无执行计划”。

## 动态计划

计划不是冻结的。执行中发现原方案不成立、需要额外修复、补充测试、增加验证或出现新的必要工作时，必须立即把变化反映到进度中，而不是继续显示旧计划。

当前 AgentDock `task_manage` 公开接口不能向既有 Task 追加 steps，因此采用“补充计划段”：

- 原 Task 保留，不直接编辑底层 JSON。
- 对新增的必要工作调用新的 `task_manage create`，title 以 `补充 · ` 开头，project 保持与当前请求一致，只包含新发现的步骤。
- 在创建补充计划段前，先对原 Task checkpoint，summary 简短说明为什么计划发生变化。
- Agent Monitor 会把同一 ChatGPT Request 中产生的多个 AgentDock Task 合并成一个动态计划，并显示新增步骤与修订次数。
- 后续如果 AgentDock 原生提供 add/insert step 能力，优先使用原生能力，不再创建补充计划段。

不要为了很小的工具动作创建补充计划段；只有会改变“还有哪些事情必须完成”的新工作才加入计划。

## 进度真实性

- `completed` 只表示该阶段结果真实完成，不表示“尝试过”。
- 测试失败后发现需要新修复：不要把验证步骤勾完成；应增加补充修复/复测步骤。
- 工具调用、ACP、进程和 token 是执行证据；Task/steps 是 LLM 声明的语义计划。两者不应互相冒充。
- 如果执行结束但 Task 仍未更新，应先修正 checkpoint 状态，再回复用户。

## 步骤粒度示例

好：

- 定位生成结束后 UI 不恢复的状态原因
- 修正 interaction-ready 状态切换
- 完成真机连续三回合验证
- 更新交接文档并形成可安装版本

不好：

- 读文件
- 跑命令
- 改代码
- 再看一下

## AgentDock 适配

AgentDock 中使用 `task_manage` 完成 create/checkpoint/block/resume/final_review/complete。普通 AgentDock 工具和 ACP 都可以作为该 Task 的执行方式；不要因为没有启动 ACP 就省略进度管理。
