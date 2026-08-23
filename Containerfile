# Multi-stage: compile the renderer, run it against the checked-in
# data/*/scores.json (no network calls, no GITHUB_TOKEN needed at build
# time), then serve the resulting static site/ with nginx.
FROM rust:1-slim AS build
WORKDIR /app
COPY . .
RUN cargo build --release && ./target/release/scorecards render --category neovim

FROM docker.io/library/nginx:alpine
COPY --from=build /app/site /usr/share/nginx/html
EXPOSE 80
