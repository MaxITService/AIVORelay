/**
 * Extended overlay states for Remote STT API
 * Fork-specific file: TypeScript types and utilities for extended overlay states.
 */

/**
 * Extended overlay state type including new states
 */
export type ExtendedOverlayState =
  | "recording"
  | "sending"
  | "transcribing"
  | "thinking"
  | "finalizing"
  | "error"
  | "profile_switch";

/**
 * Error categories matching Rust OverlayErrorCategory enum
 */
export type OverlayErrorCategory =
  | "TlsCertificate"
  | "TlsHandshake"
  | "Timeout"
  | "NetworkError"
  | "ServerError"
  | "ParseError"
  | "ExtensionOffline"
  | "MicrophoneUnavailable"
  | "Unknown";

/**
 * Extended overlay payload with error information
 */
export interface OverlayPayload {
  state: ExtendedOverlayState;
  error_category?: OverlayErrorCategory;
  error_message?: string;
  decapitalize_eligible?: boolean;
  decapitalize_armed?: boolean;
}

/**
 * Type guard to check if payload is an extended OverlayPayload object
 */
export function isExtendedPayload(payload: unknown): payload is OverlayPayload {
  return (
    typeof payload === "object" &&
    payload !== null &&
    "state" in payload &&
    typeof (payload as OverlayPayload).state === "string"
  );
}

/**
 * Get the display text for an error category (English only)
 */
export function getErrorDisplayText(category: OverlayErrorCategory): string {
  const messages: Record<OverlayErrorCategory, string> = {
    TlsCertificate: "Certificate error",
    TlsHandshake: "Connection failed",
    Timeout: "Request timed out",
    NetworkError: "Network unavailable",
    ServerError: "Server error",
    ParseError: "Invalid response",
    ExtensionOffline: "Extension offline",
    MicrophoneUnavailable: "Mic unavailable",
    Unknown: "Transcription failed",
  };
  return messages[category];
}
