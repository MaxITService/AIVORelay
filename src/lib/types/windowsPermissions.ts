export type PermissionAccess = "allowed" | "denied" | "unknown";

export interface WindowsMicrophonePermissionStatus {
  supported: boolean;
  overall_access: PermissionAccess;
  device_access: PermissionAccess;
  app_access: PermissionAccess;
  desktop_app_access: PermissionAccess;
}
