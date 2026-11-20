//! Native extraction of the preview images that DCC/CAD applications embed in their
//! own files, so these formats need no Python:
//!
//! - `.max`: 3ds Max, a DIB bitmap in the OLE `SummaryInformation` property set
//! - `.rvt`/`.rfa`: Revit, a PNG inside the OLE `RevitPreview4.0` stream
//! - `.skp`: SketchUp (classic binary format), a PNG near the start of the file
//! - `.psd`/`.psb`: Photoshop, the merged composite image, or the embedded JPEG
//!   thumbnail when the composite is blank (saved without "Maximize Compatibility")
//!
//! Every function returns `Ok(None)` when the file simply has no usable preview, so
//! the caller can fall back to the Python worker. Malformed input is an `Err`, never
//! a panic (release builds abort on panic).

use anyhow::{anyhow, bail, Result};
use image::{DynamicImage, ImageFormat, RgbImage};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Formats whose previews this module can extract.
pub const EMBEDDED_PREVIEW_EXTS: &[&str] = &["max", "rfa", "rvt", "skp", "psd", "psb"];

/// Longest edge we decode a PSD composite at; larger images are subsampled while
/// decoding so memory stays small even for huge print files.
const PSD_TARGET_EDGE: u32 = 1024;
const PSD_MAX_DIMENSION: u32 = 100_000;
const PSD_MAX_RLE_ROWS: usize = 1_000_000;
const PNG_MAX_DECODED_BYTES: usize = 64 << 20;
const OLE_PREVIEW_STREAM_MAX: usize = 32 << 20;

const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

pub fn extract_preview(source: &Path, ext: &str) -> Result<Option<DynamicImage>> {
    match ext.to_ascii_lowercase().as_str() {
        "max" => max_preview(source),
        "rfa" | "rvt" => revit_preview(source),
        "skp" => skp_preview(source),
        "psd" | "psb" => psd_preview(source),
        _ => Ok(None),
    }
}

// ── Little helpers ─────────────────────────────────────────────────────────

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn exceeds_png_limit(bytes: Option<usize>) -> bool {
    match bytes {
        Some(bytes) => bytes > PNG_MAX_DECODED_BYTES,
        None => true,
    }
}

fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn le_u16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

/// Decode the PNG that starts at the first PNG signature in `buf` (trailing bytes are ignored).
fn png_in(buf: &[u8]) -> Result<Option<DynamicImage>> {
    let Some(i) = find(buf, PNG_SIG) else {
        return Ok(None);
    };
    // Check the IHDR before invoking the decoder: it allocates based on dimensions.
    // Embedded preview images are small, so a decoded footprint over 64 MiB is corrupt
    // or hostile input, not a useful thumbnail.
    let Some(header) = buf.get(i + 16..i + 29) else {
        return Ok(None);
    };
    if buf.get(i + 8..i + 12) != Some(&13u32.to_be_bytes())
        || buf.get(i + 12..i + 16) != Some(b"IHDR")
    {
        return Ok(None);
    }
    let w = u32::from_be_bytes(header[0..4].try_into().unwrap());
    let h = u32::from_be_bytes(header[4..8].try_into().unwrap());
    let channels = match header[9] {
        0 | 3 => 1usize,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Ok(None),
    };
    let sample_bytes = (header[8] as usize).div_ceil(8);
    let decoded_bytes = (w as usize)
        .checked_mul(h as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .and_then(|bytes| bytes.checked_mul(sample_bytes.max(1)));
    if w == 0 || h == 0 || w > 8192 || h > 8192 || exceeds_png_limit(decoded_bytes) {
        return Ok(None);
    }
    match image::load_from_memory_with_format(&buf[i..], ImageFormat::Png) {
        Ok(img) => Ok(Some(img)),
        // Some SketchUp files hold a thumbnail whose chunk stream is damaged after the
        // image data (bad CRCs, garbage where IEND should be). Pillow still shows these
        // when told to load truncated images; do the same.
        Err(_) => decode_png_tolerant(&buf[i..]).map(Some),
    }
}

/// Salvage decoder for damaged 8-bit, non-interlaced PNGs: gather the IDAT data from the
/// valid leading chunks, inflate as far as the stream goes (zlib-style, keeping every
/// byte produced before an error), and undo the row filters ourselves. The `png` crate
/// can't be used here because it withholds already-inflated rows when input ends early.
fn decode_png_salvage(data: &[u8]) -> Option<DynamicImage> {
    use std::io::Read as _;
    let mut j = PNG_SIG.len();
    let (mut w, mut h, mut channels) = (0u32, 0u32, 0usize);
    let mut idat = Vec::new();
    while j + 12 <= data.len() {
        let len = u32::from_be_bytes(data[j..j + 4].try_into().ok()?) as usize;
        let kind = &data[j + 4..j + 8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || j + 12 + len > data.len() {
            break;
        }
        let body = &data[j + 8..j + 8 + len];
        match kind {
            b"IHDR" if len >= 13 => {
                w = u32::from_be_bytes(body[0..4].try_into().ok()?);
                h = u32::from_be_bytes(body[4..8].try_into().ok()?);
                let (depth, color, interlace) = (body[8], body[9], body[12]);
                channels = match (depth, color, interlace) {
                    (8, 0, 0) => 1,
                    (8, 4, 0) => 2,
                    (8, 2, 0) => 3,
                    (8, 6, 0) => 4,
                    _ => return None,
                };
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        j += 12 + len;
    }
    let decoded_bytes = (w as usize)
        .checked_mul(h as usize)
        .and_then(|pixels| pixels.checked_mul(channels));
    let decoded_too_large = match decoded_bytes {
        Some(bytes) => bytes > PNG_MAX_DECODED_BYTES,
        None => true,
    };
    if channels == 0
        || w == 0
        || h == 0
        || w > 8192
        || h > 8192
        || decoded_too_large
        || idat.is_empty()
    {
        return None;
    }

    let stride = (w as usize).checked_mul(channels)?;
    let want = stride.checked_add(1)?.checked_mul(h as usize)?;
    let mut raw = Vec::with_capacity(want);
    let mut z = flate2::read::ZlibDecoder::new(&idat[..]);
    let mut chunk = [0u8; 16384];
    while raw.len() < want {
        match z.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
        }
    }
    let rows = (raw.len() / (stride + 1)).min(h as usize);
    if rows == 0 {
        return None;
    }

    let mut pixels = vec![0u8; stride * h as usize];
    for y in 0..rows {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (done, rest) = pixels.split_at_mut(y * stride);
        let prev = if y > 0 {
            Some(&done[(y - 1) * stride..])
        } else {
            None
        };
        let cur = &mut rest[..stride];
        for x in 0..stride {
            let a = if x >= channels {
                cur[x - channels] as i16
            } else {
                0
            };
            let b = prev.map_or(0, |p| p[x] as i16);
            let c = if x >= channels {
                prev.map_or(0, |p| p[x - channels] as i16)
            } else {
                0
            };
            let pred = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => return None,
            };
            cur[x] = src[x].wrapping_add(pred as u8);
        }
    }
    Some(match channels {
        1 => DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, pixels)?),
        2 => DynamicImage::ImageLumaA8(image::GrayAlphaImage::from_raw(w, h, pixels)?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, pixels)?),
        _ => DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, pixels)?),
    })
}

/// Keep the PNG's structurally valid leading chunks, then close it with IEND.
fn repair_png_chunks(data: &[u8]) -> Vec<u8> {
    let mut out = data[..PNG_SIG.len().min(data.len())].to_vec();
    let mut j = PNG_SIG.len();
    while j + 12 <= data.len() {
        let len = u32::from_be_bytes([data[j], data[j + 1], data[j + 2], data[j + 3]]) as usize;
        let kind = &data[j + 4..j + 8];
        let end = j + 12 + len;
        if !kind.iter().all(u8::is_ascii_alphabetic) || end > data.len() || kind == b"IEND" {
            break;
        }
        out.extend_from_slice(&data[j..end]);
        j = end;
    }
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(b"IEND");
    out.extend_from_slice(&0xAE42_6082u32.to_be_bytes()); // CRC of "IEND"
    out
}

/// Decode as many rows as possible, ignoring checksums; rows past the damage stay black.
fn decode_png_tolerant(data: &[u8]) -> Result<DynamicImage> {
    if let Some(img) = decode_png_salvage(data) {
        return Ok(img);
    }
    let repaired = repair_png_chunks(data);
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&repaired[..]));
    decoder.ignore_checksums(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let (w, h) = (reader.info().width, reader.info().height);
    let (color, _) = reader.output_color_type();
    let line = reader
        .output_line_size(w)
        .ok_or_else(|| anyhow!("PNG too large"))?;
    let mut pixels = vec![0u8; line * h as usize];
    let mut rows = 0usize;
    while rows < h as usize {
        match reader.next_row() {
            Ok(Some(row)) => {
                pixels[rows * line..(rows + 1) * line].copy_from_slice(row.data());
                rows += 1;
            }
            Ok(None) | Err(_) => break,
        }
    }
    if rows == 0 || reader.info().interlaced && rows < h as usize {
        bail!("PNG thumbnail unreadable");
    }
    let bad = || anyhow!("PNG buffer size mismatch");
    Ok(match color {
        png::ColorType::Rgba => {
            DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, pixels).ok_or_else(bad)?)
        }
        png::ColorType::Rgb => {
            DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, pixels).ok_or_else(bad)?)
        }
        png::ColorType::GrayscaleAlpha => DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(w, h, pixels).ok_or_else(bad)?,
        ),
        png::ColorType::Grayscale => {
            DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, pixels).ok_or_else(bad)?)
        }
        png::ColorType::Indexed => bail!("unexpanded indexed PNG"),
    })
}

fn read_ole_stream(source: &Path, stream: &str) -> Result<Option<Vec<u8>>> {
    let mut comp = match cfb::open(source) {
        Ok(c) => c,
        Err(_) => return Ok(None), // not an OLE file
    };
    if !comp.is_stream(stream) {
        return Ok(None);
    }
    let mut data = Vec::new();
    comp.open_stream(stream)?
        .take((OLE_PREVIEW_STREAM_MAX + 1) as u64)
        .read_to_end(&mut data)?;
    if data.len() > OLE_PREVIEW_STREAM_MAX {
        bail!(
            "OLE preview stream exceeds {} MiB",
            OLE_PREVIEW_STREAM_MAX >> 20
        );
    }
    Ok(Some(data))
}

// ── Revit ──────────────────────────────────────────────────────────────────

fn revit_preview(source: &Path) -> Result<Option<DynamicImage>> {
    match read_ole_stream(source, "/RevitPreview4.0")? {
        Some(data) => png_in(&data),
        None => Ok(None),
    }
}

// ── SketchUp ───────────────────────────────────────────────────────────────

fn skp_preview(source: &Path) -> Result<Option<DynamicImage>> {
    // Classic .skp files keep the thumbnail PNG in the header, well inside 1 MB.
    let mut buf = Vec::with_capacity(1 << 20);
    std::fs::File::open(source)?
        .take(1 << 20)
        .read_to_end(&mut buf)?;
    png_in(&buf)
}

// ── 3ds Max ────────────────────────────────────────────────────────────────

/// Return the raw value bytes of PIDSI_THUMBNAIL (property 17, type VT_CF) from an OLE
/// `SummaryInformation` property-set stream.
fn summary_thumbnail(stream: &[u8]) -> Option<&[u8]> {
    const PID_THUMBNAIL: u32 = 17;
    const VT_CF: u32 = 71;
    let section = le_u32(stream, 44)? as usize; // first FMTID/offset pair starts at 28
    let nprops = le_u32(stream, section + 4)? as usize;
    for i in 0..nprops.min(1024) {
        let pid = le_u32(stream, section + 8 + i * 8)?;
        if pid != PID_THUMBNAIL {
            continue;
        }
        let value = section + le_u32(stream, section + 12 + i * 8)? as usize;
        if le_u32(stream, value)? != VT_CF {
            return None;
        }
        let size = le_u32(stream, value + 4)? as usize;
        return stream.get(value + 8..value + 8 + size);
    }
    None
}

/// Wrap a packed DIB (BITMAPINFOHEADER + palette + pixels) in a BMP file header.
fn dib_to_bmp(dib: &[u8]) -> Result<DynamicImage> {
    let header_size = le_u32(dib, 0).ok_or_else(|| anyhow!("short DIB"))? as usize;
    if header_size < 40 || header_size > dib.len() {
        bail!("invalid DIB header size");
    }
    let width = le_u32(dib, 4).ok_or_else(|| anyhow!("short DIB dimensions"))? as usize;
    let height =
        (le_u32(dib, 8).ok_or_else(|| anyhow!("short DIB dimensions"))? & 0x7fff_ffff) as usize;
    let decoded_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4));
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || exceeds_png_limit(decoded_bytes)
    {
        bail!("DIB thumbnail exceeds decode limits");
    }
    let bits = le_u16(dib, 14).ok_or_else(|| anyhow!("short DIB"))? as usize;
    let compression = le_u32(dib, 16).unwrap_or(0);
    let clr_used = le_u32(dib, 32).unwrap_or(0) as usize;
    let palette = if clr_used > 0 {
        clr_used
    } else if bits <= 8 {
        1 << bits
    } else {
        0
    };
    // BI_BITFIELDS stores three colour masks after a 40-byte header.
    let masks = if compression == 3 && header_size == 40 {
        12
    } else {
        0
    };
    let off = 14 + header_size + palette * 4 + masks;

    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&(off as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    Ok(image::load_from_memory_with_format(&bmp, ImageFormat::Bmp)?)
}

fn max_preview(source: &Path) -> Result<Option<DynamicImage>> {
    let Some(stream) = read_ole_stream(source, "/\u{5}SummaryInformation")? else {
        return Ok(None);
    };
    let Some(cf) = summary_thumbnail(&stream) else {
        return Ok(None);
    };
    // Clipboard data: a format tag and id, then the DIB. Locate the 40-byte
    // BITMAPINFOHEADER the same way the Python worker did.
    match find(cf, &[0x28, 0, 0, 0]) {
        Some(i) => Ok(Some(dib_to_bmp(&cf[i..])?)),
        None => Ok(None),
    }
}

// ── Photoshop ──────────────────────────────────────────────────────────────

struct PsdReader<R> {
    r: R,
    psb: bool,
}

impl<R: Read + Seek> PsdReader<R> {
    fn u8(&mut self) -> Result<u8> {
        let mut b = [0u8; 1];
        self.r.read_exact(&mut b)?;
        Ok(b[0])
    }
    fn u16(&mut self) -> Result<u16> {
        let mut b = [0u8; 2];
        self.r.read_exact(&mut b)?;
        Ok(u16::from_be_bytes(b))
    }
    fn u32(&mut self) -> Result<u32> {
        let mut b = [0u8; 4];
        self.r.read_exact(&mut b)?;
        Ok(u32::from_be_bytes(b))
    }
    /// Section lengths and RLE row counts are 64/32-bit in PSB, 32/16-bit in PSD.
    fn len_long(&mut self) -> Result<u64> {
        if self.psb {
            let mut b = [0u8; 8];
            self.r.read_exact(&mut b)?;
            Ok(u64::from_be_bytes(b))
        } else {
            Ok(self.u32()? as u64)
        }
    }
    fn rle_count(&mut self) -> Result<usize> {
        Ok(if self.psb {
            self.u32()? as usize
        } else {
            self.u16()? as usize
        })
    }
    fn skip(&mut self, n: u64) -> Result<()> {
        self.r.seek(SeekFrom::Current(n as i64))?;
        Ok(())
    }
}

/// PackBits decode of one row into `out` (exactly `out.len()` bytes).
fn unpack_bits(src: &[u8], out: &mut [u8]) -> Result<()> {
    let (mut i, mut o) = (0usize, 0usize);
    while o < out.len() {
        let n = *src.get(i).ok_or_else(|| anyhow!("RLE row truncated"))? as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            let chunk = src
                .get(i..i + len)
                .ok_or_else(|| anyhow!("RLE literal truncated"))?;
            let end = (o + len).min(out.len());
            out[o..end].copy_from_slice(&chunk[..end - o]);
            i += len;
            o = end;
        } else if n != -128 {
            let len = (1 - n as isize) as usize;
            let v = *src.get(i).ok_or_else(|| anyhow!("RLE run truncated"))?;
            i += 1;
            let end = (o + len).min(out.len());
            out[o..end].fill(v);
            o = end;
        }
    }
    Ok(())
}

fn psd_preview(source: &Path) -> Result<Option<DynamicImage>> {
    let mut p = PsdReader {
        r: BufReader::new(std::fs::File::open(source)?),
        psb: false,
    };

    let mut sig = [0u8; 4];
    p.r.read_exact(&mut sig)?;
    if &sig != b"8BPS" {
        bail!("not a Photoshop file");
    }
    p.psb = match p.u16()? {
        1 => false,
        2 => true,
        v => bail!("unknown PSD version {}", v),
    };
    p.skip(6)?;
    let channels = p.u16()? as usize;
    let height = p.u32()?;
    let width = p.u32()?;
    let depth = p.u16()?;
    let mode = p.u16()?;

    let color_mode_len = p.u32()? as u64;
    p.skip(color_mode_len)?;

    // Image resources: remember the embedded JPEG thumbnail as a fallback.
    let resources_len = p.u32()? as u64;
    let resources_end = p.r.stream_position()? + resources_len;
    let mut thumb: Option<(u16, Vec<u8>)> = None;
    while p.r.stream_position()? + 12 <= resources_end {
        let mut tag = [0u8; 4];
        p.r.read_exact(&mut tag)?;
        if &tag != b"8BIM" {
            break;
        }
        let id = p.u16()?;
        let name_len = p.u8()? as u64;
        p.skip(name_len + ((name_len + 1) % 2))?; // Pascal string padded to even
        let size = p.u32()? as u64;
        if (id == 1036 || id == 1033) && size > 28 && size < (16 << 20) {
            let mut data = vec![0u8; size as usize];
            p.r.read_exact(&mut data)?;
            thumb = Some((id, data));
            p.skip(size % 2)?;
        } else {
            p.skip(size + size % 2)?;
        }
    }
    p.r.seek(SeekFrom::Start(resources_end))?;

    let layers_len = p.len_long()?;
    p.skip(layers_len)?;

    let composite = decode_psd_composite(&mut p, channels, width, height, depth, mode)
        .ok()
        .flatten()
        .filter(|img| !is_flat(img));
    if let Some(img) = composite {
        return Ok(Some(DynamicImage::ImageRgb8(img)));
    }

    // Composite missing or blank: use Photoshop's own embedded thumbnail.
    match thumb {
        Some((id, data)) => {
            let mut img =
                image::load_from_memory_with_format(&data[28..], ImageFormat::Jpeg)?.to_rgb8();
            if id == 1033 {
                // Photoshop 4 stored this thumbnail as BGR.
                for px in img.pixels_mut() {
                    px.0.swap(0, 2);
                }
            }
            Ok(Some(DynamicImage::ImageRgb8(img)))
        }
        None => Ok(None),
    }
}

/// True if every pixel has the same colour (a blank composite).
fn is_flat(img: &RgbImage) -> bool {
    let mut px = img.pixels();
    match px.next() {
        Some(first) => px.all(|p| p == first),
        None => true,
    }
}

/// Decode the merged composite (8/16-bit RGB or Grayscale, raw or RLE), subsampling to
/// at most `PSD_TARGET_EDGE` on the long side. `Ok(None)` for unsupported variants.
fn decode_psd_composite<R: Read + Seek>(
    p: &mut PsdReader<R>,
    channels: usize,
    width: u32,
    height: u32,
    depth: u16,
    mode: u16,
) -> Result<Option<RgbImage>> {
    const GRAY: u16 = 1;
    const RGB: u16 = 3;
    let needed = match mode {
        RGB if channels >= 3 => 3,
        GRAY if channels >= 1 => 1,
        _ => return Ok(None),
    };
    let bpc = match depth {
        8 => 1usize,
        16 => 2usize,
        _ => return Ok(None),
    };
    if width == 0
        || height == 0
        || width > PSD_MAX_DIMENSION
        || height > PSD_MAX_DIMENSION
        || channels == 0
        || channels > 56
    {
        return Ok(None);
    }

    let step = width.max(height).div_ceil(PSD_TARGET_EDGE).max(1);
    let (ow, oh) = (width.div_ceil(step), height.div_ceil(step));
    let Some(row_bytes) = (width as usize).checked_mul(bpc) else {
        return Ok(None);
    };
    let mut planes = vec![vec![0u8; (ow * oh) as usize]; needed];

    let compression = p.u16()?;
    let Some(total_rows) = channels.checked_mul(height as usize) else {
        return Ok(None);
    };
    if total_rows > PSD_MAX_RLE_ROWS {
        return Ok(None);
    }
    let mut row = vec![0u8; row_bytes];

    let store = |c: usize, y: u32, row: &[u8], planes: &mut [Vec<u8>]| {
        if c >= needed || !y.is_multiple_of(step) {
            return;
        }
        let oy = (y / step) as usize;
        for ox in 0..ow as usize {
            let x = ox * step as usize;
            // 16-bit samples: keep the high byte.
            planes[c][oy * ow as usize + ox] = row[x * bpc];
        }
    };

    match compression {
        0 => {
            for c in 0..needed {
                for y in 0..height {
                    if y.is_multiple_of(step) {
                        p.r.read_exact(&mut row)?;
                        store(c, y, &row, &mut planes);
                    } else {
                        p.skip(row_bytes as u64)?;
                    }
                }
            }
        }
        1 => {
            let mut counts = Vec::with_capacity(total_rows);
            for _ in 0..total_rows {
                let n = p.rle_count()?;
                // PackBits data for a row cannot exceed roughly twice its decoded size.
                // Reject corrupt counts before they can drive an unbounded Vec::resize.
                if n > row_bytes.saturating_mul(2).saturating_add(1024) {
                    return Ok(None);
                }
                counts.push(n);
            }
            let mut packed = Vec::new();
            for c in 0..needed {
                for y in 0..height {
                    let n = counts[c * height as usize + y as usize];
                    if y.is_multiple_of(step) {
                        packed.resize(n, 0);
                        p.r.read_exact(&mut packed)?;
                        unpack_bits(&packed, &mut row)?;
                        store(c, y, &row, &mut planes);
                    } else {
                        p.skip(n as u64)?;
                    }
                }
            }
        }
        _ => return Ok(None), // ZIP-compressed composites are rare; use the thumbnail
    }

    let img = RgbImage::from_fn(ow, oh, |x, y| {
        let i = (y * ow + x) as usize;
        if needed == 3 {
            image::Rgb([planes[0][i], planes[1][i], planes[2][i]])
        } else {
            image::Rgb([planes[0][i]; 3])
        }
    });
    Ok(Some(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("opendam-embedded-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png_bytes(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, image::Rgb(rgb)));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn jpeg_bytes(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, image::Rgb(rgb)));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
        out.into_inner()
    }

    fn close(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter()
            .zip(b)
            .all(|(x, y)| (*x as i16 - y as i16).abs() <= 6)
    }

    #[test]
    fn unpack_bits_handles_literals_runs_and_noops() {
        // literal 3 bytes, run of 4 x 9, no-op (-128), literal 1 byte
        let src = [2u8, 1, 2, 3, (-3i8) as u8, 9, 0x80, 0, 7];
        let mut out = [0u8; 8];
        unpack_bits(&src, &mut out).unwrap();
        assert_eq!(out, [1, 2, 3, 9, 9, 9, 9, 7]);
        assert!(
            unpack_bits(&[5u8, 1], &mut [0u8; 6]).is_err(),
            "truncated input is an error"
        );
    }

    #[test]
    fn skp_png_found_in_binary_header() {
        let dir = scratch("skp");
        let f = dir.join("model.skp");
        let mut data = vec![0xFF, 0xFE, 0xFF, 0x0E];
        data.extend(std::iter::repeat_n(0xAB, 5000));
        data.extend(png_bytes(20, 10, [10, 200, 30]));
        data.extend(std::iter::repeat_n(0xCD, 5000));
        std::fs::write(&f, &data).unwrap();
        let img = extract_preview(&f, "skp").unwrap().unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (20, 10));
        assert_eq!(img.get_pixel(3, 3).0, [10, 200, 30]);

        // Same thumbnail with a corrupted IDAT CRC (seen in real SketchUp files).
        let mut png = png_bytes(20, 10, [10, 200, 30]);
        let idat = find(&png, b"IDAT").unwrap();
        let len = u32::from_be_bytes(png[idat - 4..idat].try_into().unwrap()) as usize;
        png[idat + 4 + len] ^= 0xFF;
        assert!(image::load_from_memory_with_format(&png, ImageFormat::Png).is_err());
        let mut data = vec![0xFF, 0xFE, 0xFF, 0x0E];
        data.extend(png);
        std::fs::write(&f, &data).unwrap();
        let img = extract_preview(&f, "skp").unwrap().unwrap().to_rgb8();
        assert_eq!(img.get_pixel(3, 3).0, [10, 200, 30]);

        std::fs::write(&f, vec![0u8; 4096]).unwrap();
        assert!(extract_preview(&f, "skp").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_png_is_salvaged_pixel_exact() {
        // Busy pattern so the encoder's adaptive filtering uses several filter types.
        for (w, h, rgba) in [(67u32, 41u32, false), (33, 20, true)] {
            let src = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(w, h, |x, y| {
                image::Rgba([
                    (x * 7 + y * 3) as u8,
                    (x * x + y) as u8,
                    (y * 11) as u8 ^ (x as u8),
                    255 - (x as u8),
                ])
            }));
            let src = if rgba {
                src
            } else {
                DynamicImage::ImageRgb8(src.to_rgb8())
            };
            let mut out = std::io::Cursor::new(Vec::new());
            src.write_to(&mut out, ImageFormat::Png).unwrap();
            let good = out.into_inner();
            // Re-chunk the image data into small IDATs, then corrupt the header of the
            // last one so the tail of the compressed stream is lost: the same damage as
            // the real SketchUp thumbnails (valid IDATs, then garbage).
            let ihdr_end = PNG_SIG.len() + 12 + 13;
            let mut zdata = Vec::new();
            let mut j = ihdr_end;
            while j + 12 <= good.len() {
                let len = u32::from_be_bytes(good[j..j + 4].try_into().unwrap()) as usize;
                if &good[j + 4..j + 8] == b"IDAT" {
                    zdata.extend_from_slice(&good[j + 8..j + 8 + len]);
                }
                j += 12 + len;
            }
            let mut png = good[..ihdr_end].to_vec();
            let pieces: Vec<&[u8]> = zdata.chunks(zdata.len().div_ceil(6)).collect();
            for (k, piece) in pieces.iter().enumerate() {
                let mut chunk = Vec::new();
                chunk.extend_from_slice(&(piece.len() as u32).to_be_bytes());
                chunk.extend_from_slice(if k + 1 == pieces.len() {
                    b"\xE7\x00\x00 "
                } else {
                    b"IDAT"
                });
                chunk.extend_from_slice(piece);
                chunk.extend_from_slice(&[0, 0, 0, 0]); // wrong CRC as well
                png.extend(chunk);
            }
            assert!(image::load_from_memory_with_format(&png, ImageFormat::Png).is_err());

            let got = png_in(&png).unwrap().unwrap().to_rgba8();
            let want = src.to_rgba8();
            assert_eq!(got.dimensions(), (w, h));
            let row = (w * 4) as usize;
            let intact = (0..h as usize)
                .take_while(|&y| {
                    got.as_raw()[y * row..(y + 1) * row] == want.as_raw()[y * row..(y + 1) * row]
                })
                .count();
            // 5 of 6 pieces survive, so most rows must come back exactly.
            assert!(
                intact * 10 >= h as usize * 6,
                "{}x{} rgba={}: only {} rows intact",
                w,
                h,
                rgba,
                intact
            );
        }
    }

    #[test]
    fn revit_png_from_ole_stream() {
        let dir = scratch("rvt");
        let f = dir.join("family.rfa");
        {
            let mut comp = cfb::create(&f).unwrap();
            let mut s = comp.create_stream("/RevitPreview4.0").unwrap();
            s.write_all(&[0x62, 0x19, 0x22, 0x05, 1, 2, 3, 4]).unwrap(); // header junk
            s.write_all(&png_bytes(16, 12, [200, 100, 50])).unwrap();
        }
        let img = extract_preview(&f, "rfa").unwrap().unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (16, 12));
        assert_eq!(img.get_pixel(1, 1).0, [200, 100, 50]);

        // Not an OLE file at all → no preview, no error.
        let g = dir.join("plain.rvt");
        std::fs::write(&g, b"not ole").unwrap();
        assert!(extract_preview(&g, "rvt").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Build a SummaryInformation property set holding a 24-bit DIB as PIDSI_THUMBNAIL.
    fn summary_info_with_dib(w: i32, h: i32, bgr: [u8; 3]) -> Vec<u8> {
        let row = ((w as usize * 3) + 3) & !3;
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&w.to_le_bytes());
        dib.extend_from_slice(&h.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]); // compression .. clrImportant
        for _ in 0..h {
            let mut r: Vec<u8> = (0..w).flat_map(|_| bgr).collect();
            r.resize(row, 0);
            dib.extend(r);
        }
        let mut cf = Vec::new();
        cf.extend_from_slice(&(-1i32).to_le_bytes()); // Windows clipboard format
        cf.extend_from_slice(&8u32.to_le_bytes()); // CF_DIB
        cf.extend(dib);

        let section_off = 48u32;
        let mut s = Vec::new();
        s.extend_from_slice(&[0xFE, 0xFF, 0, 0]); // byte order, version
        s.extend_from_slice(&[0u8; 4 + 16]); // system id, clsid
        s.extend_from_slice(&1u32.to_le_bytes()); // one section
        s.extend_from_slice(&[0u8; 16]); // FMTID
        s.extend_from_slice(&section_off.to_le_bytes());
        // section: size, count, (pid, offset), value
        let value_off = 8 + 8u32;
        let mut sec = Vec::new();
        sec.extend_from_slice(&0u32.to_le_bytes());
        sec.extend_from_slice(&1u32.to_le_bytes());
        sec.extend_from_slice(&17u32.to_le_bytes());
        sec.extend_from_slice(&value_off.to_le_bytes());
        sec.extend_from_slice(&71u32.to_le_bytes()); // VT_CF
        sec.extend_from_slice(&(cf.len() as u32).to_le_bytes());
        sec.extend(cf);
        s.extend(sec);
        s
    }

    #[test]
    fn max_dib_thumbnail_from_summary_information() {
        let dir = scratch("max");
        let f = dir.join("scene.max");
        {
            let mut comp = cfb::create(&f).unwrap();
            let mut s = comp.create_stream("/\u{5}SummaryInformation").unwrap();
            s.write_all(&summary_info_with_dib(7, 5, [30, 60, 220]))
                .unwrap(); // BGR
        }
        let img = extract_preview(&f, "max").unwrap().unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (7, 5));
        assert_eq!(img.get_pixel(2, 2).0, [220, 60, 30]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Minimal 8-bit PSD with an optional 1036 thumbnail resource and a composite in
    /// either raw (0) or RLE (1) encoding.
    fn psd(w: u32, h: u32, planes: [u8; 3], compression: u16, thumb: Option<Vec<u8>>) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"8BPS");
        b.extend_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&[0u8; 6]);
        b.extend_from_slice(&3u16.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&8u16.to_be_bytes());
        b.extend_from_slice(&3u16.to_be_bytes()); // RGB
        b.extend_from_slice(&0u32.to_be_bytes()); // color mode data
        let mut res = Vec::new();
        if let Some(jpeg) = thumb {
            let mut data = vec![0u8; 28];
            data.extend(jpeg);
            res.extend_from_slice(b"8BIM");
            res.extend_from_slice(&1036u16.to_be_bytes());
            res.extend_from_slice(&[0, 0]); // empty name, padded
            res.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let odd = data.len() % 2 == 1;
            res.extend(data);
            if odd {
                res.push(0);
            }
        }
        b.extend_from_slice(&(res.len() as u32).to_be_bytes());
        b.extend(res);
        b.extend_from_slice(&0u32.to_be_bytes()); // layer & mask info
        b.extend_from_slice(&compression.to_be_bytes());
        if compression == 0 {
            for v in planes {
                b.extend(std::iter::repeat_n(v, (w * h) as usize));
            }
        } else {
            // One run per row: (1 - n) = w  =>  n = 1 - w  (w <= 128)
            let row = [(1 - w as i32) as i8 as u8];
            for _ in 0..3 * h {
                b.extend_from_slice(&2u16.to_be_bytes());
            }
            for v in planes {
                for _ in 0..h {
                    b.extend_from_slice(&row);
                    b.push(v);
                }
            }
        }
        b
    }

    #[test]
    fn psd_composite_raw_and_rle() {
        let dir = scratch("psd");
        for comp in [0u16, 1] {
            let f = dir.join(format!("c{}.psd", comp));
            // Two-colour image so it isn't "flat": write a raw one by hand below.
            std::fs::write(&f, psd(40, 30, [250, 120, 10], comp, None)).unwrap();
            // A single-colour composite is treated as blank and there's no thumbnail.
            assert!(
                extract_preview(&f, "psd").unwrap().is_none(),
                "flat composite, comp {}",
                comp
            );
        }
        // Raw composite with a vertical split → real image.
        let (w, h) = (40u32, 30u32);
        let mut b = psd(w, h, [0, 0, 0], 0, None);
        let data_start = b.len() - (3 * w * h) as usize;
        for c in 0..3 {
            for y in 0..h {
                for x in 0..w {
                    let v = if x < w / 2 {
                        [250, 120, 10][c]
                    } else {
                        [5, 5, 200][c]
                    };
                    b[data_start + c * (w * h) as usize + (y * w + x) as usize] = v;
                }
            }
        }
        let f = dir.join("split.psd");
        std::fs::write(&f, &b).unwrap();
        let img = extract_preview(&f, "psd").unwrap().unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (w, h));
        assert_eq!(img.get_pixel(2, 2).0, [250, 120, 10]);
        assert_eq!(img.get_pixel(w - 2, 2).0, [5, 5, 200]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn psd_blank_composite_falls_back_to_embedded_thumbnail() {
        let dir = scratch("psdthumb");
        for comp in [0u16, 1] {
            let f = dir.join(format!("t{}.psd", comp));
            let thumb = jpeg_bytes(16, 9, [30, 140, 230]);
            std::fs::write(&f, psd(40, 30, [255, 255, 255], comp, Some(thumb))).unwrap();
            let img = extract_preview(&f, "psd").unwrap().unwrap().to_rgb8();
            assert_eq!(img.dimensions(), (16, 9));
            assert!(
                close(img.get_pixel(8, 4).0, [30, 140, 230]),
                "{:?}",
                img.get_pixel(8, 4)
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_psd_is_subsampled_while_decoding() {
        let dir = scratch("psdbig");
        let (w, h) = (2500u32, 1200u32);
        let mut b = psd(w, h, [0, 0, 0], 0, None);
        let start = b.len() - (3 * w * h) as usize;
        for (i, v) in b[start..].iter_mut().enumerate() {
            *v = (i % 251) as u8; // not flat
        }
        let f = dir.join("big.psd");
        std::fs::write(&f, &b).unwrap();
        let img = extract_preview(&f, "psd").unwrap().unwrap();
        assert!(img.width() <= PSD_TARGET_EDGE && img.height() <= PSD_TARGET_EDGE);
        assert_eq!((img.width(), img.height()), (w.div_ceil(3), h.div_ceil(3)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn garbage_is_an_error_or_none_never_a_panic() {
        let dir = scratch("garbage");
        for (name, ext) in [
            ("a.psd", "psd"),
            ("b.max", "max"),
            ("c.rvt", "rvt"),
            ("d.skp", "skp"),
        ] {
            let f = dir.join(name);
            std::fs::write(&f, b"8BPS\x00\x01garbage-garbage-garbage").unwrap();
            let _ = extract_preview(&f, ext);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Manual check against real files: OPENDAM_EMBED_SAMPLES="<a>;<b>" cargo test -- --ignored
    #[test]
    #[ignore]
    fn extract_real_files() {
        let Ok(list) = std::env::var("OPENDAM_EMBED_SAMPLES") else {
            return;
        };
        let out = std::env::var("OPENDAM_EMBED_OUT").ok();
        for s in list.split(';').filter(|s| !s.is_empty()) {
            let p = Path::new(s);
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let t = std::time::Instant::now();
            let r = extract_preview(p, &ext);
            let desc = match &r {
                Ok(Some(img)) => format!("{}x{}", img.width(), img.height()),
                Ok(None) => "none".into(),
                Err(e) => format!("ERR {}", e),
            };
            println!(
                "{:<8} {:<12} {:?}  {}",
                ext,
                desc,
                t.elapsed(),
                p.file_name().unwrap().to_string_lossy()
            );
            if let (Some(dir), Ok(Some(img))) = (&out, r) {
                let dest = Path::new(dir)
                    .join(format!("{}.png", p.file_stem().unwrap().to_string_lossy()));
                img.to_rgb8().save(dest).unwrap();
            }
        }
    }
}
