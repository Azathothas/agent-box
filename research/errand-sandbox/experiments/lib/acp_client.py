# A minimal ACP client, driving one session the way errand does: a launcher's
# stdin and stdout carry newline-delimited JSON-RPC. Not product code.
#
#   acp_client.py PROMPT -- LAUNCHER ARGS...
#
# Prints each message it reads, prefixed IN, and exits 0 once a prompt turn
# ends, 1 on an error answer, 2 if the launcher died first.
import json
import os
import subprocess
import sys
import threading
import time

sep = sys.argv.index("--")
prompt = sys.argv[1]
launcher = sys.argv[sep + 1:]
proc = subprocess.Popen(launcher, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=sys.stderr, process_group=0)
pending = {}
answers = {}
lock = threading.Condition()
next_id = [0]


def send(method, params):
    with lock:
        next_id[0] += 1
        n = next_id[0]
    line = json.dumps({"jsonrpc": "2.0", "id": n, "method": method, "params": params})
    print("OUT", line[:300], flush=True)
    proc.stdin.write((line + "\n").encode())
    proc.stdin.flush()
    return n


def wait(n, timeout=120):
    end = time.time() + timeout
    with lock:
        while n not in answers:
            left = end - time.time()
            if left <= 0 or proc.poll() is not None:
                return None
            lock.wait(min(left, 0.5))
        return answers.pop(n)


def reader():
    for raw in proc.stdout:
        msg = json.loads(raw)
        print("IN", json.dumps(msg)[:400], flush=True)
        if "method" in msg and "id" in msg:
            # The agent asking the client. --yolo means no permission asks;
            # anything else is refused so the turn does not hang.
            reply = {"jsonrpc": "2.0", "id": msg["id"],
                     "error": {"code": -32601, "message": "not offered by this client"}}
            proc.stdin.write((json.dumps(reply) + "\n").encode())
            proc.stdin.flush()
        elif "id" in msg:
            with lock:
                answers[msg["id"]] = msg
                lock.notify_all()


threading.Thread(target=reader, daemon=True).start()
started = time.time()
init = wait(send("initialize", {"protocolVersion": 1, "clientCapabilities": {}}))
if not init or "error" in init:
    print("FAIL initialize", init)
    sys.exit(2 if init is None else 1)
ready_ms = int((time.time() - started) * 1000)
new = wait(send("session/new", {"cwd": "/workspace", "mcpServers": []}))
if not new or "error" in new:
    print("FAIL session/new", new)
    sys.exit(2 if new is None else 1)
sid = new["result"]["sessionId"]
turn = wait(send("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]}), 180)
total_ms = int((time.time() - started) * 1000)
print("TIMING initialize_ms=%d turn_total_ms=%d" % (ready_ms, total_ms), flush=True)
os.killpg(proc.pid, 15)
proc.wait(10)
if not turn or "error" in turn:
    print("FAIL prompt", turn)
    sys.exit(2 if turn is None else 1)
print("STOP", turn["result"].get("stopReason"))
sys.exit(0)
