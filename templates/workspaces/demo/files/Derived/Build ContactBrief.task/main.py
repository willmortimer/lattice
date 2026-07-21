"""Build Derived/dist/index.html from Derived/input.txt for the stale→rebuild demo."""

from pathlib import Path

derived_dir = Path(__file__).resolve().parent.parent
input_path = derived_dir / "input.txt"
out_path = derived_dir / "dist" / "index.html"
body = input_path.read_text(encoding="utf-8").strip() if input_path.is_file() else "(missing input)"
out_path.parent.mkdir(parents=True, exist_ok=True)
out_path.write_text(
    "<!DOCTYPE html>\n"
    "<html lang=\"en\">\n"
    "<head><meta charset=\"utf-8\" /><title>Contact brief</title></head>\n"
    "<body>\n"
    "<h1>Contact brief</h1>\n"
    f"<pre>{body}</pre>\n"
    "</body>\n"
    "</html>\n",
    encoding="utf-8",
)
print(f"wrote {out_path}")
print("ok")
