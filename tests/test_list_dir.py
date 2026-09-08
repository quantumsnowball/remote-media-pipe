import re
import socket
import sys
import urllib.request


def test_ssdp_discovery():
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

        print(f"\n[1/3] SSDP Discovery Response from {addr}:")
        print("----------------------------------------")
        print(response.strip())
        print("----------------------------------------")

        # Extract Location header URL
        match = re.search(r"Location:\s*(http://[^\r\n]+)", response, re.IGNORECASE)
        if not match:
            print("[FAIL] Missing Location header in SSDP response.")
            sys.exit(1)

        location_url = match.group(1).strip()
        print(f"[PASS] Discovered DLNA Location: {location_url}")
        return location_url

    except socket.timeout:
        print("\n[FAIL] SSDP request timed out. Is the Rust server running on port 1900?")
        sys.exit(1)
    finally:
        sock.close()


def test_fetch_root_desc(location_url):
    print(f"\n[2/3] Fetching XML Device Description from {location_url}...")
    try:
        req = urllib.request.Request(location_url)
        with urllib.request.urlopen(req, timeout=3.0) as resp:
            xml_content = resp.read().decode("utf-8")

        print("----------------------------------------")
        print(xml_content.strip())
        print("----------------------------------------")

        if "<root" in xml_content and "Remote Media Pipe" in xml_content:
            print("[PASS] rootDesc.xml fetched and validated.")
        else:
            print("[FAIL] Unexpected rootDesc.xml body structure.")
            sys.exit(1)

    except Exception as e:
        print(f"[FAIL] Failed to fetch rootDesc.xml: {e}")
        sys.exit(1)


def test_browse_content_directory(location_url):
    # Derive control URL endpoint base (http://127.0.0.1:7879/ctl/ContentDirectory)
    base_url = "/".join(location_url.split("/")[:3])
    control_url = f"{base_url}/ctl/ContentDirectory"

    print(f"\n[3/3] Sending SOAP Browse request to {control_url}...")

    soap_body = (
        '<?xml version="1.0"?>'
        '<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" '
        's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">'
        '<s:Body>'
        '<u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">'
        '<ObjectID>0</ObjectID>'
        '<BrowseFlag>BrowseDirectChildren</BrowseFlag>'
        '<Filter>*</Filter>'
        '<StartingIndex>0</StartingIndex>'
        '<RequestedCount>0</RequestedCount>'
        '<SortCriteria></SortCriteria>'
        '</u:Browse>'
        '</s:Body>'
        '</s:Envelope>'
    )

    headers = {
        "Content-Type": 'text/xml; charset="utf-8"',
        "SOAPAction": '"urn:schemas-upnp-org:service:ContentDirectory:1#Browse"',
    }

    try:
        req = urllib.request.Request(
            control_url,
            data=soap_body.encode("utf-8"),
            headers=headers,
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=3.0) as resp:
            response_xml = resp.read().decode("utf-8")

        print("----------------------------------------")
        print(response_xml.strip())
        print("----------------------------------------")

        if "Dummy Movies" in response_xml or "Dummy Shows" in response_xml:
            print("[PASS] Dummy directory listing retrieved successfully!")
        else:
            print("[FAIL] Directory listing missing dummy folders.")
            sys.exit(1)

    except Exception as e:
        print(f"[FAIL] ContentDirectory Browse request failed: {e}")
        sys.exit(1)


def main():
    location_url = test_ssdp_discovery()
    test_fetch_root_desc(location_url)
    test_browse_content_directory(location_url)
    print("\n========================================")
    print(" ALL TESTS PASSED: DLNA PIPE IS WORKING ")
    print("========================================")


if __name__ == "__main__":
    main()
