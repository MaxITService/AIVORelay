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
  | "Auth"
  | "RateLimited"
  | "Billing"
  | "BadRequest"
  | "TlsCertificate"
  | "TlsHandshake"
  | "Timeout"
  | "NetworkError"
  | "ServerError"
  | "ParseError"
  | "ExtensionOffline"
  | "MicrophoneUnavailable"
  | "Unknown";

export type OverlayErrorProvider =
  | "soniox"
  | "open_ai_compatible"
  | "local"
  | "extension"
  | "unknown";

export type OverlayErrorTransport = "ws" | "http" | "local" | "unknown";

export type OverlayErrorPhase =
  | "connect"
  | "start"
  | "stream"
  | "finalize"
  | "process"
  | "unknown";

export type OverlayCanonicalErrorCode =
  | "E_AUTH"
  | "E_BAD_REQUEST"
  | "E_BILLING"
  | "E_RATE_LIMIT"
  | "E_TIMEOUT"
  | "E_NETWORK"
  | "E_SERVER"
  | "E_PARSE"
  | "E_EXTENSION_OFFLINE"
  | "E_MIC_UNAVAILABLE"
  | "E_UNKNOWN";

export interface OverlayErrorEnvelope {
  provider: OverlayErrorProvider;
  transport: OverlayErrorTransport;
  canonical_code: OverlayCanonicalErrorCode;
  status_code?: number;
  provider_code?: string;
  phase: OverlayErrorPhase;
  user_message: string;
  technical_message?: string;
  retryable: boolean;
  display_code: string;
}

/**
 * Extended overlay payload with error information
 */
export interface OverlayPayload {
  state: ExtendedOverlayState;
  error_category?: OverlayErrorCategory;
  error_message?: string;
  error_envelope?: OverlayErrorEnvelope;
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
    Auth: "Authentication failed",
    RateLimited: "Rate limit exceeded",
    Billing: "Billing required",
    BadRequest: "Invalid request",
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

export function fallbackCodeFromCategory(
  category?: OverlayErrorCategory,
): string {
  if (!category) return "E_UNKNOWN";

  const map: Record<OverlayErrorCategory, string> = {
    Auth: "E_AUTH",
    RateLimited: "E_RATE",
    Billing: "E_BILL",
    BadRequest: "E_BADREQ",
    TlsCertificate: "E_NET",
    TlsHandshake: "E_NET",
    Timeout: "E_TIMEOUT",
    NetworkError: "E_NET",
    ServerError: "E_SERVER",
    ParseError: "E_PARSE",
    ExtensionOffline: "E_EXT",
    MicrophoneUnavailable: "E_MIC",
    Unknown: "E_UNKNOWN",
  };
  return map[category];
}
