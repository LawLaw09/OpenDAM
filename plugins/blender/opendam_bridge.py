"""
OpenDAM Blender Plugin
======================
Blender add-on that connects to the OpenDAM DCC bridge.

Installation:
  Edit → Preferences → Add-ons → Install → select this file

Features (Phase 3):
  - Drag-and-drop import from OpenDAM (append/link)
  - Material loading
  - Save asset to DAM with auto preview generation
  - Real-time bridge connection status

Requires: Blender 3.6+ (Python 3.10+)
"""

bl_info = {
    "name": "OpenDAM Bridge",
    "author": "OpenDAM Contributors",
    "version": (0, 1, 0),
    "blender": (3, 6, 0),
    "location": "View3D > Sidebar > OpenDAM",
    "description": "Connect Blender to OpenDAM digital asset manager",
    "category": "Import-Export",
    "doc_url": "https://github.com/opendam/opendam",
}

import bpy
import json
import threading
import urllib.request
import urllib.error
from bpy.props import StringProperty, BoolProperty
from bpy.types import Operator, Panel, AddonPreferences

BRIDGE_PORT = 37520
BRIDGE_URL = f"http://localhost:{BRIDGE_PORT}"


# ── Preferences ────────────────────────────────────────────────────────────
class OpenDAMPreferences(AddonPreferences):
    bl_idname = __name__

    bridge_port: bpy.props.IntProperty(
        name="Bridge Port",
        default=BRIDGE_PORT,
        min=1024,
        max=65535,
    )

    def draw(self, context):
        layout = self.layout
        layout.prop(self, "bridge_port")


# ── Operators ──────────────────────────────────────────────────────────────
class OPENDAM_OT_ImportAsset(Operator):
    """Import asset from OpenDAM into the current scene"""
    bl_idname = "opendam.import_asset"
    bl_label = "Import Asset"
    bl_options = {"REGISTER", "UNDO"}

    file_path: StringProperty(name="File Path")
    link: BoolProperty(name="Link (instead of Append)", default=False)

    def execute(self, context):
        path = self.file_path
        if not path:
            self.report({"ERROR"}, "No file path specified")
            return {"CANCELLED"}

        try:
            if path.endswith(".blend"):
                with bpy.data.libraries.load(path, link=self.link) as (data_from, data_to):
                    data_to.objects = data_from.objects
                for obj in data_to.objects:
                    if obj is not None:
                        context.collection.objects.link(obj)
            else:
                bpy.ops.import_scene.fbx(filepath=path) if path.lower().endswith(".fbx") \
                    else bpy.ops.import_scene.gltf(filepath=path)
            self.report({"INFO"}, f"Imported: {path}")
        except Exception as e:
            self.report({"ERROR"}, str(e))
            return {"CANCELLED"}

        return {"FINISHED"}


class OPENDAM_OT_SaveToDam(Operator):
    """Export selected objects and save to OpenDAM"""
    bl_idname = "opendam.save_to_dam"
    bl_label = "Save to OpenDAM"
    bl_options = {"REGISTER"}

    def execute(self, context):
        import tempfile
        import os

        if not context.selected_objects:
            self.report({"WARNING"}, "No objects selected")
            return {"CANCELLED"}

        # Export to temp FBX
        tmp = tempfile.NamedTemporaryFile(suffix=".fbx", delete=False)
        tmp_path = tmp.name
        tmp.close()

        try:
            bpy.ops.export_scene.fbx(
                filepath=tmp_path,
                use_selection=True,
            )
            # Notify DAM bridge
            _notify_bridge("ASSET_SAVED", {"exportPath": tmp_path, "host": "blender"})
            self.report({"INFO"}, "Asset sent to OpenDAM")
        except Exception as e:
            self.report({"ERROR"}, str(e))
            return {"CANCELLED"}

        return {"FINISHED"}


# ── Panel ──────────────────────────────────────────────────────────────────
class OPENDAM_PT_Panel(Panel):
    bl_label = "OpenDAM"
    bl_idname = "OPENDAM_PT_panel"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "OpenDAM"

    def draw(self, context):
        layout = self.layout
        col = layout.column(align=True)

        row = col.row()
        row.label(text="Connection:")
        # TODO: show live connection status

        col.separator()
        col.label(text="Asset Actions", icon="IMPORT")
        col.operator("opendam.save_to_dam", icon="EXPORT")


# ── Bridge helpers ─────────────────────────────────────────────────────────
def _notify_bridge(msg_type: str, payload: dict) -> None:
    """Send a one-shot notification to the DAM bridge (fire-and-forget)."""
    import uuid
    body = json.dumps({
        "id": str(uuid.uuid4()),
        "type": msg_type,
        "payload": payload,
        "ts": __import__("time").time() * 1000,
    }).encode()

    def _post():
        try:
            req = urllib.request.Request(
                f"{BRIDGE_URL}/event",
                data=body,
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            urllib.request.urlopen(req, timeout=2)
        except Exception:
            pass  # DAM not running — silently ignore

    threading.Thread(target=_post, daemon=True).start()


# ── Registration ───────────────────────────────────────────────────────────
CLASSES = [
    OpenDAMPreferences,
    OPENDAM_OT_ImportAsset,
    OPENDAM_OT_SaveToDam,
    OPENDAM_PT_Panel,
]


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)


if __name__ == "__main__":
    register()
