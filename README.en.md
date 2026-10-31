![OdeKrai banner](banner.png)

[Português (Brasil)](README.md) · [English](README.en.md)

**An AI assistant for your Windows PC, with a mascot by your side.**

OdeKrai chats with the AI provider you configure and can run skills on your computer within the permissions you set. The interface brings together conversations, settings, and a mascot that accompanies you while using the app.

> Pre-release project (version 0.15.0), for Windows.

## Highlights

- **Choose your AI:** cloud providers, options flagged with a free plan, and connections to compatible local servers.
- **A named mascot:** 12 characters with animations and Island and Free modes.
- **Local calendar:** organize events by day and month, set recurrence, and import or export `.ics` files.
- **Local skills:** the agent can run actions registered by the app, subject to Rust-enforced permissions.
- **Google Calendar:** connect your primary calendar under Skills > Calendar and reminders > Manage to view and manage events, with approval for changes. See [setup and scopes](docs/GOOGLE-CALENDAR.md).
- **You stay in control:** permission modes, approvals, activity records, and a kill switch.
- **Local settings:** provider credentials are stored by Rust in Windows Credential Manager.

## What OdeKrai does

### Conversations and providers

The catalog includes OpenAI, Anthropic, Google Gemini, xAI, Mistral, Cohere, Groq, DeepSeek, Perplexity, Cerebras, Together AI, OpenRouter, Fireworks AI, Azure OpenAI, and AWS Bedrock. Vertex AI appears as an integration guide, without integrated authentication. Local connections support compatible servers such as Ollama and llama.cpp that are already running.

The catalog flags free options for Google Gemini, Mistral, Groq, Cerebras, and OpenRouter; availability and limits depend on the provider, model, and account. You can configure a primary provider and backups. Failover is limited to transient failures before any output is observed; tool calls are not repeated by failover.

Conversations are kept in local SQLite history. To generate a reply, submitted content is sent to the configured provider. The app also offers streaming responses when supported by the adapter, searchable history, and external links that require confirmation by domain.

### Agent and skills

Turn on agent mode in a conversation to let the model request skills registered in Rust. Available skills include reading and organizing files in allowed folders; creating, listing, and updating local calendar events, deleting them with approval, or creating events in a selected `.ics` file; viewing and managing Google Calendar events, with approval for changes; creating Markdown, text, and CSV files; opening links, files, and Windows settings; notifications, clipboard, system information, volume, and media controls; email drafts; approved Discord webhook messages; reading web pages; and code delegation to Codex CLI or Claude Code.

Skills start disabled and can be enabled on the Skills screen. Results from external content are treated as untrusted, and actions with side effects may require approval under the policy enforced by Rust.

### Permissions and security

- **Approve for me**, **Always ask**, and **Full control** are the available modes; fixed blocks apply in every mode.
- Approvals appear in a dedicated window. The configured global shortcut triggers the kill switch, which stops runs and blocks new actions until resumed.
- The local audit log chains entries and blocks actions if a write or verification fails.
- Rust validates skills, risks, paths, and permissions. The WebView displays state and relays user decisions.

### Quick access and performance

- **Home** brings together a starting message, suggestions based on available data, recent conversations, and a setup checklist. Suggestions fill the field for review; they are never sent automatically.
- **Ctrl+K** opens the command palette in the focused window. The Activity panel opens on demand; Compact mode expands the existing mascot window and provides a quick command field.
- **Performance** offers Automatic, On, and Off modes. Automatic activates when Windows requests reduced motion, with up to 8 GiB of RAM, or with up to 4 logical CPU cores; On reduces visual effects and mascot animation.

### Identity, mascot, and sounds

The app provides a personal folder, user-approved `ODEKRAI.md` identity, editable memory notes, and a local profile. The mascot has 12 characters and animations associated with app states. It can appear in Island and Free modes, with its position saved. The app has configurable interface sounds and an option to minimize to the tray when the window closes.

### Privacy

Provider credentials are stored by the Rust backend in Windows Credential Manager. History, profile, and preferences use local storage; this does not mean the local database or backups are encrypted. Conversation content is sent to the provider you choose. Diagnostic logs do not include conversation content.

## Installation and development

The project is in pre-release. The configured Windows bundle is NSIS. Development requires Windows, Node.js 22.12 or newer, Rust, the Microsoft C++ build tools, Windows SDK, and WebView2 Runtime.

```powershell
npm ci
npm run tauri:dev
```

## Screenshots

Screenshots live in `docs/screenshots/`.

## Licenses and credits

See [LICENSE](LICENSE), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), and [LICENSE-ASSETS.md](LICENSE-ASSETS.md) for the terms covering the code, dependencies, and assets. Do not distribute the owner's private assets.
