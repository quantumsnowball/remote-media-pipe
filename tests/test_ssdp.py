import socket
import sys


def main():
    msg = (
        "M-SEARCH * HTTP/1.1\r\n"
        "HOST: 239.255.255.250:1900\r\n"
        'MAN: "ssdp:discover"\r\n'
        "MX: 2\r\n"
        "ST: ssdp:all\r\n\r\n"
    ).encode("utf-8")

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.settimeout(2.0)

    try:
        sock.sendto(msg, ("127.0.0.1", 1900))
        data, addr = sock.recvfrom(1024)
        response = data.decode("utf-8")

        print(f"\n[SUCCESS] Received SSDP response from {addr}:")
        print("----------------------------------------")
        print(response.strip())
        print("----------------------------------------")

        if "HTTP/1.1 200 OK" in response and "Location:" in response:
            print("[PASS] Valid 200 OK SSDP discovery response.")
            sys.exit(0)
        else:
            print("[FAIL] Missing expected HTTP/1.1 200 OK or Location header.")
            sys.exit(1)

    except socket.timeout:
        print("\n[FAIL] Request timed out. Is the Rust server running on port 1900?")
        sys.exit(1)
    except Exception as e:
        print(f"\n[ERROR] An error occurred: {e}")
        sys.exit(1)
    finally:
        sock.close()


if __name__ == "__main__":
    main()
