# A fake OpenAI-compatible model on host loopback, standing in for errand's
# broker route /provider/<name>. Not product code.
#
#   fake_model.py PORT
#
# First request of a conversation: answers with one call to whichever tool
# looks like a shell, running COMMAND below. Once a tool result is in the
# conversation: answers with text quoting it. Logs, per request, the path, the
# Authorization header and the tool names offered, so what the sandbox sent is
# read here, outside it.
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

COMMAND = ("echo written-by-kage > /workspace/from-kage.txt; uname -sr; id -u; "
           "cat /etc/passwd 2>&1 | head -1; ls /home /root 2>&1 | head -2")


def log(*parts):
    print(*parts, flush=True)


def shell_tool(tools):
    names = [t.get("function", {}).get("name", "") for t in tools]
    for want in ("bash", "shell", "exec", "run", "command"):
        for n in names:
            if want in n.lower():
                return n
    return names[0] if names else None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        log("GET", self.path, "auth=" + str(self.headers.get("Authorization")))
        body = json.dumps({"object": "list", "data": [{"id": "fake-1", "object": "model"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        req = json.loads(raw or b"{}")
        tools = req.get("tools", [])
        msgs = req.get("messages", [])
        log("POST", self.path, "auth=" + str(self.headers.get("Authorization")),
            "tools=" + ",".join(t.get("function", {}).get("name", "") for t in tools))
        tool_results = [m for m in msgs if m.get("role") == "tool"]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()

        def chunk(delta, finish=None):
            event = {"id": "c1", "object": "chat.completion.chunk", "model": req.get("model", "fake-1"),
                     "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
            self.wfile.write(b"data: " + json.dumps(event).encode() + b"\n\n")
            self.wfile.flush()

        if not tool_results:
            name = shell_tool(tools)
            log("CALLING", name)
            args = json.dumps({"command": COMMAND})
            chunk({"role": "assistant", "content": None,
                   "tool_calls": [{"index": 0, "id": "call_1", "type": "function",
                                   "function": {"name": name, "arguments": ""}}]})
            chunk({"tool_calls": [{"index": 0, "function": {"arguments": args}}]})
            chunk({}, "tool_calls")
        else:
            content = tool_results[-1].get("content")
            text = content if isinstance(content, str) else json.dumps(content)
            log("TOOL-RESULT", json.dumps(text)[:600])
            chunk({"role": "assistant", "content": "tool said: " + text[:300]})
            chunk({}, "stop")
        usage = {"id": "c1", "object": "chat.completion.chunk", "choices": [],
                 "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}
        self.wfile.write(b"data: " + json.dumps(usage).encode() + b"\n\ndata: [DONE]\n\n")
        self.wfile.flush()


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
