/** Current planner→executor protocol version. Single source of truth for
 * the frontend — the Rust side has its own default in `PackOptions` and an
 * allowlist in `settings.rs`; all three must move together when a new
 * protocol version ships. */
export const PROTOCOL_VERSION = "plan-exec-v1";
