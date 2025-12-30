# AivoRelay

![large_logo](Promo/large_logo.jpg)

AI Voice Relay
A fork of [cjpais/Handy](https://github.com/cjpais/Handy) with additional features for Windows.

## Fork Features

### 🌐 Remote STT API

Use any OpenAI-compatible speech-to-text API instead of local Whisper model.

- No need to download 1GB+ models
- Use fast cloud services (Groq, Deepgram, etc.)
- Great for machines without GPU

**Setup:** Settings → Advanced → Transcription Provider → Remote OpenAI Compatible

### 🤖 AI Replace Selection (Windows only)

Voice-controlled text editing:

1. Select text in any application
2. Hold the AI Replace shortcut, speak your instruction
3. Release — selected text is replaced with AI result

**Examples:**

- Select code → say "add error handling" → get improved code
- Select paragraph → say "make it shorter" → get condensed version
- Empty field + "no selection" mode → say "write a greeting email" → get generated text

**Setup:** Settings → Advanced → AI Replace Settings

### 📤 Send to Extension

Send voice to ChatGPT/Perplexity via **AivoRelay Connector** Chrome extension.

> ⚠️ **Requires:** [AivoRelay Connector](https://github.com/MaxITService/AivoRelay-relay) Chrome extension must be installed and running. Without it, "Send to Extension" features won't work.

**Three modes:**

| Action                  | Input                 | Output to ChatGPT              |
| ----------------------- | --------------------- | ------------------------------ |
| **Send to Extension**   | Voice only            | Just your question             |
| **Send with Selection** | Voice + selected text | Question with context          |
| **Send Screenshot**     | Voice + screenshot    | Question with image attachment |

**Examples:**

- Press shortcut, say "what is recursion" → ChatGPT gets your question
- Select error log, say "why is this failing" → ChatGPT gets question + the log
- Select article, say "summarize this" → ChatGPT gets instruction + full text
- Capture region, say "explain this chart" → ChatGPT gets question + screenshot

### 📷 Send Screenshot to Extension (Windows only)

Capture a screenshot region and send it with voice instruction to ChatGPT/Claude.

**Requires:** External screenshot tool like [ShareX](https://getsharex.com/) (free, open source)

**How it works:**

1. Press and hold the shortcut, speak your instruction (e.g., "explain this chart")
2. Release the shortcut — voice is transcribed first
3. Screenshot tool launches automatically
4. Select screen region with your screenshot tool
5. Screenshot + your voice instruction sent to extension

**Workflow:**

```
[Hold shortcut] → [Speak] → [Release] → [Transcribe voice] → [Screenshot tool opens] → [Capture region] → [Sent to ChatGPT]
```

**"Allow Without Voice" mode:** Can send screenshot with just a default prompt (e.g., "Look at this picture") — useful when you just want to share an image without speaking.

**Setup:**

- Settings → Browser Connector → Screenshot Settings
- Configure your screenshot tool command (default: ShareX `-RectangleRegion`)
- Set screenshot folder path where your tool saves images

---

## Configuration

### LLM API Relay Configuration

**Settings → LLM API Relay**

Configure your LLM provider (OpenAI, Anthropic, etc.) to power Post-Processing and AI Replace features. You can use the same model for both or configure separate ones if needed.

### AI Replace Settings

**Settings → AI Replace**

| Setting                        | Description                                                                 |
| ------------------------------ | --------------------------------------------------------------------------- |
| **System Prompt**              | Instructions for the LLM (e.g., "return only transformed text")             |
| **User Prompt Template**       | Template with `${instruction}` (your voice) and `${output}` (selected text) |
| **No Selection System Prompt** | Alternative prompt when no text is selected                                 |
| **Max Characters**             | Limit for selected text (default: 20000)                                    |

Default user template:

```
INSTRUCTION:
${instruction}

TEXT:
${output}
```

### Send to Extension Prompts

"Send with Selection" uses the same AI Replace prompt templates to format the message before sending to ChatGPT.

### AivoRelay Connector Setup

1. Install [AivoRelay Connector](https://github.com/MaxITService/AivoRelay-relay) Chrome extension
2. Open ChatGPT or Perplexity in a browser tab
3. Click extension icon → "Bind to this tab"
4. Extension polls `http://127.0.0.1:63155` by default for messages from AivoRelay. Port must match in extension and in AivoRelay settings.

---

## Limitations

- **AI Replace Selection**: Windows only
- **Send Screenshot to Extension**: Windows only (uses ShareX or similar)
- **Remote STT**: Windows only (API keys in Windows Credential Manager)

## Original Features

All original Handy features remain: local Whisper, VAD, global shortcuts, push-to-talk, LLM post-processing, transcription history.

## License

MIT License — NO WARRANTIES.
