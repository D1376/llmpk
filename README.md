# llmpk

`llmpk` is a terminal TUI for comparing LLM and AI-model leaderboard data from
Artificial Analysis and Arena. It fetches each board on demand, keeps the UI
responsive with background fetches, and renders sortable tables directly in the
terminal.

No API keys, headless browser, or JavaScript runtime are required.

## Features

- 11 boards: Artificial Analysis plus 10 Arena leaderboards.
- Lazy fetching: boards load the first time you open them.
- Session caching: switching back to a loaded board is instant.
- Manual refresh for the current board.
- Sortable model tables with board-specific metrics.
- Artificial Analysis table and chart views.
- Missing or `null` fields render as `-` instead of crashing the TUI.

## Data Sources

| Board | Source | Data shown |
| --- | --- | --- |
| AA | <https://artificialanalysis.ai/> | Intelligence index, output speed, blended price, context window, release date, open-weight status |
| Arena Text | <https://arena.ai/leaderboard/text> | Rank, rating, votes, organization, text prices, context, license |
| Arena Search | <https://arena.ai/leaderboard/search> | Rank, rating, votes, organization, text prices, context, license |
| Arena Vision | <https://arena.ai/leaderboard/vision> | Rank, rating, votes, organization, text prices, context, license |
| Arena Document | <https://arena.ai/leaderboard/document> | Rank, rating, votes, organization, text prices, context, license |
| Arena Code | <https://arena.ai/leaderboard/code> | Rank, rating, votes, organization, text prices, context, license |
| Arena T2I | <https://arena.ai/leaderboard/text-to-image> | Rank, rating, votes, organization, price per image, license |
| Arena ImgEdit | <https://arena.ai/leaderboard/image-edit> | Rank, rating, votes, organization, price per image, license |
| Arena T2V | <https://arena.ai/leaderboard/text-to-video> | Rank, rating, votes, organization, price per second, license |
| Arena I2V | <https://arena.ai/leaderboard/image-to-video> | Rank, rating, votes, organization, price per second, license |
| Arena VidEdit | <https://arena.ai/leaderboard/video-edit> | Rank, rating, votes, organization, price per second, license |

## Install

Install the latest release:

```sh
curl -fsSL https://github.com/D1376/llmpk/releases/latest/download/install.sh | bash
```

This installs the prebuilt binary for macOS Apple Silicon or Linux x86_64. To
install somewhere specific:

```sh
curl -fsSL https://github.com/D1376/llmpk/releases/latest/download/install.sh | env LLMPK_INSTALL_DIR="$HOME/.local/bin" bash
```

## Uninstall

Remove the installed binary:

```sh
rm -f "$HOME/.local/bin/llmpk"
rm -f /usr/local/bin/llmpk
```

If you installed to a custom directory, remove `llmpk` from that directory.

## Build From Source

Requirements:

- Rust stable
- Network access to the source sites

Build a release binary:

```sh
cargo build --release
```

Install from the local checkout:

```sh
cargo install --path .
```

## Run

```sh
cargo run --release
```

Or run the built binary directly:

```sh
./target/release/llmpk
```

## Controls

| Key | Action |
| --- | --- |
| `q`, `Esc`, `Ctrl-C` | Quit |
| `[`, `]` | Previous / next board |
| `1`-`9`, `0`, `-` | Jump to boards 1-11 |
| `r` | Refresh the current board |
| `Up`, `Down`, `k`, `j` | Move the selected row |
| `o` | Toggle sort direction |
| `v` | Toggle AA table/chart view |

AA sort keys:

| Key | Sort metric |
| --- | --- |
| `i` | Intelligence |
| `s` | Output speed |
| `p` | Blended price |
| `c` | Context window |

Arena sort keys:

| Key | Sort metric |
| --- | --- |
| `n` | Rank |
| `i` | Rating |
| `v` | Votes |
| `p` | Price |
| `c` | Context window |

## How It Works

Both source sites are Next.js apps that embed leaderboard data in streamed RSC
payloads through `self.__next_f.push([1, "..."])` chunks. `llmpk` fetches the
HTML, extracts and decodes those chunks, then parses the embedded JSON data.

- Artificial Analysis parsing scans for balanced model objects containing
  `intelligence_index`.
- Arena parsing finds the first balanced `entries` array for each leaderboard.
- Fetches run in background threads so the TUI does not block.
- Each board is cached for the current process until you press `r`.

This is intentionally lightweight and brittle in the same way any page scrape is
brittle. If a source site changes its page payload shape, update the scraper
rather than adding broad retries.

## Tests

Run the default test suite:

```sh
cargo test
```

Fixture-backed tests are opt-in:

```sh
LLMPK_HOMEPAGE_FIXTURE=path/to/aa.html cargo test
LLMPK_ARENA_FIXTURE_DIR=path/to/arena_fixtures cargo test
```

Live fetch tests are also opt-in:

```sh
LLMPK_LIVE=1 cargo test live_fetch
```

## Project Layout

```text
src/main.rs    Terminal setup, event loop, background fetch dispatch
src/rsc.rs     Shared HTTP fetch, RSC extraction, balanced scanners
src/aa.rs      Artificial Analysis parser
src/arena.rs   Arena parser, slugs, entry types
src/board.rs   Board enum, status/data wrappers, fetch dispatch
src/ui.rs      Tabs, tables, chart view, sorting, key handling
```
