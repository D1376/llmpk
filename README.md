# llmpk

A terminal TUI for browsing LLM and coding-agent leaderboards from one keyboard-driven interface.

> **Note:** This is a personal vibe coding project using [Claude Code](https://docs.anthropic.com/en/docs/claude-code). Built for my own use to quickly compare LLM models and coding agents without opening a browser. Expect rough edges.

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
│ ? keybindings  q quit  [ ] board  j/k row  r reload  y copy                 │
│ i/s/p/c sort  o asc/desc  m chart view  / filter                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Features

- **3 leaderboard boards** — Artificial Analysis model rankings, Artificial Analysis coding-agent rankings, and DeepSWE coding-agent benchmark
- **Parallel startup fetching** — both boards load in background threads on startup; `r` refetches the current board
- **Table and full-width horizontal chart views** — toggle with `m`
- **Provider-aware colors** — AA provider colors and agent creator/provider accents carry into tables and charts
- **Per-board filtering** — type `/` to filter; filters are cached and scoped to the active board
- **Responsive layout** — adapts columns to terminal width; wide table views get selected-row detail panes
- **Metric sorting** — missing values stay last; cost-like metrics default to ascending when selected
- **Mouse support** — scroll with mouse wheel, click tabs to switch boards
- **Row position indicator** — table view footer shows selected row / visible row count
- **OSC 52 copy** — press `y` to copy the selected model or agent label through terminal clipboard support

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

Requires Rust 1.80+.

## Usage

### Navigation

| Key | Action |
|-----|--------|
| `q`, `Esc`, `Ctrl-C` | Quit |
| `[` / `]` | Previous / next board |
| `1`, `2`, `3` | Jump to AA / AA Agents / DeepSWE |
| `r` | Reload current board |
| `y` | Copy selected row label to clipboard (OSC 52) |
| `↑` / `↓` (or `k` / `j`) | Move selected row |
| `PgUp` / `PgDn` | Move by 10 rows |
| `Home` / `End` (or `g` / `G`) | Jump to first / last row |
| `?` or `h` | Toggle in-app help (lists every keybinding and explains each metric) |
| Mouse scroll | Scroll the table |

### Sorting

| Key | AA | AA Agents | DeepSWE |
|-----|----|-----------|---------|
| `i` | Intelligence | Index | — |
| `a` | — | Pass@1 | Pass@1 |
| `s` | Speed | Turns | Steps |
| `p` | Price | Cost | Cost |
| `t` | — | Time | Time |
| `u` | — | Tokens | Tokens |
| `c` | Context | — | — |
| `o` | Toggle sort direction | Toggle sort direction | Toggle sort direction |

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
| AA Agents | [artificialanalysis.ai/agents/coding-agents](https://artificialanalysis.ai/agents/coding-agents) | Coding Agent Index, Pass@1, mean cost, mean execution time, mean tokens, mean turns |
| DeepSWE | [deepswe.datacurve.ai](https://deepswe.datacurve.ai/) | Pass@1, mean cost ($), mean duration (s), mean total tokens, mean agent steps |

## How it works

Artificial Analysis is a Next.js application. It embeds leaderboard data in the HTML stream via React Server Components (`self.__next_f.push([1, "..."])` calls).

llmpk:

1. Fetches each page with a shared `reqwest` client (connection pooling, TLS session reuse)
2. Extracts all RSC push chunks with a regex
3. Decodes JS string escapes and concatenates into a single stream
4. Parses AA model objects by scanning for balanced JSON objects containing `intelligence_index`
5. Parses AA Agents rows by scanning for the first balanced `rows` array
6. Fetches DeepSWE data directly from a JSON API endpoint
7. Preserves provider metadata where available so table and chart colors stay tied to the underlying model or agent creator

This is inherently fragile. If the site changes its markup, the scraper breaks. That's a feature, not a bug — it surfaces real breakage instead of hiding it behind retries.

## Building

```sh
cargo build --release
```

The release profile uses thin LTO, single codegen unit, and symbol stripping for a small, fast binary.

Run from source with:

```sh
cargo run --release
```

### Testing

Fixture and live tests are gated behind environment variables:

```sh
# Fixture-based tests
LLMPK_HOMEPAGE_FIXTURE=path/to/aa.html cargo test
LLMPK_CODING_AGENTS_FIXTURE=path/to/coding-agents.html cargo test
LLMPK_DEEPSWE_FIXTURE=path/to/leaderboard-live.json cargo test

# Live network tests
LLMPK_LIVE=1 cargo test live_fetch

# Benchmarks
LLMPK_BENCH=1 cargo test bench_fetch -- --nocapture
LLMPK_HOMEPAGE_FIXTURE=path/to/aa.html LLMPK_BENCH=1 cargo test bench_regex -- --nocapture
```

## Project structure

```
src/
  main.rs           Entry point, terminal setup/teardown, event loop, fetch dispatch
  rsc.rs            HTTP client, RSC stream extraction, brace/bracket scanners
  aa.rs             artificialanalysis.ai parser
  coding_agents.rs  artificialanalysis.ai coding agents parser
  deepswe.rs        deepswe.datacurve.ai parser (JSON API, best-per-model dedup)
  board.rs          Board enum, Data/Status wrappers, fetch dispatch
  ui.rs             AppState, sort/filter state, render dispatch, shared helpers
  ui/aa_board.rs    AA table/detail/chart rendering
  ui/agents.rs      Agents table/chart rendering, brand colors
  ui/deepswe_board.rs  DeepSWE table/detail/chart rendering, model colors
  ui/chart.rs       Shared horizontal chart infrastructure
  ui/filter.rs      Per-board filter matching and filter cache helpers
  ui/sort.rs        Sort keys, sort direction, and missing-value ordering
  ui/state.rs       Runtime app state, selection, filters, views, and sorting
  ui/chrome.rs      Tabs, header, footer, help overlay, loading/error screens
```

## License

MIT © Dsh
