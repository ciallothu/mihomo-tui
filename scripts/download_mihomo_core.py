#!/usr/bin/env python3
import gzip
import json
import os
import stat
import sys
import urllib.request
import zipfile
from pathlib import Path


API_URL = "https://api.github.com/repos/MetaCubeX/mihomo/releases/latest"


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: download_mihomo_core.py <rust-target> <output-dir>", file=sys.stderr)
        return 2

    target = sys.argv[1]
    output_dir = Path(sys.argv[2])
    output_dir.mkdir(parents=True, exist_ok=True)

    release = fetch_json(API_URL)
    asset = choose_asset(release["assets"], target)
    if asset is None:
        print(f"no mihomo asset found for {target} in {release['tag_name']}", file=sys.stderr)
        return 1

    archive = download(asset["browser_download_url"])
    binary_name = "mihomo.exe" if "windows" in target else "mihomo"
    binary_path = output_dir / binary_name

    if asset["name"].endswith(".gz"):
        binary_path.write_bytes(gzip.decompress(archive))
    elif asset["name"].endswith(".zip"):
        extract_zip_binary(archive, binary_path)
    else:
        binary_path.write_bytes(archive)

    make_executable(binary_path)
    (output_dir / "mihomo-core-version.txt").write_text(
        f"{release['tag_name']}\n{asset['name']}\n", encoding="utf-8"
    )
    print(f"bundled {asset['name']} as {binary_path}")
    return 0


def fetch_json(url: str) -> dict:
    request = urllib.request.Request(url, headers={"User-Agent": "mihomo-tui-ci"})
    with urllib.request.urlopen(request) as response:
        return json.loads(response.read().decode("utf-8"))


def download(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "mihomo-tui-ci"})
    with urllib.request.urlopen(request) as response:
        return response.read()


def choose_asset(assets: list[dict], target: str) -> dict | None:
    os_name, arch, extension = target_parts(target)
    candidates = []
    for asset in assets:
        name = asset["name"].lower()
        if (
            os_name in name
            and arch in name
            and name.endswith(extension)
            and "android" not in name
        ):
            candidates.append(asset)
    if not candidates:
        return None
    return max(candidates, key=lambda asset: asset_score(asset["name"]))


def target_parts(target: str) -> tuple[str, str, str]:
    if "windows" in target:
        os_name = "windows"
        extension = ".zip"
    elif "apple-darwin" in target:
        os_name = "darwin"
        extension = ".gz"
    else:
        os_name = "linux"
        extension = ".gz"

    if target.startswith("x86_64"):
        arch = "amd64"
    elif target.startswith("aarch64"):
        arch = "arm64"
    elif target.startswith("i686"):
        arch = "386"
    else:
        arch = target.split("-", 1)[0]

    return os_name, arch, extension


def asset_score(name: str) -> int:
    lower = name.lower()
    score = 0
    if "compatible" in lower:
        score += 30
    if "-v1-" in lower:
        score += 20
    for index, go_version in enumerate(["go120", "go121", "go122", "go123", "go124", "go125"], 1):
        if go_version in lower:
            score += index
    return score


def extract_zip_binary(archive: bytes, binary_path: Path) -> None:
    from io import BytesIO

    with zipfile.ZipFile(BytesIO(archive)) as zipped:
        for entry in zipped.infolist():
            lower = entry.filename.lower()
            if lower.endswith("mihomo.exe") or lower.endswith("mihomo"):
                binary_path.write_bytes(zipped.read(entry))
                return
    raise RuntimeError("mihomo executable not found in zip asset")


def make_executable(path: Path) -> None:
    if os.name != "nt":
        mode = path.stat().st_mode
        path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


if __name__ == "__main__":
    raise SystemExit(main())
