状态：草案，待人工确认。基线：9865050f8628ef399cbab786d79026539830d3f2。

## 范围
使用 Rust stable 和 Cargo 实现单进程、内存存储的 HTTP 任务清单 API。可执行文件名为 demo02，监听 0.0.0.0:${PORT}，未设置 PORT 时使用 8080。重启后任务数据清空。
任务包含以下字段：
- id：从 1 开始，按创建顺序递增；删除后不复用。
- title：去掉首尾空白后保存，长度为 1 至 200 个 Unicode 标量。
- completed：布尔值，创建时为 false。
- created_at：UTC RFC 3339 时间字符串，以 Z 结尾。

实现以下接口：
| 请求 | 成功响应 | 未找到时 |
|---|---|---|
| GET /health | 200，{"status":"ok"} | — |
| POST /tasks，请求体为 {"title":"..."} | 201，返回创建的任务 | — |
| GET /tasks | 200，返回按 id 升序排列的任务数组 | — |
| GET /tasks/{id} | 200，返回任务 | 404，{"error":"not_found"} |
| POST /tasks/{id}/complete | 200，返回 completed=true 的任务；重复调用结果相同 | 404，{"error":"not_found"} |
| DELETE /tasks/{id} | 204，无响应体 | 404，{"error":"not_found"} |

错误响应采用 {"error":"<代码>"}。非 JSON 对象、缺少 title 或 title 非字符串时返回 400 invalid_json；去除首尾空白后标题为空时返回 400 title_empty；超过 200 个 Unicode 标量时返回 400 title_too_long；id 不是正整数时返回 404 not_found。请求体超过 64 KiB 时返回 413。
并发请求下保证任务创建、ID 分配和写操作的状态一致性。任何输入不得使服务崩溃。

## 验收标准
1. 服务按上述运行方式启动，各接口的状态码、响应体和任务状态符合本 Spec。
2. 标题去空白、长度边界、非法 JSON、缺失或错误类型的 title、非法或不存在的 ID、超限请求体均按规定处理。
3. ID 按创建顺序递增且删除后不复用；列表按 ID 升序；完成操作可重复调用；删除成功响应无响应体。
4. 自动化测试覆盖正常路径、输入校验和未找到场景，cargo test --locked 通过。
5. acceptance/fixtures/DEMO-02/cases.json 中的请求序列按顺序执行并逐条通过，包括依赖前一步 ID 的请求。
6. 按 PRD 要求，最终验收材料包含 PR、Reviewer 结论、Release、镜像和部署回执。

## 不做
- 数据持久化、数据库或重启后恢复。
- 分页和认证。
- 并发写冲突控制以外的事务。

## 风险与待确认事项
- PRD 仍为草案，待 AtomGit 书面确认并冻结；需求变更可能影响本 Spec 和验收结果。
- 验收 fixture 的具体内容尚未在本阶段核对，其可用性和覆盖范围需在后续实施时确认。
- PRD 规定超限请求返回 413，但未指定该情形的错误代码；实现前需确认，避免与验收期望不一致。
- 进程重启会清空数据，部署和验收流程需考虑这一限制。
