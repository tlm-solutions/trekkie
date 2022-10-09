# Trekkie

[![built with nix](https://builtwithnix.org/badge.svg)](https://builtwithnix.org)

**Contact:** <dump@dvb.solutions>

This service takes your GPS tracks and times and regenerates the position mapping.

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

The `POST /user/create` endpoint will create a simple user and return the `user_id` and `password` which should be saved persistently, because they are required to authenticate against the `/user/login` endpoint.

Uploading a track is a two stage process the first is submitting the GPX file to `/travel/submit/gpx`. The second part is uploading the measurement intervals with the `/travel/submit/run` endpoint this endpoint requires the user to specify the corresponding gpx file.


