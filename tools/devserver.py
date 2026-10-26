#!/usr/bin/env python3
"""Static server for the browser preview that also accepts PUT under web/local/ (gitignored),
so scripted recordings (?save=put) land on disk. Local development only: binds to 127.0.0.1."""
import http.server
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UPLOADS = os.path.join(ROOT, "web", "local")


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def do_PUT(self):
        target = os.path.realpath(os.path.join(ROOT, self.path.lstrip("/").split("?")[0]))
        if not target.startswith(os.path.realpath(UPLOADS) + os.sep):
            self.send_error(403, "uploads only under web/local/")
            return
        os.makedirs(os.path.dirname(target), exist_ok=True)
        length = int(self.headers.get("Content-Length", 0))
        with open(target, "wb") as f:
            f.write(self.rfile.read(length))
        self.send_response(201)
        self.end_headers()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4420
    http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
