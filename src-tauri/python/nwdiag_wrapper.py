import sys
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
