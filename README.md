# llmpk

A terminal TUI that aggregates LLM and AI-model leaderboards from multiple sources into a single, navigable interface.

> **Note:** This is a personal vibe coding project using [Claude Code](https://docs.anthropic.com/en/docs/claude-code). Built for my own use to quickly compare LLM models across leaderboards without opening a browser. Expect rough edges.

No API keys. No headless browser. No JavaScript runtime. Just HTTP and regex.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Boards                                                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ llmpk - AA                                                                  │
│ artificialanalysis.ai  |  102 models  |  view: table  |  sort: Intelligence │
├─────────────────────────────────────────────────────────────────────────────┤
│ #  Model                Intel  $/M   Provider    Ctx                        │
│ 1  Claude Sonnet 4.5     69.2  3.00  Anthropic   200K                       │
│ 2  GPT-5                  68.4  5.00  OpenAI      256K                       │
│ 3  Gemini 2.5 Pro        67.0  3.50  Google       1M                        │
│ ...                                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ ? keybindings  q quit  [ ] board  ↑/↓ move row  r reload                   │
│ i/s/p/c sort  o asc/desc  m chart view  / filter                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Features

- **11 leaderboard boards** — Artificial Analysis (intelligence, speed, price, context) and Arena (text, search, vision, document, code, text-to-image, image-edit, text-to-video, image-to-video, video-edit)
- **Lazy fetching** — boards load on first visit, cached for the session
- **Neighbor prefetching** — adjacent boards load in the background after the current one finishes
- **Table and chart views** — toggle with `m`
- **Per-board filtering** — type `/` to filter, `Ctrl-U` to clear
- **Responsive layout** — adapts columns and detail pane to terminal size
- **Sorting** — by any metric, ascending or descending

## Installation

### Prebuilt binaries

```sh
curl -fsSL https://github.com/D1376/llmpk/releases/latest/download/install.sh | bash
```

Prebuilt for macOS Apple Silicon (`aarch64-apple-darwin`) and Linux x86_64 (`x86_64-unknown-linux-gnu`). Set `LLMPK_INSTALL_DIR` to override the destination directory.

### From source

```sh
cargo install --git https://github.com/D1376/llmpk.git
```

Requires Rust 1.70+.

## Usage

### Navigation

| Key | Action |
|-----|--------|
| `q`, `Esc`, `Ctrl-C` | Quit |
| `[` / `]` | Previous / next board |
| `1`–`9`, `0`, `-` | Jump to board 1–11 |
| `r` | Reload current board |
| `↑` / `↓` (or `k` / `j`) | Move selected row |
| `?` | Toggle keybindings help |

### Sorting

| Key | AA | Arena |
|-----|----|-------|
| `i` | Intelligence | Rating |
| `s` | Speed | — |
| `p` | Price | Price |
| `c` | Context | Context |
| `n` | — | Rank |
| `v` | — | Votes |
| `o` | Toggle sort direction | Toggle sort direction |

### View and filter

| Key | Action |
|-----|--------|
| `m` | Toggle table / chart view |
| `/` | Edit filter for current board |
| `Backspace` | Delete filter character |
| `Enter` / `Esc` | Finish editing filter |
| `Ctrl-U` | Clear filter |

## Data sources

| Board | Source | Metrics |
|-------|--------|---------|
| AA | [artificialanalysis.ai](https://artificialanalysis.ai/) | Intelligence index, output speed (t/s), blended price ($/M tokens), context window |
| Arena Text | [arena.ai/leaderboard/text](https://arena.ai/leaderboard/text) | ELO rating, votes, input/output price ($/M tokens), context length |
| Arena Search | [arena.ai/leaderboard/search](https://arena.ai/leaderboard/search) | ELO rating, votes |
| Arena Vision | [arena.ai/leaderboard/vision](https://arena.ai/leaderboard/vision) | ELO rating, votes |
| Arena Document | [arena.ai/leaderboard/document](https://arena.ai/leaderboard/document) | ELO rating, votes |
| Arena Code | [arena.ai/leaderboard/code](https://arena.ai/leaderboard/code) | ELO rating, votes |
| Arena T2I | [arena.ai/leaderboard/text-to-image](https://arena.ai/leaderboard/text-to-image) | ELO rating, votes, price per image |
| Arena ImgEdit | [arena.ai/leaderboard/image-edit](https://arena.ai/leaderboard/image-edit) | ELO rating, votes, price per image |
| Arena T2V | [arena.ai/leaderboard/text-to-video](https://arena.ai/leaderboard/text-to-video) | ELO rating, votes, price per second |
| Arena I2V | [arena.ai/leaderboard/image-to-video](https://arena.ai/leaderboard/image-to-video) | ELO rating, votes, price per second |
| Arena VidEdit | [arena.ai/leaderboard/video-edit](https://arena.ai/leaderboard/video-edit) | ELO rating, votes, price per second |

## How it works

Both Artificial Analysis and Arena are Next.js applications. They embed leaderboard data in the HTML stream via React Server Components (`self.__next_f.push([1, "..."])` calls).

llmpk:

1. Fetches the page HTML with a shared `reqwest` client (connection pooling, TLS session reuse)
2. Extracts all RSC push chunks with a regex
3. Decodes JS string escapes and concatenates into a single stream
4. Parses the JSON data directly from the stream — no browser, no JS engine

This is inherently fragile. If either site changes its markup, the scraper breaks. That's a feature, not a bug — it surfaces real breakage instead of hiding it behind retries.

## Building

```sh
cargo build --release
```

The release profile uses thin LTO, single codegen unit, and symbol stripping for a small, fast binary.

### Testing

Fixture and live tests are gated behind environment variables:

```sh
# Fixture-based tests
LLMPK_HOMEPAGE_FIXTURE=path/to/aa.html cargo test
LLMPK_ARENA_FIXTURE_DIR=path/to/arena_fixtures cargo test

# Live network tests
LLMPK_LIVE=1 cargo test live_fetch

# Benchmarks
LLMPK_BENCH=1 cargo test bench_fetch -- --nocapture
```

## Project structure

```
src/
  main.rs    Entry point, terminal setup/teardown, event loop, fetch dispatch
  rsc.rs     HTTP client, RSC stream extraction, brace/bracket scanners
  aa.rs      artificialanalysis.ai parser
  arena.rs   arena.ai parser, slug enum, entry struct
  board.rs   Board enum, Data/Status wrappers, fetch dispatch
  ui.rs      Tabs, table/chart rendering, sort state, key handling
```

## License

MIT © Dsh
