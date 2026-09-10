/**
 * OpenDAM DCC Bridge Protocol
 * ============================
 * WebSocket-based protocol between the DAM desktop app and DCC host app plugins.
 *
 * Transport: WebSocket on localhost:37520 (configurable)
 * Encoding:  JSON
 * Pattern:   Request / Response (+ server-push events)
 *
 * Each message is a JSON object with:
 *   { id, type, payload }
 */

// ── Message types ──────────────────────────────────────────────────────────
export type MessageType =
  // DAM → Plugin (commands)
  | 'MERGE_ASSET'          // merge/import an asset into the scene
  | 'LOAD_MATERIAL'        // load a material from a .mat / library
  | 'REPLACE_OBJECT'       // replace a scene object with an asset
  | 'SAVE_ASSET'           // save an asset from DCC → DAM
  | 'UPDATE_ASSET'         // update an existing DAM asset
  | 'RELINK_TEXTURES'      // relink missing textures
  | 'GET_SCENE_INFO'       // query current scene metadata
  // Plugin → DAM (events)
  | 'SCENE_CHANGED'        // scene was modified
  | 'ASSET_SAVED'          // asset was saved to DAM
  | 'DRAG_START'           // user started dragging an asset from DAM
  | 'PING'
  | 'PONG'
  // Generic
  | 'ACK'
  | 'ERROR';

export interface BridgeMessage<T = unknown> {
  /** Unique request ID (UUID v4) */
  id: string;
  type: MessageType;
  payload: T;
  /** Timestamp (ms since epoch) */
  ts: number;
}

export interface BridgeResponse<T = unknown> {
  /** Matches the request id */
  id: string;
  type: 'ACK' | 'ERROR';
  payload: T;
  error?: string;
  ts: number;
}

// ── Payload definitions ────────────────────────────────────────────────────
export interface MergeAssetPayload {
  /** Absolute path of the file to merge */
  filePath: string;
  /** Where to place it in the scene (optional) */
  position?: { x: number; y: number; z: number };
  /** Merge mode */
  mode: 'import' | 'xref' | 'proxy';
}

export interface LoadMaterialPayload {
  filePath: string;
  materialName?: string;
  applyToSelection: boolean;
}

export interface ReplaceObjectPayload {
  /** Scene object name / ID */
  objectId: string;
  newAssetPath: string;
  keepTransform: boolean;
}

export interface SaveAssetPayload {
  /** Temporary export path the plugin wrote to */
  exportPath: string;
  /** Suggested library destination */
  targetLibraryId?: string;
  metadata?: Record<string, unknown>;
}

export interface SceneInfoPayload {
  host: string;           // "3dsmax" | "blender" | "c4d" | ...
  version: string;
  scenePath?: string;
  objectCount: number;
}

export interface RelinkTexturesPayload {
  assetPath: string;
  /** New root dir to search in */
  searchRoot: string;
}

// ── Client helper ──────────────────────────────────────────────────────────
export const BRIDGE_PORT = 37520;
export const BRIDGE_VERSION = '1';

export function makeBridgeMessage<T>(
  type: MessageType,
  payload: T
): BridgeMessage<T> {
  return {
    id: crypto.randomUUID(),
    type,
    payload,
    ts: Date.now(),
  };
}
