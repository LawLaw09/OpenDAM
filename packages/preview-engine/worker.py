"""
OpenDAM Preview Engine
======================
Worker process that generates thumbnails and metadata for asset files.
Called by the Tauri backend via a JSON-RPC protocol over stdin/stdout.

Supported operations (Phase 1):
  - image_thumb   → generate thumbnail for common image formats
  - video_thumb   → extract a frame from a video (requires ffmpeg)
  - model_meta    → extract metadata from 3D model files (stub → Phase 2)

Phase 2 will add:
  - 3D thumbnails via assimp/Three.js headless or Blender headless
  - PSD/EXR/HDR via OpenImageIO
  - .max/.mat embedded preview extraction
  - HDRI sphere render
  - Shader-ball material preview
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


# ── Image thumbnails ──────────────────────────────────────────────────────
def image_thumb(source: str, output: str, size: int = 1024) -> dict:
    """Generate a high-resolution thumbnail for a supported image file using Pillow."""
    try:
        from PIL import Image

        Path(output).parent.mkdir(parents=True, exist_ok=True)
        img = Image.open(source)
        if max(img.size) > size:
            img.thumbnail((size, size), Image.LANCZOS)
        if img.mode not in ("RGB", "RGBA"):
            img = img.convert("RGB")
        img.save(output, quality=95)
        return {"ok": True, "output": output}
    except Exception as e:
        return {"ok": False, "error": str(e)}


# ── Video thumbnails ──────────────────────────────────────────────────────
def video_thumb(source: str, output: str, timestamp: str = "00:00:01") -> dict:
    """Extract a frame from a video file using ffmpeg."""
    try:
        Path(output).parent.mkdir(parents=True, exist_ok=True)
        result = subprocess.run(
            [
                "ffmpeg",
                "-ss", timestamp,
                "-i", source,
                "-frames:v", "1",
                "-vf", "scale=720:720:force_original_aspect_ratio=decrease",
                "-y",
                output,
            ],
            capture_output=True,
            timeout=30,
        )
        if result.returncode == 0:
            return {"ok": True, "output": output}
        return {"ok": False, "error": result.stderr.decode()}
    except FileNotFoundError:
        return {"ok": False, "error": "ffmpeg not found — install ffmpeg and add to PATH"}
    except Exception as e:
        return {"ok": False, "error": str(e)}


# ── 3D model metadata & previews ──────────────────────────────────────────────
def model_meta(source: str, output_dir: str) -> dict:
    """
    Extract metadata and preview from a 3D model file.
    Supports: .max, .skp, .rfa, .rvt, .obj, .stl, .ply, .dae, .gltf, .glb, .fbx, .3ds, .dwg, .3dm
    """
    p = Path(source)
    stat = p.stat()
    ext = p.suffix.lower()
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    meta = {
        "size_bytes": stat.st_size,
        "extension": ext,
        "poly_count": None,
        "object_count": None,
        "dimensions": None,
    }
    
    res = {"ok": True, "metadata": meta}
    
    if ext == ".max":
        # Extract embedded thumbnail from 3ds Max OLE storage
        try:
            import olefile
            import struct
            import io
            from PIL import Image, ImageFilter
            
            if olefile.isOleFile(source):
                with olefile.OleFileIO(source) as ole:
                    if '\x05SummaryInformation' in [s[0] for s in ole.listdir()]:
                        props = ole.getproperties('\x05SummaryInformation')
                        thumb_data = props.get(17)
                        if thumb_data:
                            idx = thumb_data.find(b'\x28\x00\x00\x00')
                            if idx != -1:
                                dib = thumb_data[idx:]
                                biWidth, biHeight = struct.unpack_from('<II', dib, 4)
                                biBitCount = struct.unpack_from('<H', dib, 14)[0]
                                colors = 1 << biBitCount if biBitCount <= 8 else 0
                                bfOffBits = 14 + 40 + (colors * 4)
                                bfSize = 14 + len(dib)
                                bmp_header = struct.pack('<2sIHHI', b'BM', bfSize, 0, 0, bfOffBits)
                                img = Image.open(io.BytesIO(bmp_header + dib))
                                w, h = img.size
                                if max(w, h) < 600:
                                    scale = max(1, int(1024 / max(w, h)))
                                    img = img.resize((w * scale, h * scale), Image.LANCZOS)
                                    if img.mode != 'RGB':
                                        img = img.convert('RGB')
                                    img = img.filter(ImageFilter.UnsharpMask(radius=1.2, percent=120, threshold=2))
                                elif img.mode != 'RGB':
                                    img = img.convert('RGB')
                                out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                                img.save(out_path, format="PNG")
                                res["thumbnail"] = str(out_path)
        except Exception as e:
            res["preview_error"] = f"Max extract failed: {e}"
            
    elif ext == ".skp":
        # Extract embedded thumbnail from SketchUp archive (zip for v2021+, binary for classic v3-v2020)
        try:
            import zipfile
            import io
            from PIL import Image, ImageFilter
            if zipfile.is_zipfile(source):
                with zipfile.ZipFile(source, 'r') as z:
                    names = z.namelist()
                    thumb_name = None
                    for candidate in ['meta/model_thumbnail.png', 'meta/preview_thumbnail.png', 'thumbnails/thumbnail.png']:
                        if candidate in names:
                            thumb_name = candidate
                            break
                    if not thumb_name:
                        for name in names:
                            if 'thumb' in name.lower() and (name.endswith('.png') or name.endswith('.jpg')):
                                thumb_name = name
                                break
                    if thumb_name:
                        data = z.read(thumb_name)
                        try:
                            img = Image.open(io.BytesIO(data))
                            w, h = img.size
                            if max(w, h) < 600:
                                scale = max(1, int(1024 / max(w, h)))
                                img = img.resize((w * scale, h * scale), Image.LANCZOS)
                                if img.mode != 'RGB':
                                    img = img.convert('RGB')
                                img = img.filter(ImageFilter.UnsharpMask(radius=1.2, percent=120, threshold=2))
                            elif img.mode != 'RGB':
                                img = img.convert('RGB')
                            out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                            img.save(out_path, format="PNG")
                            res["thumbnail"] = str(out_path)
                        except Exception:
                            out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                            out_path.write_bytes(data)
                            res["thumbnail"] = str(out_path)
            
            # Classic SketchUp binary models (v3 through v2020) embed a PNG thumbnail in the header
            if "thumbnail" not in res:
                with open(source, "rb") as f:
                    buf = f.read(1024 * 1024)
                png_idx = buf.find(b"\x89PNG\r\n\x1a\n")
                if png_idx != -1:
                    try:
                        img = Image.open(io.BytesIO(buf[png_idx:]))
                        w, h = img.size
                        if max(w, h) < 600:
                            scale = max(1, int(1024 / max(w, h)))
                            img = img.resize((w * scale, h * scale), Image.LANCZOS)
                            if img.mode != 'RGB':
                                img = img.convert('RGB')
                            img = img.filter(ImageFilter.UnsharpMask(radius=1.2, percent=120, threshold=2))
                        elif img.mode != 'RGB':
                            img = img.convert('RGB')
                        out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                        img.save(out_path, format="PNG")
                        res["thumbnail"] = str(out_path)
                    except Exception:
                        pass
        except Exception as e:
            res["preview_error"] = f"Skp extract failed: {e}"

    elif ext in [".rfa", ".rvt"]:
        # Extract embedded thumbnail from Revit OLE stream
        try:
            import olefile
            import io
            from PIL import Image, ImageFilter
            if olefile.isOleFile(source):
                with olefile.OleFileIO(source) as ole:
                    if ole.exists('RevitPreview4.0'):
                        stream = ole.openstream('RevitPreview4.0').read()
                        png_idx = stream.find(b'\x89PNG')
                        if png_idx != -1:
                            try:
                                img = Image.open(io.BytesIO(stream[png_idx:]))
                                w, h = img.size
                                if max(w, h) < 600:
                                    scale = max(1, int(1024 / max(w, h)))
                                    img = img.resize((w * scale, h * scale), Image.LANCZOS)
                                    if img.mode != 'RGB':
                                        img = img.convert('RGB')
                                    img = img.filter(ImageFilter.UnsharpMask(radius=1.2, percent=120, threshold=2))
                                elif img.mode != 'RGB':
                                    img = img.convert('RGB')
                                out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                                img.save(out_path, format="PNG")
                                res["thumbnail"] = str(out_path)
                            except Exception:
                                out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                                out_path.write_bytes(stream[png_idx:])
                                res["thumbnail"] = str(out_path)
        except Exception as e:
            res["preview_error"] = f"Revit extract failed: {e}"

    elif ext in [".obj", ".stl", ".ply", ".dae", ".gltf", ".glb"]:
        # Convert to GLB and generate snapshot PNG via Trimesh
        try:
            import trimesh
            scene = trimesh.load(source, force='scene')
            
            out_path = Path(output_dir) / f"{p.stem}_preview.glb"
            scene.export(str(out_path), file_type='glb')
            res["preview_model"] = str(out_path)
            
            try:
                png = scene.save_image(resolution=(1024, 1024))
                if png:
                    thumb_path = Path(output_dir) / f"{p.stem}_thumb.png"
                    thumb_path.write_bytes(png)
                    res["thumbnail"] = str(thumb_path)
            except Exception:
                pass
            
            meta["poly_count"] = sum(len(g.faces) for g in scene.geometry.values() if hasattr(g, 'faces'))
            meta["object_count"] = len(scene.geometry)
        except Exception as e:
            res["preview_error"] = f"3D convert failed: {e}"

    elif ext in [".fbx", ".3ds"]:
        # Convert FBX/3DS via assimp_py and generate GLB + snapshot PNG
        try:
            import assimp_py
            import trimesh
            import numpy as np
            
            ai_scene = assimp_py.import_file(source, assimp_py.Process_Triangulate | assimp_py.Process_GenNormals)
            meshes = []
            poly_count = 0
            for m in ai_scene.meshes:
                v = np.array(m.vertices).reshape(-1, 3)
                f = np.array(m.indices).reshape(-1, 3)
                if len(v) > 0 and len(f) > 0:
                    meshes.append(trimesh.Trimesh(vertices=v, faces=f))
                    poly_count += len(f)
            
            if meshes:
                t_scene = trimesh.Scene(meshes)
                out_path = Path(output_dir) / f"{p.stem}_preview.glb"
                t_scene.export(str(out_path), file_type='glb')
                res["preview_model"] = str(out_path)
                
                try:
                    png = t_scene.save_image(resolution=(1024, 1024))
                    if png:
                        thumb_path = Path(output_dir) / f"{p.stem}_thumb.png"
                        thumb_path.write_bytes(png)
                        res["thumbnail"] = str(thumb_path)
                except Exception:
                    pass
                
                meta["poly_count"] = poly_count
                meta["object_count"] = len(meshes)
            else:
                res["preview_error"] = "No valid meshes in FBX/3DS file"
        except Exception as e:
            res["preview_error"] = f"FBX/3DS convert failed: {e}"


    elif ext == ".vrmat":
        # V-Ray Material file preview
        try:
            import re
            import base64
            import xml.etree.ElementTree as ET
            import io
            from PIL import Image, ImageDraw, ImageFilter
            
            raw_text = p.read_text(encoding="utf-8", errors="ignore")
            thumb_saved = False
            out_path = Path(output_dir) / f"{p.stem}_thumb.jpg"

            def save_thumb(img_or_path) -> bool:
                try:
                    if isinstance(img_or_path, (str, Path)):
                        im = Image.open(img_or_path)
                    else:
                        im = img_or_path
                    if im.mode not in ("RGB", "RGBA"):
                        im = im.convert("RGB")
                    w, h = im.size
                    if max(w, h) > 1024:
                        im.thumbnail((1024, 1024), Image.LANCZOS)
                    elif max(w, h) < 600:
                        scale = max(1, int(1024 / max(w, h)))
                        if scale > 1:
                            im = im.resize((w * scale, h * scale), Image.LANCZOS)
                    if im.mode != "RGB":
                        im = im.convert("RGB")
                    im.save(out_path, format="JPEG", quality=95)
                    res["thumbnail"] = str(out_path)
                    return True
                except Exception:
                    return False

            # 1. Direct companion preview image in the same directory
            for ext_cand in [".jpg", ".jpeg", ".png", ".webp"]:
                cand = p.with_suffix(ext_cand)
                if cand.exists() and cand.is_file():
                    thumb_saved = save_thumb(cand)
                    break
                cand_prev = p.parent / f"{p.stem}_preview{ext_cand}"
                if cand_prev.exists() and cand_prev.is_file():
                    thumb_saved = save_thumb(cand_prev)
                    break
                cand_th = p.parent / f"{p.stem}_thumb{ext_cand}"
                if cand_th.exists() and cand_th.is_file():
                    thumb_saved = save_thumb(cand_th)
                    break

            # 2. Check dedicated preview folders (e.g. Previews/Laminate_01.png)
            if not thumb_saved:
                preview_dirs = [
                    p.parent / "Previews", p.parent / "previews",
                    p.parent / "Preview", p.parent / "preview",
                    p.parent / "Renders", p.parent / "renders",
                    p.parent.parent / "Previews", p.parent.parent / "previews",
                ]
                for pdir in preview_dirs:
                    if pdir.exists() and pdir.is_dir():
                        valid_imgs = [
                            f for f in pdir.iterdir()
                            if f.is_file() and f.suffix.lower() in [".jpg", ".jpeg", ".png", ".webp"]
                        ]
                        if valid_imgs:
                            stem_lower = p.stem.lower()
                            matched = None
                            for f in valid_imgs:
                                fn_lower = f.stem.lower()
                                if fn_lower in stem_lower or stem_lower in fn_lower:
                                    matched = f
                                    break
                            best_img = matched or valid_imgs[0]
                            thumb_saved = save_thumb(best_img)
                            break

            # 3. Embedded base64 preview inside VRMAT XML
            if not thumb_saved:
                m = re.search(r'<preview[^>]*>(.+?)</preview>', raw_text, re.DOTALL | re.IGNORECASE)
                if m:
                    b64_str = m.group(1).strip()
                    if len(b64_str) > 100:
                        try:
                            img_bytes = base64.b64decode(b64_str)
                            im = Image.open(io.BytesIO(img_bytes))
                            thumb_saved = save_thumb(im)
                        except Exception:
                            pass

            # 4. Check referenced textures in XML (Strictly prioritize Diffuse/Color over Normal/Bump/Gloss/Mask)
            if not thumb_saved:
                try:
                    root = ET.fromstring(raw_text)
                    texture_candidates = []

                    for param in root.findall(".//parameter"):
                        pname = (param.get("name") or "").lower()
                        plabel = (param.get("label") or "").lower()
                        if pname in ["file", "bitmap", "filename"] or "texture" in pname or "color" in pname:
                            for val in param.findall(".//value"):
                                if val.text and any(val.text.lower().endswith(im_ext) for im_ext in [".jpg", ".jpeg", ".png", ".tif", ".tiff", ".tga", ".bmp", ".webp"]):
                                    tex_name = Path(val.text.strip()).name
                                    search_paths = [
                                        p.parent / tex_name,
                                        p.parent / "maps" / tex_name,
                                        p.parent / "textures" / tex_name,
                                        p.parent.parent / "maps" / tex_name,
                                        p.parent.parent / "textures" / tex_name,
                                    ]
                                    for sp in search_paths:
                                        if sp.exists() and sp.is_file():
                                            tex_lower = tex_name.lower()
                                            score = 10
                                            if any(k in tex_lower for k in ["diff", "alb", "base", "color", "col", "tex"]):
                                                score += 60
                                            if any(k in pname or k in plabel for k in ["diffuse", "albedo", "color", "base"]):
                                                score += 50
                                            if any(k in tex_lower for k in ["norm", "nrm"]):
                                                score -= 100
                                            if any(k in tex_lower for k in ["bump", "bmp"]):
                                                score -= 80
                                            if any(k in tex_lower for k in ["gloss", "glos", "gl"]):
                                                score -= 70
                                            if any(k in tex_lower for k in ["rough", "rgh"]):
                                                score -= 70
                                            if any(k in tex_lower for k in ["mask", "msk"]):
                                                score -= 60
                                            if any(k in tex_lower for k in ["disp", "height"]):
                                                score -= 60
                                            if any(k in tex_lower for k in ["alpha", "opac"]):
                                                score -= 60
                                            if any(k in tex_lower for k in ["ao", "ambient"]):
                                                score -= 50
                                            if any(k in tex_lower for k in ["refl", "spec"]):
                                                score -= 40

                                            texture_candidates.append((score, sp))
                                            break

                    if texture_candidates:
                        texture_candidates.sort(key=lambda x: x[0], reverse=True)
                        best_tex = texture_candidates[0][1]
                        thumb_saved = save_thumb(best_tex)
                except Exception:
                    pass

            # 5. Fallback: Render a realistic material sphere swatch using color from XML
            if not thumb_saved:
                sphere_color = (80, 110, 160)
                try:
                    color_matches = re.findall(r'<parameter[^>]*name=["\'](?:color|diffuse|diffuse_color|base_color)["\'][^>]*>.*?<r>([0-9.]+)</r>.*?<g>([0-9.]+)</g>.*?<b>([0-9.]+)</b>', raw_text, re.DOTALL | re.IGNORECASE)
                    if color_matches:
                        r, g, b = [float(x) for x in color_matches[0]]
                        sphere_color = (
                            max(15, min(245, int(r * 255))),
                            max(15, min(245, int(g * 255))),
                            max(15, min(245, int(b * 255))),
                        )
                except Exception:
                    pass

                size = 512
                swatch = Image.new("RGB", (size, size), (24, 26, 32))
                draw = ImageDraw.Draw(swatch)
                radius = int(size * 0.38)
                cx, cy = size // 2, size // 2
                light_x, light_y = cx - int(radius * 0.35), cy - int(radius * 0.35)

                sr, sg, sb = sphere_color
                for r_step in range(radius, 0, -2):
                    frac = 1.0 - (r_step / float(radius))
                    cr = min(255, int(sr * 0.45 + (255 - sr * 0.45) * (frac ** 2.2)))
                    cg = min(255, int(sg * 0.45 + (255 - sg * 0.45) * (frac ** 2.2)))
                    cb = min(255, int(sb * 0.45 + (255 - sb * 0.45) * (frac ** 2.2)))
                    step_cx = int(cx + (light_x - cx) * (frac ** 0.8))
                    step_cy = int(cy + (light_y - cy) * (frac ** 0.8))
                    draw.ellipse([step_cx - r_step, step_cy - r_step, step_cx + r_step, step_cy + r_step], fill=(cr, cg, cb))

                swatch = swatch.filter(ImageFilter.SMOOTH)
                swatch.save(out_path, format="JPEG", quality=95)
                res["thumbnail"] = str(out_path)
        except Exception as e:
            res["preview_error"] = f"Vrmat preview failed: {e}"

    elif ext == ".3dm":
        # Rhino 3DM preview (Meshes -> GLB + PNG, or CAD curve vector blueprint)
        try:
            import rhino3dm
            from PIL import Image, ImageDraw
            import trimesh
            import numpy as np
            
            doc = rhino3dm.File3dm.Read(source)
            if not doc:
                res["preview_error"] = "Failed to parse 3DM archive"
            else:
                t_meshes = []
                poly_count = 0
                for obj in doc.Objects:
                    geo = obj.Geometry
                    if isinstance(geo, rhino3dm.Mesh):
                        v = np.array([[pt.X, pt.Y, pt.Z] for pt in geo.Vertices])
                        f = []
                        for face in geo.Faces:
                            f.append([face[0], face[1], face[2]])
                            if face[2] != face[3]:
                                f.append([face[0], face[2], face[3]])
                        if len(v) > 0 and len(f) > 0:
                            t_meshes.append(trimesh.Trimesh(vertices=v, faces=np.array(f)))
                            poly_count += len(f)
                    elif hasattr(geo, "GetMesh"):
                        m = geo.GetMesh(rhino3dm.MeshType.Any)
                        if m:
                            v = np.array([[pt.X, pt.Y, pt.Z] for pt in m.Vertices])
                            f = []
                            for face in m.Faces:
                                f.append([face[0], face[1], face[2]])
                                if face[2] != face[3]:
                                    f.append([face[0], face[2], face[3]])
                            if len(v) > 0 and len(f) > 0:
                                t_meshes.append(trimesh.Trimesh(vertices=v, faces=np.array(f)))
                                poly_count += len(f)

                if t_meshes:
                    t_scene = trimesh.Scene(t_meshes)
                    out_glb = Path(output_dir) / f"{p.stem}_preview.glb"
                    t_scene.export(str(out_glb), file_type='glb')
                    res["preview_model"] = str(out_glb)
                    try:
                        png = t_scene.save_image(resolution=(1024, 1024))
                        if png:
                            thumb_path = Path(output_dir) / f"{p.stem}_thumb.png"
                            thumb_path.write_bytes(png)
                            res["thumbnail"] = str(thumb_path)
                    except Exception:
                        pass
                    meta["poly_count"] = poly_count
                    meta["object_count"] = len(t_meshes)
                else:
                    # 2D CAD architectural curve blueprint
                    bbox = doc.Objects.GetBoundingBox()
                    lines = []
                    for i in range(len(doc.Objects)):
                        geo = doc.Objects[i].Geometry
                        if isinstance(geo, rhino3dm.Curve):
                            t0, t1 = geo.Domain.T0, geo.Domain.T1
                            n_pts = 2 if isinstance(geo, rhino3dm.LineCurve) else 16
                            pts = [geo.PointAt(t0 + (t1 - t0) * s / (n_pts - 1)) for s in range(n_pts)]
                            lines.append([(pt.X, pt.Y) for pt in pts])
                        elif isinstance(geo, rhino3dm.Polyline):
                            lines.append([(pt.X, pt.Y) for pt in geo])

                    if lines:
                        size = 1024
                        img = Image.new('RGB', (size, size), (22, 24, 30))
                        draw = ImageDraw.Draw(img)
                        bw = bbox.Max.X - bbox.Min.X
                        bh = bbox.Max.Y - bbox.Min.Y
                        scale = (size - 64) / max(bw, bh) if max(bw, bh) > 0 else 1
                        cx = (bbox.Min.X + bbox.Max.X) / 2
                        cy = (bbox.Min.Y + bbox.Max.Y) / 2
                        for poly in lines:
                            s_pts = [(int(size/2 + (x - cx)*scale), int(size/2 - (y - cy)*scale)) for x, y in poly]
                            if len(s_pts) > 1:
                                draw.line(s_pts, fill=(100, 180, 255), width=3)
                        out_thumb = Path(output_dir) / f"{p.stem}_thumb.png"
                        img.save(out_thumb)
                        res["thumbnail"] = str(out_thumb)
                        meta["object_count"] = len(doc.Objects)
                    else:
                        res["preview_error"] = "No renderable geometry found in 3DM file"
        except Exception as e:
            res["preview_error"] = f"3DM preview failed: {e}"

    elif ext == ".dwg":
        # AutoCAD DWG embedded preview extraction
        try:
            import io
            import struct
            from PIL import Image, ImageFilter

            # Read up to first 8MB (DWG thumbnail is in the header / preview block)
            with open(source, "rb") as f:
                data = f.read(8 * 1024 * 1024)

            sentinel = bytes([0x1F, 0x25, 0x6D, 0x07, 0xD4, 0x36, 0x28, 0x28, 0x9D, 0x57, 0xCA, 0x3F, 0x9D, 0x44, 0x10, 0x2B])
            idx = data.find(sentinel)
            thumb_extracted = False

            if idx != -1:
                block = data[idx + 16:]
                if len(block) >= 5:
                    total_len, num_images = struct.unpack_from('<IB', block, 0)
                    pos = 5
                    for _ in range(min(num_images, 16)):
                        if pos + 9 > len(block):
                            break
                        type_code, offset, length = struct.unpack_from('<BII', block, pos)
                        pos += 9
                        if offset > 0 and length > 0 and offset + length <= len(data):
                            img_bytes = data[offset:offset + length]
                            try:
                                img = None
                                if img_bytes.startswith(b'\x89PNG'):
                                    img = Image.open(io.BytesIO(img_bytes))
                                elif img_bytes.startswith(b'BM'):
                                    img = Image.open(io.BytesIO(img_bytes))
                                elif len(img_bytes) > 40 and struct.unpack_from('<I', img_bytes, 0)[0] == 40:
                                    biSize, biWidth, biHeight, biPlanes, biBitCount, biCompression, biSizeImage, biXPels, biYPels, biClrUsed, biClrImp = struct.unpack_from('<IIIHHIIIIII', img_bytes, 0)
                                    colors = biClrUsed if biClrUsed > 0 else (1 << biBitCount if biBitCount <= 8 else 0)
                                    bfOffBits = 14 + biSize + (colors * 4)
                                    bfSize = 14 + len(img_bytes)
                                    bmp_header = struct.pack('<2sIHHI', b'BM', bfSize, 0, 0, bfOffBits)
                                    img = Image.open(io.BytesIO(bmp_header + img_bytes))

                                if img is not None:
                                    w, h = img.size
                                    target_dim = 1024
                                    scale = max(1, int(target_dim / max(w, h)))
                                    if scale > 1:
                                        img = img.resize((w * scale, h * scale), Image.LANCZOS)
                                        if img.mode != 'RGB':
                                            img = img.convert('RGB')
                                        img = img.filter(ImageFilter.UnsharpMask(radius=1.2, percent=120, threshold=2))
                                    elif max(w, h) > 1440:
                                        img.thumbnail((1440, 1440), Image.LANCZOS)
                                        if img.mode != 'RGB':
                                            img = img.convert('RGB')
                                    elif img.mode != 'RGB':
                                        img = img.convert('RGB')

                                    out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                                    img.save(out_path, format="PNG")
                                    res["thumbnail"] = str(out_path)
                                    thumb_extracted = True
                                    break
                            except Exception:
                                pass

            # Fallback search if sentinel parsing didn't extract thumbnail
            if not thumb_extracted:
                png_idx = data.find(b"\x89PNG")
                if png_idx != -1 and png_idx < 4 * 1024 * 1024:
                    try:
                        img = Image.open(io.BytesIO(data[png_idx:]))
                        w, h = img.size
                        target_dim = 1024
                        scale = max(1, int(target_dim / max(w, h)))
                        if scale > 1:
                            img = img.resize((w * scale, h * scale), Image.LANCZOS)
                            if img.mode != 'RGB':
                                img = img.convert('RGB')
                            img = img.filter(ImageFilter.UnsharpMask(radius=1.2, percent=120, threshold=2))
                        elif max(w, h) > 1440:
                            img.thumbnail((1440, 1440), Image.LANCZOS)
                            if img.mode != 'RGB':
                                img = img.convert('RGB')
                        elif img.mode != 'RGB':
                            img = img.convert('RGB')

                        out_path = Path(output_dir) / f"{p.stem}_thumb.png"
                        img.save(out_path, format="PNG")
                        res["thumbnail"] = str(out_path)
                        thumb_extracted = True
                    except Exception:
                        pass

            if not thumb_extracted:
                res["preview_error"] = "No embedded preview found in DWG file"
        except Exception as e:
            res["preview_error"] = f"DWG preview failed: {e}"


    elif ext in [".psd", ".psb"]:
        # Photoshop PSD/PSB preview
        try:
            from PIL import Image
            img = Image.open(source)
            if max(img.size) > 1440:
                img.thumbnail((1440, 1440), Image.LANCZOS)
            if img.mode != "RGB":
                img = img.convert("RGB")
            out_path = Path(output_dir) / f"{p.stem}_thumb.png"
            img.save(out_path, format="PNG")
            res["thumbnail"] = str(out_path)
        except Exception as e:
            res["preview_error"] = f"PSD preview failed: {e}"

    elif ext == ".ai":
        # Adobe Illustrator preview (via PDF render engine)
        try:
            import pypdfium2 as pdfium
            pdf = pdfium.PdfDocument(source)
            page = pdf[0]
            img = page.render(scale=3).to_pil()
            if max(img.size) > 1440:
                img.thumbnail((1440, 1440), Image.LANCZOS)
            if img.mode != "RGB":
                img = img.convert("RGB")
            out_path = Path(output_dir) / f"{p.stem}_thumb.png"
            img.save(out_path, format="PNG")
            res["thumbnail"] = str(out_path)
        except Exception as e:
            res["preview_error"] = f"Illustrator preview failed: {e}"


    elif ext in [".exr", ".hdr"]:
        # OpenEXR / HDR tone-mapped thumbnail
        try:
            from PIL import Image
            import numpy as np
            if ext == ".exr":
                import openexr_numpy
                try:
                    rgb = openexr_numpy.imread(source)
                except Exception:
                    rgb = openexr_numpy.imread(source, channel_names=['R'])
            else:
                import imageio.v2 as iio
                rgb = iio.imread(source)

            # Fast strided downsample only if image is huge (e.g. 8k, 16k maps)
            max_dim = max(rgb.shape[:2])
            if max_dim > 2048:
                step = max(1, max_dim // 2048)
                rgb = rgb[::step, ::step]

            rgb = np.maximum(rgb, 0)
            mapped = rgb / (rgb + 1.0)
            gamma = np.power(mapped, 1.0 / 2.2)
            u8 = (np.clip(gamma * 255.0, 0, 255)).astype(np.uint8)
            img = Image.fromarray(u8)
            if img.mode != "RGB":
                img = img.convert("RGB")
            if max(img.size) > 1440:
                img.thumbnail((1440, 1440), Image.LANCZOS)
            out_path = Path(output_dir) / f"{p.stem}_thumb.jpg"
            img.save(out_path, format="JPEG", quality=95)
            res["thumbnail"] = str(out_path)
        except Exception as e:
            res["preview_error"] = f"EXR/HDR preview failed: {e}"

    # If neither thumbnail nor preview_model was produced, mark ok=False
    if "thumbnail" not in res and "preview_model" not in res:
        res["ok"] = False
        res["error"] = res.get("preview_error", "No preview available for this format")

    return res


# ── JSON-RPC loop ─────────────────────────────────────────────────────────
DISPATCH = {
    "image_thumb": lambda p: image_thumb(p["source"], p["output"], p.get("size", 256)),
    "video_thumb": lambda p: video_thumb(p["source"], p["output"], p.get("timestamp", "00:00:01")),
    "model_meta":  lambda p: model_meta(p["source"], p.get("output_dir", "")),
}


def main() -> None:
    for raw_line in sys.stdin:
        raw_line = raw_line.strip()
        if not raw_line:
            continue
        try:
            req = json.loads(raw_line)
            method = req.get("method", "")
            params = req.get("params", {})
            req_id = req.get("id")

            if method not in DISPATCH:
                resp = {"id": req_id, "error": f"Unknown method: {method}"}
            else:
                result = DISPATCH[method](params)
                resp = {"id": req_id, "result": result}
        except Exception as e:
            resp = {"id": None, "error": str(e)}

        print(json.dumps(resp), flush=True)


if __name__ == "__main__":
    main()
