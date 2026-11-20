"""Build the private Python runtime that ships inside the OpenDAM installer.

Produces apps/desktop/src-tauri/resources/preview-engine/:
    python/          Windows *embeddable* CPython + the preview worker's packages
    worker.py        copy of packages/preview-engine/worker.py

Why the embeddable distribution: it is a plain folder. It writes nothing to the
registry, does not touch PATH, is not visible to `py`/`python` on the user's
machine, and its `python312._pth` file pins sys.path to the bundle while ignoring
PYTHONPATH/PYTHONHOME. A Python the user installed themselves is never used or
modified, and it can't leak packages into ours.

Run with any Python 3.9+ that has pip (stdlib only otherwise):
    python tools/bundle-python/bundle_python.py
Wheels are fetched for win_amd64 / CPython 3.12 regardless of the host Python.
"""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

PY_VERSION = "3.12.10"
PY_TAG = "312"
EMBED_URL = f"https://www.python.org/ftp/python/{PY_VERSION}/python-{PY_VERSION}-embed-amd64.zip"
# SHA-256 of the zip above. When pinned, the file's MD5 (fe8ef205f2e9c3ba44d0cf9954e1abd3)
# matched the one published on https://www.python.org/downloads/release/python-31210/.
# 3.12.10 is the last 3.12 release with Windows binaries; later 3.12.x are source-only.
EMBED_SHA256 = "4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3"

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
CACHE = HERE / ".cache"
OUT = REPO / "apps" / "desktop" / "src-tauri" / "resources" / "preview-engine"
WORKER_SRC = REPO / "packages" / "preview-engine" / "worker.py"
REQS = HERE / "requirements-bundle.txt"

# Modules the worker imports; each must import inside the bundle.
SMOKE_IMPORTS = [
    "PIL", "olefile", "numpy", "trimesh", "networkx", "pyglet", "collada", "dateutil",
    "tzdata", "assimp_py", "rhino3dm", "pypdfium2", "imageio", "OpenEXR", "openexr_numpy",
]


def log(msg: str) -> None:
    print(f"[bundle-python] {msg}", flush=True)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fetch_embed_zip() -> Path:
    CACHE.mkdir(exist_ok=True)
    zip_path = CACHE / Path(EMBED_URL).name
    if not zip_path.exists():
        log(f"downloading {EMBED_URL}")
        tmp = zip_path.with_suffix(".part")
        with urllib.request.urlopen(EMBED_URL, timeout=120) as r, tmp.open("wb") as f:
            shutil.copyfileobj(r, f)
        tmp.replace(zip_path)
    digest = sha256(zip_path)
    if digest != EMBED_SHA256:
        zip_path.unlink()
        sys.exit(f"SHA-256 mismatch for {zip_path.name}: got {digest}, expected {EMBED_SHA256}")
    log(f"{zip_path.name} sha256={digest}")
    return zip_path


def write_pth(py_dir: Path) -> None:
    # Exactly these entries on sys.path, nothing from the environment or the user's
    # site-packages. `import site` stays disabled so no .pth hooks run either.
    pth = py_dir / f"python{PY_TAG}._pth"
    pth.write_text(
        f"python{PY_TAG}.zip\n.\nLib\\site-packages\n# import site is intentionally disabled\n",
        encoding="utf-8",
    )


def install_packages(site_packages: Path) -> None:
    site_packages.mkdir(parents=True, exist_ok=True)
    cmd = [
        sys.executable, "-m", "pip", "install",
        "--target", str(site_packages),
        "--no-deps",  # requirements-bundle.txt is a complete, pinned lock list
        "--only-binary=:all:",
        "--platform", "win_amd64",
        "--python-version", PY_TAG[0] + "." + PY_TAG[1:],
        "--implementation", "cp",
        "--no-compile",
        "--no-warn-script-location",
        "--disable-pip-version-check",
        "--no-user",
        "-r", str(REQS),
    ]
    log("installing worker packages")
    subprocess.run(cmd, check=True)
    # Console-script launchers and bytecode caches are never used by the worker.
    for junk in ("bin", "Scripts"):
        shutil.rmtree(site_packages / junk, ignore_errors=True)
    for cache in site_packages.rglob("__pycache__"):
        shutil.rmtree(cache, ignore_errors=True)


def runtime_matches(py_dir: Path) -> bool:
    """Reuse a complete private runtime from a previous release build when versions match."""
    site_packages = py_dir / "Lib" / "site-packages"
    if not (py_dir / "python.exe").is_file() or not site_packages.is_dir():
        return False
    import importlib.metadata

    normalize = lambda s: s.lower().replace("_", "-").replace(".", "-")
    installed = {
        normalize(d.metadata.get("Name", "")): d.version
        for d in importlib.metadata.distributions(path=[str(site_packages)])
    }
    for line in REQS.read_text(encoding="utf-8").splitlines():
        requirement = line.split("#", 1)[0].strip()
        if not requirement:
            continue
        name, version = (part.strip() for part in requirement.split("==", 1))
        if installed.get(normalize(name)) != version:
            return False
    return True


def smoke_test(py_dir: Path) -> None:
    """Prove the bundle is isolated and complete: poison the environment, then check
    that sys.path only points inside the bundle and every worker import resolves."""
    python = py_dir / "python.exe"
    with tempfile.TemporaryDirectory() as decoy:
        env = dict(os.environ)
        env["PYTHONPATH"] = decoy
        env["PYTHONHOME"] = decoy
        env["PYTHONUSERBASE"] = decoy
        code = (
            "import sys, json, importlib\n"
            f"mods = {SMOKE_IMPORTS!r}\n"
            "bad = []\n"
            "for m in mods:\n"
            "    try:\n"
            "        importlib.import_module(m)\n"
            "    except Exception as e:\n"
            "        bad.append(f'{m}: {e}')\n"
            "print(json.dumps({'path': sys.path, 'bad': bad, 'prefix': sys.prefix}))\n"
        )
        out = subprocess.run(
            [str(python), "-I", "-B", "-c", code],
            env=env, cwd=decoy, capture_output=True, text=True, check=True,
        ).stdout
    import json

    info = json.loads(out.strip().splitlines()[-1])
    root = str(py_dir.resolve()).lower()
    outside = [p for p in info["path"] if not str(Path(p).resolve()).lower().startswith(root)]
    if outside:
        sys.exit(f"bundle sys.path escapes the bundle: {outside}")
    if info["bad"]:
        sys.exit("bundle is missing modules:\n  " + "\n  ".join(info["bad"]))
    log(f"smoke test ok: sys.path={info['path']}")


def dir_size_mb(path: Path) -> float:
    return sum(f.stat().st_size for f in path.rglob("*") if f.is_file()) / 1e6


def main() -> None:
    zip_path = fetch_embed_zip()
    py_dir = OUT / "python"
    if runtime_matches(py_dir):
        log("reusing bundled Python with matching pinned package versions")
    else:
        if OUT.exists():
            shutil.rmtree(OUT)
        py_dir.mkdir(parents=True)
        with zipfile.ZipFile(zip_path) as z:
            z.extractall(py_dir)
        write_pth(py_dir)
        install_packages(py_dir / "Lib" / "site-packages")
    shutil.copy2(WORKER_SRC, OUT / "worker.py")
    smoke_test(py_dir)
    files = sum(1 for f in OUT.rglob("*") if f.is_file())
    log(f"done: {OUT} ({dir_size_mb(OUT):.0f} MB, {files} files)")


if __name__ == "__main__":
    main()
