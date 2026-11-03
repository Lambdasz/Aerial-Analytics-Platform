"""Unduh sebagian dataset UAV-VisLoc langsung dari share Nextcloud tim.

Dataset lengkapnya sekitar 22 GB, padahal plugin hanya butuh foto drone untuk diberi label
dan dipakai fine-tuning, dan beberapa ratus foto sudah cukup. Share dibaca lewat endpoint
WebDAV publik yang disediakan Nextcloud untuk setiap link share, jadi hanya file yang
dipilih di sini yang diunduh; peta satelit (``satelliteXX.tif``) dilewati.

UAV-VisLoc adalah dataset lokalisasi visual: isinya foto drone, CSV penerbangan, dan peta
satelit, tetapi tanpa label pohon. Label dibuat belakangan dengan ``train_deepforest.py
prelabel`` lalu dikoreksi di alat anotasi.

Cara pakai (hanya pustaka standar Python)::

    python training/fetch_visloc.py --out %USERPROFILE%/datasets/uav-visloc --per-flight 20

Jalankan lagi untuk melanjutkan: file yang sudah ada dengan ukuran yang benar dilewati.
"""

import argparse
import os
import sys
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from base64 import b64encode
from collections.abc import Iterable, Sequence
from typing import NamedTuple

DEFAULT_SHARE_URL = "https://nextcloud.putrahomelab.com/s/y5on5gBHR7n5M3A"
DATASET_DIR = "UAV_VisLoc_dataset"
FLIGHTS = tuple(f"{index:02d}" for index in range(1, 12))
DAV = "{DAV:}"
CHUNK_BYTES = 1 << 20


class RemoteFile(NamedTuple):
    path: str  # relative to the share root, e.g. "UAV_VisLoc_dataset/01/drone/01_0001.JPG"
    size: int


class Share(NamedTuple):
    webdav_root: str  # e.g. "https://host/public.php/webdav"
    authorization: str  # HTTP Basic header value: the share token is the user name


def share_of(share_url: str) -> Share:
    """WebDAV endpoint and credentials of a public link ``https://host/s/<token>``."""
    parsed = urllib.parse.urlparse(share_url)
    parts = [part for part in parsed.path.split("/") if part]
    if len(parts) < 2 or parts[-2] != "s":
        raise ValueError(f"Not a Nextcloud share link: {share_url}")
    token = parts[-1]
    credentials = b64encode(f"{token}:".encode()).decode()
    return Share(f"{parsed.scheme}://{parsed.netloc}/public.php/webdav", f"Basic {credentials}")


def parse_listing(xml_text: str, root_path: str) -> tuple[RemoteFile, ...]:
    """Files (not folders) in a PROPFIND ``Depth: 1`` answer, relative to the share root."""
    files = []
    for response in ET.fromstring(xml_text).iter(f"{DAV}response"):
        href = urllib.parse.unquote(response.findtext(f"{DAV}href", ""))
        length = response.findtext(f".//{DAV}getcontentlength")
        if length is None or href.endswith("/"):
            continue
        relative = href.split(root_path, 1)[-1].lstrip("/")
        files.append(RemoteFile(relative, int(length)))
    return tuple(files)


def evenly_spaced(items: Sequence[RemoteFile], count: int) -> tuple[RemoteFile, ...]:
    """``count`` items spread over the whole flight instead of the first ``count``.

    Consecutive photos overlap heavily, so the first 20 of a flight show nearly the
    same trees; spreading the pick covers more scenes for the same download size.
    """
    if count <= 0 or count >= len(items):
        return tuple(items)
    step = len(items) / count
    return tuple(items[int(index * step)] for index in range(count))


def _request(share: Share, relative: str, method: str = "GET") -> urllib.request.Request:
    url = f"{share.webdav_root}/{urllib.parse.quote(relative)}"
    request = urllib.request.Request(url, method=method)
    request.add_header("Authorization", share.authorization)
    if method == "PROPFIND":
        request.add_header("Depth", "1")
    return request


def list_folder(share: Share, relative: str) -> tuple[RemoteFile, ...]:
    with urllib.request.urlopen(_request(share, relative + "/", "PROPFIND")) as answer:
        body = answer.read().decode("utf-8")
    root_path = urllib.parse.urlparse(share.webdav_root).path
    return parse_listing(body, root_path)


def download(share: Share, remote: RemoteFile, out_dir: str) -> bool:
    """Download one file unless it is already complete. Returns True if it was fetched."""
    target = os.path.join(out_dir, *remote.path.split("/")[1:])  # drop DATASET_DIR
    if os.path.isfile(target) and os.path.getsize(target) == remote.size:
        return False
    os.makedirs(os.path.dirname(target), exist_ok=True)
    partial = target + ".part"
    with urllib.request.urlopen(_request(share, remote.path)) as answer, open(partial, "wb") as out:
        while chunk := answer.read(CHUNK_BYTES):
            out.write(chunk)
    os.replace(partial, target)
    return True


def flight_files(share: Share, flight: str, per_flight: int) -> tuple[RemoteFile, ...]:
    """The flight CSV plus ``per_flight`` drone photos spread over the flight."""
    folder = f"{DATASET_DIR}/{flight}"
    csvs = tuple(f for f in list_folder(share, folder) if f.path.lower().endswith(".csv"))
    photos = sorted(list_folder(share, f"{folder}/drone"), key=lambda f: f.path)
    return csvs + evenly_spaced(photos, per_flight)


def fetch(share: Share, flights: Iterable[str], per_flight: int, out_dir: str) -> None:
    for flight in flights:
        files = flight_files(share, flight, per_flight)
        megabytes = sum(f.size for f in files) / 1e6
        print(f"[{flight}] {len(files)} file, {megabytes:.0f} MB", flush=True)
        for remote in files:
            if download(share, remote, out_dir):
                print(f"  {remote.path}", flush=True)


def main(argv: Sequence[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", required=True, help="folder tujuan, taruh di luar repo")
    parser.add_argument("--share-url", default=DEFAULT_SHARE_URL)
    parser.add_argument("--flights", nargs="+", default=list(FLIGHTS), choices=FLIGHTS)
    parser.add_argument(
        "--per-flight", type=int, default=20, help="foto drone per penerbangan (0 = semua)"
    )
    args = parser.parse_args(argv)

    fetch(share_of(args.share_url), args.flights, args.per_flight, os.path.abspath(args.out))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
