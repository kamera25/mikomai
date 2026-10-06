from pathlib import Path
from PyInstaller.utils.hooks import collect_entry_point, collect_submodules, copy_metadata
drawer_data, drawer_imports = collect_entry_point("blockdiag_imagedrawers")
plugin_data, plugin_imports = collect_entry_point("blockdiag_plugins")
node_data, node_imports = collect_entry_point("blockdiag_noderenderer")
root = Path(SPECPATH).parents[1]
assets = root / "mikomai-core" / "assets"
a = Analysis([str(Path(SPECPATH) / "worker.py")], pathex=[str(assets / "network")],
    datas=[(str(assets / "templates"), "templates"), (str(assets / "fonts"), "fonts")] + drawer_data + plugin_data + node_data + copy_metadata("nwdiag", recursive=True),
    hiddenimports=["netmiko_patches", "config_helper", "nwdiag_wrapper", "nwdiag", "blockdiag", "fontTools.subset"] + drawer_imports + plugin_imports + node_imports + collect_submodules("fontTools.ttLib.tables"),
    hookspath=[], hooksconfig={}, runtime_hooks=[], excludes=[], noarchive=False)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, a.binaries, a.datas, [], name="mikomai-device-worker-macos-arm64",
    console=True, target_arch="arm64", strip=False, upx=False)
