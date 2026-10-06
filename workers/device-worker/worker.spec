from pathlib import Path
root = Path(SPECPATH).parents[1]
assets = root / "mikomai-core" / "assets"
a = Analysis([str(Path(SPECPATH) / "worker.py")], pathex=[str(assets / "network")],
    datas=[(str(assets / "templates"), "templates"), (str(assets / "fonts"), "fonts")],
    hiddenimports=["netmiko_patches", "config_helper", "nwdiag_wrapper", "nwdiag", "blockdiag"],
    hookspath=[], hooksconfig={}, runtime_hooks=[], excludes=[], noarchive=False)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, a.binaries, a.datas, [], name="mikomai-device-worker-macos-arm64",
    console=True, target_arch="arm64", strip=False, upx=False)
