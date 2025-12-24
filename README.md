# Handy Pro Plus (Fork)

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
Send voice to ChatGPT/Perplexity via **Handy Connector** Chrome extension.

> ⚠️ **Requires:** [Handy Connector](https://github.com/MaxITService/Handy-connector) Chrome extension must be installed and running. Without it, "Send to Extension" features won't work.

**Three modes:**

| Action | Input | Output to ChatGPT |
|--------|-------|-------------------|
| **Send to Extension** | Voice only | Just your question |
| **Send with Selection** | Voice + selected text | Question with context |
| **Send Screenshot** | Voice + screenshot | Question with image attachment |

**Examples:**
- Press shortcut, say "what is recursion" → ChatGPT gets your question
- Select error log, say "why is this failing" → ChatGPT gets question + the log
- Select article, say "summarize this" → ChatGPT gets instruction + full text
- Capture region, say "explain this chart" → ChatGPT gets question + screenshot

### 📷 Send Screenshot to Extension (Windows only) (Requires external screenshot tool, like ShareX!)
Capture a screenshot region and send it with voice instruction to ChatGPT/Claude.

1. Press the shortcut and speak your instruction
2. Select screen region with ShareX (or your configured tool)
3. Screenshot + instruction sent to extension

**"Allow Without Voice" mode:** Can send screenshot with just a default prompt (e.g., "Look at this picture") — no voice needed.

**Setup:** Settings → Browser Connector → Screenshot Settings

---

## Keyboard Shortcuts

| Action | Default Shortcut |
|--------|-----------------|
| Transcribe | `Ctrl+Space` (Win/Linux), `Alt+Space` (macOS) |
| AI Replace Selection | `Ctrl+Shift+Space` |
| Send to Extension | Configure in Settings → Shortcuts |
| Send with Selection | Configure in Settings → Shortcuts |
| Send Screenshot | Configure in Settings → Shortcuts |

---

## Configuration

### LLM Provider (Required for AI Replace)
Settings → Post-Processing → Configure your LLM provider (OpenAI, Anthropic, etc.)

AI Replace uses the same LLM provider configured for post-processing.

### AI Replace Prompts
Settings → Advanced → AI Replace Settings

| Setting | Description |
|---------|-------------|
| **System Prompt** | Instructions for the LLM (e.g., "return only transformed text") |
| **User Prompt Template** | Template with `${instruction}` (your voice) and `${output}` (selected text) |
| **No Selection System Prompt** | Alternative prompt when no text is selected |
| **Max Characters** | Limit for selected text (default: 20000) |

Default user template:
```
INSTRUCTION:
${instruction}

TEXT:
${output}
```

### Send to Extension Prompts
"Send with Selection" uses the same AI Replace prompt templates to format the message before sending to ChatGPT.

### Handy Connector Setup
1. Install [Handy Connector](https://github.com/user/handy-connector) Chrome extension
2. Open ChatGPT or Perplexity in a browser tab
3. Click extension icon → "Bind to this tab"
4. Extension polls `http://127.0.0.1:63155` for messages from Handy

---

## Limitations
- **AI Replace Selection**: Windows only
- **Send Screenshot to Extension**: Windows only (uses ShareX or similar)
- **Remote STT**: Windows only (API keys in Windows Credential Manager)

## Original Features
All original Handy features remain: local Whisper, VAD, global shortcuts, push-to-talk, LLM post-processing, transcription history.

## License
MIT License — NO WARRANTIES.
