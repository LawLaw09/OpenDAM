use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: String,
    pub name: String,
    #[serde(rename = "rootPaths")]
    pub root_paths: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "assetCount")]
    pub asset_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Model3d,
    Material,
    Texture,
    Hdri,
    Ies,
    Video,
    Image,
    Document,
    Other,
}

impl AssetKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "fbx" | "obj" | "3ds" | "dae" | "gltf" | "glb" | "skp" | "blend"
            | "c4d" | "3dm" | "rfa" | "rvt" | "dwg" | "dxf" | "stl" | "usd" | "usdz" | "max"
            | "abc" | "ply" | "x3d" | "wrl" => AssetKind::Model3d,

            "mat" | "vrmat" | "sbsar" | "sbs" | "mdl" => AssetKind::Material,

            "jpg" | "jpeg" | "png" | "tga" | "tif" | "tiff" | "bmp" | "dds"
            | "ktx" | "ktx2" | "webp" | "avif" => AssetKind::Texture,

            "hdr" | "exr" | "hdri" => AssetKind::Hdri,

            "ies" | "ldt" => AssetKind::Ies,

            "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => AssetKind::Video,

            "psd" | "psb" | "ai" | "svg" | "gif" => AssetKind::Image,

            "pdf" | "txt" | "md" | "rtf" | "docx" => AssetKind::Document,

            _ => AssetKind::Other,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AssetKind::Model3d   => "3d_model",
            AssetKind::Material  => "material",
            AssetKind::Texture   => "texture",
            AssetKind::Hdri      => "hdri",
            AssetKind::Ies       => "ies",
            AssetKind::Video     => "video",
            AssetKind::Image     => "image",
            AssetKind::Document  => "document",
            AssetKind::Other     => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "3d_model"  => AssetKind::Model3d,
            "material"  => AssetKind::Material,
            "texture"   => AssetKind::Texture,
            "hdri"      => AssetKind::Hdri,
            "ies"       => AssetKind::Ies,
            "video"     => AssetKind::Video,
            "image"     => AssetKind::Image,
            "document"  => AssetKind::Document,
            _           => AssetKind::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    pub extension: String,
    pub kind: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: i64,
    #[serde(rename = "modifiedAt")]
    pub modified_at: i64,
    #[serde(rename = "indexedAt")]
    pub indexed_at: i64,
    pub rating: i64,
    #[serde(rename = "colorLabel")]
    pub color_label: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(rename = "thumbnailPath")]
    pub thumbnail_path: Option<String>,
    #[serde(rename = "previewStatus")]
    pub preview_status: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(rename = "assetIds")]
    pub asset_ids: Vec<String>,
    #[serde(rename = "isSmart")]
    pub is_smart: bool,
    #[serde(rename = "filterSpec")]
    pub filter_spec: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewJob {
    pub id: String,
    #[serde(rename = "assetId")]
    pub asset_id: String,
    pub status: String,
    pub progress: Option<i64>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

// ── Command payload types ──────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct AddLibraryPayload {
    pub name: String,
    #[serde(rename = "rootPaths")]
    pub root_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchPayload {
    pub query: SearchQuery,
    pub page: i64,
    #[serde(rename = "pageSize")]
    pub page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub filter: FilterSpec,
    pub sort: SortSpec,
    #[serde(rename = "libraryId")]
    pub library_id: Option<String>,
    #[serde(rename = "collectionId")]
    pub collection_id: Option<String>,
    #[serde(rename = "includeSubcollections")]
    pub include_subcollections: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FilterSpec {
    pub kinds: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub rating: Option<RatingRange>,
    #[serde(rename = "colorLabels")]
    pub color_labels: Option<Vec<String>>,
    pub extensions: Option<Vec<String>>,
    pub unorganized: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RatingRange {
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Deserialize)]
pub struct SortSpec {
    pub field: String,
    pub order: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub assets: Vec<Asset>,
    pub total: i64,
    pub page: i64,
    #[serde(rename = "pageSize")]
    pub page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct AssetPatch {
    pub rating: Option<i64>,
    #[serde(rename = "colorLabel")]
    pub color_label: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "thumbnailPath")]
    pub thumbnail_path: Option<String>,
    #[serde(rename = "previewStatus")]
    pub preview_status: Option<String>,
}
