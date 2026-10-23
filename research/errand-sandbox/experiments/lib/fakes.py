# Test doubles for the experiments, run on the host. Not product code.
#
#   fakes.py origin  PORT CERT KEY   TLS origin; answers with the Authorization
#                                    header it received, and logs it
#   fakes.py broker  PORT ORIGIN_PORT
#                                    stands in for errand's egress broker: a
#                                    CONNECT proxy that sends every tunnel to
#                                    127.0.0.1:ORIGIN_PORT and logs each
#                                    request line; plain requests get 200
import socket
import ssl
import sys
import threading


def log(*parts):
    print(*parts, flush=True)


def read_head(sock):
    data = b""
    while b"\r\n\r\n" not in data:
        byte = sock.recv(1)
        if not byte:
            return None
        data += byte
    return data.decode("latin-1")


def pump(a, b):
    try:
        while True:
            d = a.recv(65536)
            if not d:
                break
            b.sendall(d)
    except OSError:
        pass
    finally:
        for s in (a, b):
            try:
                s.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass


def origin(port, cert, key):
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert, key)
    srv = socket.create_server(("127.0.0.1", port))
    while True:
        raw, _ = srv.accept()
        try:
            conn = ctx.wrap_socket(raw, server_side=True)
            head = read_head(conn) or ""
            auth = [l for l in head.split("\r\n") if l.lower().startswith("authorization:")]
            line = auth[0] if auth else "Authorization: <none>"
            log("ORIGIN-RECEIVED", line)
            # A digest, not the header: an origin that echoes the credential
            # hands it back into the sandbox (see results/30-run1-*).
            import hashlib
            body = ("received sha256=" + hashlib.sha256(line.encode()).hexdigest()[:16] + "\n").encode()
            conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % len(body) + body)
            conn.close()
        except (OSError, ssl.SSLError) as e:
            log("ORIGIN-ERROR", e)


def broker(port, origin_port):
    srv = socket.create_server(("127.0.0.1", port))

    def serve(conn):
        head = read_head(conn)
        if head is None:
            return
        first = head.split("\r\n")[0]
        log("BROKER", first)
        if first.upper().startswith("CONNECT"):
            up = socket.create_connection(("127.0.0.1", origin_port))
            conn.sendall(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            threading.Thread(target=pump, args=(conn, up), daemon=True).start()
            pump(up, conn)
        else:
            conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nbroker")
            conn.close()

    while True:
        c, _ = srv.accept()
        threading.Thread(target=serve, args=(c,), daemon=True).start()


if __name__ == "__main__":
    if sys.argv[1] == "origin":
        origin(int(sys.argv[2]), sys.argv[3], sys.argv[4])
    else:
        broker(int(sys.argv[2]), int(sys.argv[3]))
