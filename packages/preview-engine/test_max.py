import io
import struct
import sys

import olefile
from PIL import Image


def extract_max_thumb(max_file, output_file):
    with olefile.OleFileIO(max_file) as ole:
        if "\x05SummaryInformation" in [s[0] for s in ole.listdir()]:
            props = ole.getproperties("\x05SummaryInformation")
            thumb_data = props.get(17)
            if thumb_data:
                # The data is a CF_DIB. The first 4 bytes are usually the header size
                # of the VT_CF type, but olefile strips it?
                # Let's search for the BITMAPINFOHEADER (size 40 = 0x28000000)
                idx = thumb_data.find(b"\x28\x00\x00\x00")
                if idx != -1:
                    dib = thumb_data[idx:]
                    # Create a BMP header (14 bytes)
                    # 'BM' (2) + size (4) + reserved (4) + offset (4)
                    # For a basic DIB without palette, offset is 14 + 40 = 54
                    bmp_header = (
                        b"BM"
                        + struct.pack("<I", len(dib) + 14)
                        + b"\x00\x00\x00\x00"
                        + struct.pack("<I", 54)
                    )
                    try:
                        img = Image.open(io.BytesIO(bmp_header + dib))
                        img.save(output_file)
                        print(f"Saved {output_file}")
                    except Exception as e:
                        print(f"Failed to read image: {e}")
                else:
                    print("DIB header not found")
        else:
            print("SummaryInformation not found")


if __name__ == "__main__":
    extract_max_thumb(sys.argv[1], sys.argv[2])
