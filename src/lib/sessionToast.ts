import { isValidElement, type ReactNode } from "react";
import { toast as sonnerToast, type ExternalToast } from "sonner";
import {
  type SessionToastLevel,
  useSessionToastStore,
} from "@/stores/sessionToastStore";

type ToastMessage = Parameters<typeof sonnerToast.error>[0];
type ToastResult = ReturnType<typeof sonnerToast.error>;

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
  options?: ExternalToast,
): ToastResult => {
  const resolvedMessage = resolveToastNode(message);
  const resolvedDescription = resolveToastNode(options?.description);
  const liveOptions =
    typeof options?.description === "function"
      ? { ...options, description: resolvedDescription }
      : options;

  const toastId = sonnerToast[level](resolvedMessage, liveOptions);

  useSessionToastStore.getState().addToast({
    level,
    message: getNodeText(resolvedMessage) ?? "",
    description: getNodeText(resolvedDescription),
    actionLabel: getActionLabel(options?.action),
  });

  return toastId;
};

const passthroughToast = ((...args: Parameters<typeof sonnerToast>) =>
  sonnerToast(...args)) as typeof sonnerToast;

// Keep Sonner's complete API intact; only error and warning are intercepted.
export const sessionToast = Object.assign(passthroughToast, sonnerToast, {
  error: (message: ToastMessage, options?: ExternalToast) =>
    showTrackedToast("error", message, options),
  warning: (message: ToastMessage, options?: ExternalToast) =>
    showTrackedToast("warning", message, options),
});
