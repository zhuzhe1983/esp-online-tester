# One HTTP response from the original guest's own lwIP and socket implementation.
# This RAM fixture neither replaces Wi-Fi functions nor injects network responses.
import network, socket
ap = network.WLAN(network.AP_IF)
ap.active(True)
ap.config(ssid="CoreMatrix-check", security=0)
server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("0.0.0.0", 18080))
server.listen(1)
print("CM_SOFTAP_READY")
client, peer = server.accept()
request = b""
while b"\r\n\r\n" not in request:
    request += client.recv(1024)
assert request.startswith(b"GET /corematrix HTTP/1.1\r\n"), request
body = b"COREMATRIX_HTTP_" * 256
client.write(b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(body)).encode() + b"\r\nConnection: close\r\n\r\n" + body)
client.close()
server.close()
print("CM_SOFTAP_HTTP_PASS")
