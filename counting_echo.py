#!/usr/bin/env python3
"""Counting echo server: GET anything, get back a counter + echo."""
import itertools
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

counter = itertools.count(1)

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = f"count={next(counter)} path={self.path} host={self.headers.get('Host')}\n"
        data = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *a):
        pass

ThreadingHTTPServer(("0.0.0.0", 8000), Handler).serve_forever()
