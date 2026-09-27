# Render all categories from committed data + serve site/ locally.
# No GitHub API calls — pure template/scores.json render.

render:
    cargo run -q -- render
    cargo run -q -- render --category pi
    cargo run -q -- render --category herdr

serve: render
    python3 -m http.server -d site 8000
