<div align="right">
  <a href="../../../README.md"><img src="https://img.shields.io/badge/🇺🇸-English-blue?style=flat-square" alt="English"/></a>
  <a href="../th/README.md"><img src="https://img.shields.io/badge/🇹🇭-ภาษาไทย-red?style=flat-square" alt="ภาษาไทย"/></a>
  <a href="../zh/README.md"><img src="https://img.shields.io/badge/🇨🇳-简体中文-yellow?style=flat-square" alt="简体中文"/></a>
</div>

<div align="center">
  <img src="../../../assets/logo.png" alt="Garudust" width="260"/>
</div>

# Garudust

**ระบบรันไทม์ AI agent ที่โฮสต์เองได้ เขียนด้วย Rust**

แชทจากเทอร์มินัล เชื่อมต่อกับ Telegram / Discord / Slack / Matrix หรือเรียกใช้งานผ่าน HTTP — ทั้งหมดจากไบนารีเดียว

[![CI](https://github.com/garudust-org/garudust/actions/workflows/ci.yml/badge.svg)](https://github.com/garudust-org/garudust/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../../LICENSE)
![Rust 1.75+](https://img.shields.io/badge/rust-1.75+-orange.svg)

---

## ทำไมต้อง Garudust?

เฟรมเวิร์ก AI agent ส่วนใหญ่เขียนด้วย Python ขนาดหนัก และสตาร์ทช้า Garudust แตกต่างออกไป:

- **ไบนารีขนาด ~10 MB, cold start < 20 ms** — ไม่ต้องใช้ Python runtime หรือ Docker สำหรับการใช้งานบนเครื่องท้องถิ่น
- **เปลี่ยนผู้ให้บริการ LLM ด้วย env var เดียว** — รองรับ Anthropic, OpenRouter, AWS Bedrock, OpenAI Responses API หรือ endpoint ที่เข้ากันได้กับ OpenAI
- **รันได้ทุกที่** — TUI บนแล็ปท็อป, เซิร์ฟเวอร์ headless, Docker, Telegram, Discord, Slack, Matrix, HTTP
- **ประกอบต่อได้ง่าย** — แต่ละส่วนแยกเป็น crate อิสระ เพิ่มเครื่องมือ แพลตฟอร์ม หรือทรานสปอร์ตได้โดยไม่กระทบโค้ดส่วนอื่น

---

## เริ่มต้นใช้งาน

### 1. Docker (เร็วที่สุด)

```bash
# ผู้ให้บริการ cloud (OpenRouter, Anthropic, …)
echo "OPENROUTER_API_KEY=sk-or-..." > .env
docker compose up

# หรือ: โมเดลบนเครื่องผ่าน Ollama (ไม่ต้องใช้ API key)
echo "OLLAMA_BASE_URL=http://host.docker.internal:11434/v1" > .env
echo "GARUDUST_MODEL=llama3.2" >> .env
docker compose up
```

```bash
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "2+2 เท่ากับเท่าไร?"}'
```

### 2. Build จาก source

**ข้อกำหนดเบื้องต้น:** Rust 1.75+ และ API key จาก [OpenRouter](https://openrouter.ai) / [Anthropic](https://console.anthropic.com) หรือมี [Ollama](https://ollama.com) ทำงานอยู่

```bash
git clone https://github.com/garudust-org/garudust
cd garudust
cargo build --release
export PATH="$PATH:$(pwd)/target/release"

garudust setup   # เลือก provider และบันทึก API key
garudust         # เปิด TUI แบบโต้ตอบ
```

---

## การใช้งาน CLI

### TUI แบบโต้ตอบ

```bash
garudust
```

| ปุ่ม | การทำงาน |
|------|----------|
| `Enter` | ส่งข้อความ |
| `↑ ↓` | เลื่อนดูประวัติ |
| `/new` | ล้างประวัติ เริ่มเซสชันใหม่ |
| `/model <ชื่อ>` | เปลี่ยนโมเดลขณะใช้งาน |
| `/help` | แสดงคำสั่ง slash ทั้งหมด |
| `Ctrl+C` | ออกจากโปรแกรม |

### คำสั่งแบบ one-shot

```bash
garudust "สรุป git log จาก 7 วันที่ผ่านมาเป็น changelog"
garudust --model anthropic/claude-opus-4-7 "ตรวจสอบ PR นี้ด้านความปลอดภัย"
```

### คำสั่ง config

```bash
garudust setup                              # wizard ตั้งค่าครั้งแรก
garudust doctor                             # ตรวจสอบ API key, การเชื่อมต่อ, DB
garudust config show                        # แสดง config ที่ใช้งานอยู่
garudust config set model anthropic/claude-opus-4-7
garudust config set OPENROUTER_API_KEY sk-or-...
garudust config set ANTHROPIC_API_KEY  sk-ant-...
```

---

## เซิร์ฟเวอร์แบบ Headless

`garudust-server` รัน HTTP gateway, platform adapter ทั้งหมด และ cron job ในกระบวนการเดียว

```bash
garudust-server --anthropic-key sk-ant-... --port 3000
```

### HTTP API

```bash
# แบบ blocking
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "เขียน haiku เกี่ยวกับ Rust"}'

# Streaming (Server-Sent Events)
curl -X POST http://localhost:3000/chat/stream \
  -H "Content-Type: application/json" \
  -d '{"message": "อธิบาย async/await ใน 3 ประโยค"}'

# WebSocket
# เชื่อมต่อ: ws://localhost:3000/chat/ws
# ส่ง:      {"message": "งานของคุณ"}
# รับ:      text chunks … จากนั้น {"done":true}

# Health & metrics
curl http://localhost:3000/health
curl http://localhost:3000/metrics   # รองรับ Prometheus
```

---

## Platform Adapter

เชื่อมต่อ agent กับแพลตฟอร์มส่งข้อความต่าง ๆ โดยตั้งค่า env var ที่เกี่ยวข้องและสตาร์ท `garudust-server`

### Telegram

1. สร้างบอทผ่าน [@BotFather](https://t.me/botfather) แล้วคัดลอก token
2. สตาร์ทเซิร์ฟเวอร์:

```bash
TELEGRAM_TOKEN=123456:ABC... garudust-server --anthropic-key sk-ant-...
```

### Discord

1. สร้าง application ที่ [discord.com/developers](https://discord.com/developers/applications)
2. ในส่วน **Bot** เปิดใช้ **Message Content Intent** และคัดลอก token

```bash
DISCORD_TOKEN=Bot_... garudust-server --anthropic-key sk-ant-...
```

### Slack

1. สร้าง Slack app ที่ [api.slack.com/apps](https://api.slack.com/apps)
2. เปิด **Socket Mode** และสร้าง App-Level Token (`xapp-…`)
3. เพิ่ม Bot Token Scopes: `chat:write`, `channels:history`, `im:history`
4. ติดตั้งใน workspace และคัดลอก Bot Token (`xoxb-…`)

```bash
SLACK_BOT_TOKEN=xoxb-... \
SLACK_APP_TOKEN=xapp-... \
garudust-server --anthropic-key sk-ant-...
```

### Matrix

รองรับ Matrix homeserver ทุกประเภท (matrix.org, Synapse/Dendrite แบบโฮสต์เอง ฯลฯ)

```bash
MATRIX_HOMESERVER=https://matrix.org \
MATRIX_USER=@yourbot:matrix.org \
MATRIX_PASSWORD=secret \
garudust-server --anthropic-key sk-ant-...
```

### Webhook

รับ `POST /webhook` รัน agent แล้วส่งคำตอบกลับไปยัง `callback_url`

```bash
curl -X POST http://localhost:3001/webhook \
  -H "Content-Type: application/json" \
  -d '{"text":"สรุปวันนี้","callback_url":"https://your-app/reply"}'
```

---

## ผู้ให้บริการ LLM

| ผู้ให้บริการ | วิธีเลือก | หมายเหตุ |
|-------------|-----------|----------|
| Anthropic | ตั้ง `ANTHROPIC_API_KEY` | Direct Messages API |
| OpenRouter | ตั้ง `OPENROUTER_API_KEY` *(ค่าเริ่มต้น)* | โมเดลกว่า 200 รายการ |
| AWS Bedrock | ตั้ง `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` | Converse API, SigV4 |
| OpenAI Responses API | `garudust config set provider codex` | endpoint `/v1/responses` |
| Ollama | ตั้ง `OLLAMA_BASE_URL` | บนเครื่อง ไม่ต้องใช้ key |
| vLLM | ตั้ง `VLLM_BASE_URL` | เซิร์ฟเวอร์ OpenAI-compatible บนเครื่อง |
| OpenAI-compatible อื่น ๆ | ตั้ง `GARUDUST_BASE_URL` | ทรานสปอร์ต OpenAI-compatible |

### Ollama (บนเครื่อง ไม่ต้องใช้ key)

```bash
OLLAMA_BASE_URL=http://localhost:11434/v1
GARUDUST_MODEL=llama3.2
```

### vLLM (เซิร์ฟเวอร์ OpenAI-compatible บนเครื่อง)

```bash
VLLM_BASE_URL=http://localhost:8000/v1
VLLM_API_KEY=token-abc123
GARUDUST_MODEL=meta-llama/Llama-3.1-8B-Instruct
```

### AWS Bedrock

```bash
garudust config set provider bedrock
garudust config set model    anthropic.claude-3-5-sonnet-20241022-v2:0
```

---

## เครื่องมือในตัว

| เครื่องมือ | คำอธิบาย |
|-----------|----------|
| `web_fetch` | ดึงข้อมูลจาก URL (หน้าสแตติก) |
| `web_search` | ค้นหาผ่าน Brave Search API (`BRAVE_SEARCH_API_KEY`) |
| `browser` | ควบคุม Chrome/Chromium ผ่าน CDP — navigate, คลิก, พิมพ์, screenshot, รัน JS |
| `read_file` | อ่านไฟล์จากระบบไฟล์ |
| `write_file` | เขียนไฟล์ไปยังระบบไฟล์ |
| `terminal` | รันคำสั่ง shell |
| `memory` | หน่วยความจำถาวรแบบ key-value (add / read / replace / remove) |
| `session_search` | ค้นหาแบบ full-text ข้ามการสนทนาในอดีต (SQLite FTS5) |
| `delegate_task` | สร้าง sub-agent แบบขนานสำหรับงานที่แบ่งย่อย |
| `skills_list` | แสดงรายการสกิลที่มีอยู่ |
| `skill_view` | โหลดคำแนะนำของสกิลตามชื่อ |

### MCP Tools

เชื่อมต่อ [MCP](https://modelcontextprotocol.io) server ใด ๆ ใน `~/.garudust/config.yaml`:

```yaml
mcp_servers:
  - name: filesystem
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
  - name: postgres
    command: npx
    args: ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/mydb"]
```

---

## สกิล

สกิลคือชุดคำแนะนำที่นำมาใช้ซ้ำได้ เก็บไว้ใน `~/.garudust/skills/` อ่านจากดิสก์ทุกครั้งที่เรียกใช้งาน

```
~/.garudust/skills/
  git-workflow/SKILL.md
  daily-standup/SKILL.md
```

---

## Environment Variable ทั้งหมด

| ตัวแปร | ค่าเริ่มต้น | คำอธิบาย |
|--------|------------|----------|
| `ANTHROPIC_API_KEY` | — | Anthropic key (เลือก Anthropic transport อัตโนมัติ) |
| `OPENROUTER_API_KEY` | — | OpenRouter key (provider เริ่มต้น) |
| `OLLAMA_BASE_URL` | — | Ollama base URL — ไม่ต้องใช้ key |
| `VLLM_BASE_URL` | — | vLLM base URL |
| `VLLM_API_KEY` | — | vLLM API key (ไม่บังคับ) |
| `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` | — | Bedrock credentials |
| `BRAVE_SEARCH_API_KEY` | — | เปิดใช้เครื่องมือ `web_search` |
| `GARUDUST_MODEL` | `anthropic/claude-sonnet-4-6` | identifier ของโมเดล |
| `GARUDUST_PORT` | `3000` | พอร์ต HTTP gateway |
| `GARUDUST_WEBHOOK_PORT` | `3001` | พอร์ต Webhook adapter (`0` = ปิดใช้งาน) |
| `GARUDUST_BASE_URL` | — | Override LLM base URL |
| `GARUDUST_API_KEY` | — | Bearer token สำหรับ `/chat*` |
| `GARUDUST_APPROVAL_MODE` | `smart` | `auto` \| `smart` \| `deny` |
| `GARUDUST_RATE_LIMIT` | — | Rate limit ต่อ IP (requests/นาที) |
| `TELEGRAM_TOKEN` | — | Telegram bot token |
| `DISCORD_TOKEN` | — | Discord bot token |
| `SLACK_BOT_TOKEN` | — | Slack bot token (`xoxb-…`) |
| `SLACK_APP_TOKEN` | — | Slack app token (`xapp-…`) |
| `MATRIX_HOMESERVER` | — | Matrix homeserver URL |
| `MATRIX_USER` | — | Matrix username |
| `MATRIX_PASSWORD` | — | Matrix password |
| `GARUDUST_CRON_JOBS` | — | คู่ `"cron_expr=task"` คั่นด้วยจุลภาค |
| `RUST_LOG` | `info` | ระดับ log |

---

## สถาปัตยกรรม

```
crates/
  garudust-core        trait และ type ที่ใช้ร่วมกัน — ไม่มี I/O
  garudust-transport   LLM adapter: Anthropic, OpenAI-compat, Codex, Bedrock, Ollama, vLLM
  garudust-tools       Tool registry + toolset ในตัว
  garudust-memory      FileMemoryStore (markdown) + SessionDb (SQLite + FTS5)
  garudust-agent       Agent run loop, context compressor, prompt builder
  garudust-platforms   Telegram, Discord, Slack, Matrix, Webhook
  garudust-cron        Cron scheduler
  garudust-gateway     axum HTTP gateway

bin/
  garudust             CLI: TUI โต้ตอบ, one-shot, setup, doctor, config
  garudust-server      Headless: ทุกแพลตฟอร์ม + HTTP + cron
```

---

## การมีส่วนร่วม

```bash
git clone https://github.com/garudust-org/garudust
cd garudust
cargo build
cargo test --workspace
cargo clippy --workspace --all-targets -- -W clippy::all -W clippy::pedantic
```

อ่าน [CONTRIBUTING.md](../../../CONTRIBUTING.md) สำหรับแนวทางโค้ดและ checklist CI

---

## ใบอนุญาต

MIT — ดูที่ [LICENSE](../../../LICENSE)
