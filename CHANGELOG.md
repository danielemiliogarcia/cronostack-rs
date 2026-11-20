# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-11-20

First release.

### Added

- Columns side by side, each with a stopwatch and optional countdown, daily
  alarm, local clock, and project/developer labels.
- "One at a time": starting a stopwatch pauses the others.
- "Copy" puts developer, time, date and project on the clipboard as a
  spreadsheet row.
- Classic and System (light/dark) themes.
- System tray icon to hide the window, pause all, and quit.
- State saved as TOML in the OS data folder on exit and every 30 seconds;
  "Where is my data?" shows the path.
- `--version`, `--help` and `--smoke-test` command-line options.
- Release binaries for Linux (x86-64, ARM64), macOS (Apple silicon) and
  Windows (x86-64), with install scripts.
