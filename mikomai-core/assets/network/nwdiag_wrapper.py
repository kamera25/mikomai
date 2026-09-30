import sys
import types

# Ensure pkg_resources shim for Python 3.12+ / setuptools >= 70
try:
    import pkg_resources
except ImportError:
    try:
        import importlib.metadata as importlib_metadata
        shim = types.ModuleType("pkg_resources")
        def iter_entry_points(group, name=None):
            eps = importlib_metadata.entry_points()
            if hasattr(eps, "select"):
                matched = eps.select(group=group)
            else:
                matched = eps.get(group, [])
            if name:
                matched = [ep for ep in matched if ep.name == name]
            return matched
        shim.iter_entry_points = iter_entry_points
        shim.resource_filename = lambda package, resource: ""
        shim.resource_string = lambda package, resource: b""
        shim.Requirement = type("Requirement", (), {"parse": staticmethod(lambda s: s)})
        sys.modules["pkg_resources"] = shim
    except Exception:
        pass

from PIL import ImageFont

# Monkey-patch FreeTypeFont.getsize for Pillow >= 10 compatibility
if not hasattr(ImageFont.FreeTypeFont, 'getsize'):
    def getsize(self, text, *args, **kwargs):
        bbox = self.getbbox(text, *args, **kwargs)
        if bbox is None:
            return (0, 0)
        return (bbox[2] - bbox[0], bbox[3] - bbox[1])
    ImageFont.FreeTypeFont.getsize = getsize

from nwdiag.command import main

if __name__ == '__main__':
    sys.exit(main())
