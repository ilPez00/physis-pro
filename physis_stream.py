#!/usr/bin/env python3
"""
Physis Stream Daemon — runs on omo, proxies to pansa master.
Caches classify/embedd locally for fast reads.
Writes go directly to pansa (source of truth).
Forwards Authorization + X-Physis-User headers.
"""

import json
import os
import sqlite3
import time
import hashlib
from http.server import HTTPServer, BaseHTTPRequestHandler
import requests

PANSA_PHYSIS = "http://192.168.1.15:19876"
LISTEN_PORT = 19876
CACHE_DB = "/home/gio/.physis/stream_cache.db"

class Cache:
    def __init__(self, db_path=CACHE_DB):
        self.db = sqlite3.connect(db_path)
        self.db.execute("CREATE TABLE IF NOT EXISTS classify (hash TEXT PRIMARY KEY, result TEXT, ts REAL)")
        self.db.execute("CREATE TABLE IF NOT EXISTS embedd (hash TEXT PRIMARY KEY, result TEXT, ts REAL)")
        self.db.commit()
    
    def _key(self, text):
        return hashlib.md5(text.encode()).hexdigest()
    
    def get_classify(self, text):
        h = self._key(text)
        row = self.db.execute("SELECT result, ts FROM classify WHERE hash=?", (h,)).fetchone()
        if row and time.time() - row[1] < 3600:
            return json.loads(row[0])
        return None
    
    def put_classify(self, text, result):
        h = self._key(text)
        self.db.execute("INSERT OR REPLACE INTO classify VALUES (?,?,?)", (h, json.dumps(result), time.time()))
        self.db.commit()
    
    def get_embedd(self, text):
        h = self._key(text)
        row = self.db.execute("SELECT result, ts FROM embedd WHERE hash=?", (h,)).fetchone()
        if row and time.time() - row[1] < 3600:
            return json.loads(row[0])
        return None
    
    def put_embedd(self, text, result):
        h = self._key(text)
        self.db.execute("INSERT OR REPLACE INTO embedd VALUES (?,?,?)", (h, json.dumps(result), time.time()))
        self.db.commit()

cache = Cache()

class Handler(BaseHTTPRequestHandler):
    def _forward_headers(self):
        """Forward auth + tenant headers from client to pansa."""
        headers = {}
        auth = self.headers.get("Authorization")
        if auth:
            headers["Authorization"] = auth
        tenant = self.headers.get("X-Physis-User")
        if tenant:
            headers["X-Physis-User"] = tenant
        return headers
    
    def do_GET(self):
        if self.path == "/health":
            self._json(200, {"status": "ok", "mode": "stream", "upstream": PANSA_PHYSIS})
        elif self.path == "/api/v1/embedder":
            try:
                r = requests.get(f"{PANSA_PHYSIS}/api/v1/embedder", timeout=5)
                self._json(r.status_code, r.json())
            except:
                self._json(502, {"error": "upstream unavailable"})
        else:
            self._json(404, {"error": "not found"})
    
    def do_POST(self):
        body = self._read_body()
        headers = self._forward_headers()
        
        if self.path == "/api/v1/classify":
            text = body.get("text", "")
            cached = cache.get_classify(text)
            if cached:
                self._json(200, cached)
                return
            try:
                r = requests.post(f"{PANSA_PHYSIS}/api/v1/classify", json=body, headers=headers, timeout=10)
                if r.status_code == 200:
                    cache.put_classify(text, r.json())
                self._json(r.status_code, r.json())
            except Exception as e:
                self._json(502, {"error": str(e)})
        
        elif self.path == "/api/v1/embed":
            text = body.get("text", "")
            cached = cache.get_embedd(text)
            if cached:
                self._json(200, cached)
                return
            try:
                r = requests.post(f"{PANSA_PHYSIS}/api/v1/embed", json=body, headers=headers, timeout=10)
                if r.status_code == 200:
                    cache.put_embedd(text, r.json())
                self._json(r.status_code, r.json())
            except Exception as e:
                self._json(502, {"error": str(e)})
        
        else:
            try:
                r = requests.post(f"{PANSA_PHYSIS}{self.path}", json=body, headers=headers, timeout=30)
                self._json(r.status_code, r.json())
            except Exception as e:
                self._json(502, {"error": str(e)})
    
    def _read_body(self):
        length = int(self.headers.get("Content-Length", 0))
        if length:
            return json.loads(self.rfile.read(length))
        return {}
    
    def _json(self, code, data):
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("X-Physis-Mode", "stream")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())
    
    def log_message(self, format, *args):
        pass

print(f"[physis-stream] listening on :{LISTEN_PORT}")
print(f"[physis-stream] upstream: {PANSA_PHYSIS}")
server = HTTPServer(("127.0.0.1", LISTEN_PORT), Handler)
server.serve_forever()
