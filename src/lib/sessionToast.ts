import { isValidElement, type ReactNode } from "react";
import { publishUiNotice } from "./activeNotices";
import { useActiveNoticeStore } from "@/stores/activeNoticeStore";
import { toast as sonnerToast, type ExternalToast } from "sonner";
import {
  type SessionToastLevel,
  useSessionToastStore,
} from "@/stores/sessionToastStore";

type ToastMessage = Parameters<typeof sonnerToast.error>[0];
type ToastResult = ReturnType<typeof sonnerToast.error>;
type SessionToastOptions = ExternalToast & { retainActive?: boolean; recordHistory?: boolean };

const resolveToastNode = (node: ToastMessage | undefined): ReactNode =>
  typeof node === "function" ? node() : node;

const getNodeText = (node: ReactNode): string | undefined => {
  if (typeof node === "string" || typeof node === "number") {
    return String(node);
  }

  if (Array.isArray(node)) {
    const text = node.map(getNodeText).filter((part) => part !== undefined);
    return text.length > 0 ? text.join("") : undefined;
  }

  if (isValidElement(node)) {
    const props = node.props as {
      children?: ReactNode;
      "aria-label"?: string;
    };
    return getNodeText(props.children) ?? props["aria-label"];
  }

  return undefined;
};

const getActionLabel = (
  action: ExternalToast["action"],
): string | undefined => {
  if (!action || typeof action !== "object" || isValidElement(action)) {
    return undefined;
  }

  if (!("label" in action)) {
    return undefined;
  }

  return getNodeText((action as { label?: ReactNode }).label);
};

const showTrackedToast = (
  level: SessionToastLevel,
  message: ToastMessage,
  options?: SessionToastOptions,
): ToastResult => {
  const { retainActive = true, recordHistory = true, ...toastOptions } = options ?? {};
  const resolvedMessage = resolveToastNode(message);
  const resolvedDescription = resolveToastNode(toastOptions.description);
  const liveOptions =
    typeof toastOptions.description === "function"
      ? { ...toastOptions, description: resolvedDescription }
      : toastOptions;

  const toastId = sonnerToast[level](resolvedMessage, {
    ...liveOptions,
    closeButton: liveOptions.closeButton ?? true,
    onDismiss: (dismissed) => {
      liveOptions.onDismiss?.(dismissed);
      const notice = useActiveNoticeStore.getState().notices.find(item => item.source === `ui:${dismissed.id}`);
      if (notice) void useActiveNoticeStore.getState().dismiss(notice.id)
        .catch(error => console.warn("Failed to dismiss clicked notice:", error));
    },
  });

  if (recordHistory) useSessionToastStore.getState().addToast({
    level,
    message: getNodeText(resolvedMessage) ?? "",
    description: getNodeText(resolvedDescription),
    actionLabel: getActionLabel(options?.action),
  });

  if (!retainActive) return toastId;
  const source = String(toastId);
  const action = options?.action;
  useActiveNoticeStore.getState().setAction(`ui:${source}`,
    action && typeof action === "object" && "onClick" in action && typeof action.onClick === "function"
      ? { label: getActionLabel(action) ?? "", onClick: action.onClick } : undefined);
  publishUiNotice(source, level, getNodeText(resolvedMessage) ?? "", getNodeText(resolvedDescription));
  return toastId;
};

const passthroughToast = ((...args: Parameters<typeof sonnerToast>) =>
  sonnerToast(...args)) as typeof sonnerToast;

// Keep Sonner's complete API intact; only error and warning are intercepted.
export const sessionToast = Object.assign(passthroughToast, sonnerToast, {
  error: (message: ToastMessage, options?: SessionToastOptions) =>
    showTrackedToast("error", message, options),
  warning: (message: ToastMessage, options?: SessionToastOptions) =>
    showTrackedToast("warning", message, options),
});
