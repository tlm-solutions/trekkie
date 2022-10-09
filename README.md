# Trekkie

[![built with nix](https://builtwithnix.org/badge.svg)](https://builtwithnix.org)

**Contact:** <dump@dvb.solutions>

This service takes your gps tracks and times and regenerates the position mapping.

## Building

```bash
    $ nix build
```

## Usage 

```mermaid
graph TD
    A[Start] -->|/user/login| B(Authenticated)
    A[Start] -->|/user/create| B(Authenticated)
    B -->|/travel/submit/gpx| C(Travel created & GPX Uploaded)
    C -->|/travel/submit/run| D(Upload Measurement Intervals)
    D --> E(Done)
```

