/**
 * Type re-export facade over the auto-generated `tauri-specta` bindings.
 * App code imports wire types from here so it doesn't depend on the
 * auto-generated file directly. Runtime IPC calls go through
 * `commands.*` from `../bindings`.
 */
export type {
  PackFormat,
  PackOptions,
  PackResult,
  PackStats,
  PlanValidation,
  ProgressEvent,
  Settings,
  TransformReport,
} from "../bindings";
