# 0.5 Request progress architecture

## Semantic layer

The primary owner-facing unit is one ChatGPT user message: a **Request**.

```text
Project
  -> ChatGPT Conversation
    -> Request (exact user-message id)
      -> declared plan / Steps
      -> Execution evidence
          -> direct AgentDock tools
          -> AgentDock task_manage Task(s)
          -> ACP / Codex Agent(s)
```

A Request exists even when it is only a query. Execution is optional. ACP is no longer required for representing work.

## Request capture

The unpacked Chrome/Edge extension observes the existing ChatGPT conversation transport. `conversation_id` and user-message ID are the identity boundary. The extension posts bounded Request lifecycle events to the paired loopback bridge:

- `/v1/request`: started/completed user Request;
- `/v1/tool`: structured AgentDock invocation/result metadata;
- `/v1/bind`: strict ACP/thread binding when exact ACP evidence exists.

Ordinary tool execution stores tool name, action and bounded safe project/path/task identifiers. It does not persist tool stdout, arbitrary command bodies, file replacement content, cookies or full transcript history.

## Declared progress

`execution-progress` is a user-level AgentDock Skill, not a patch to the AgentDock executable. It instructs execution requests to create a result-oriented `task_manage` plan, checkpoint steps as they complete, block/resume honestly and complete only after final review.

Monitor treats this plan as **LLM-declared semantic progress**. Direct tool/ACP activity remains **observed execution evidence**. If execution is observed but no Task plan is claimed, the Request is marked `unclaimed` rather than inventing progress.

## Dynamic plan revisions

AgentDock 0.8.1 `task_manage` cannot append steps to an existing Task. When execution discovers genuinely necessary additional work, the policy creates a supplemental Task titled `补充 · ...` in the same Request. The collector gathers every `task_id` observed in that Request and emits:

- `plans[]` in observation order;
- combined `completed / total`;
- current active step, preferring a later supplemental plan;
- `revisionCount = plans.length - 1`.

No AgentDock Task JSON is edited by Monitor. If a future AgentDock version adds native step insertion, replace the supplemental-Task fallback with that capability while keeping the Request-level model.

## Persistence

Monitor owns:

- `chat_requests`: exact conversation + user-message identity, bounded prompt/title and lifecycle;
- `request_tools`: structured AgentDock execution events and discovered task/entity IDs.

Both follow the Monitor 90-day observation retention window. Native AgentDock Tasks remain in `~/.agentdock/tasks` and are read by the collector.

## Browser boundary

A normal installer cannot silently install or reload an unpacked Chrome/Edge extension. Therefore source/unit/installed-loopback acceptance is not equivalent to a real authenticated ChatGPT acceptance. `sources.bridge.lastExtensionSeen` must be observed after loading/reloading the 0.5 extension before claiming live browser Request capture.
