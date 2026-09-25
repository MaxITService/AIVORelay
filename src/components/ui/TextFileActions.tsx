import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, FolderOpen, Save } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { sessionToast as toast } from "@/lib/sessionToast";
import { Button } from "./Button";

interface TextFileActionsProps {
  value: string;
  /** Receives the text of the opened file. */
  onOpen: (text: string) => void | Promise<void>;
  defaultFileName: string;
  extensions?: string[];
  /** Returns an error message when the opened text must not be loaded. */
  validate?: (text: string) => string | null;
  /** Rejects files longer than this many characters when greater than 0. */
  maxLength?: number;
  disabled?: boolean;
  className?: string;
}

/** Save / Open / Copy buttons for a plain-text field. */
export const TextFileActions: React.FC<TextFileActionsProps> = ({
  value,
  onOpen,
  defaultFileName,
  extensions = ["txt"],
  validate,
  maxLength = 0,
  disabled = false,
  className = "",
}) => {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);

  const filters = [
    {
      name: t("common.textFileActions.textFiles", "Text files"),
      extensions,
    },
  ];

  const handleSave = async () => {
    setBusy(true);
    try {
      const destination = await save({ defaultPath: defaultFileName, filters });
      if (!destination) return;
      await writeTextFile(destination, value);
      toast.success(t("common.textFileActions.saved", "Saved to file."));
    } catch (error) {
      console.error("Failed to save text field to file:", error);
      toast.error(
        t("common.textFileActions.saveFailed", "Could not save the file: {{error}}", {
          error: String(error),
        }),
      );
    } finally {
      setBusy(false);
    }
  };

  const getValidationError = (text: string): string | null => {
    // NUL bytes or U+FFFD replacement characters mean a binary or non-UTF-8 file.
    if (text.includes("\u0000") || text.includes("�")) {
      return t(
        "common.textFileActions.notText",
        "The selected file is not a UTF-8 text file.",
      );
    }
    if (!text.trim()) {
      return t("common.textFileActions.empty", "The file is empty.");
    }
    if (maxLength > 0 && text.length > maxLength) {
      return t(
        "common.textFileActions.tooLong",
        "The file has {{length}} characters; the limit is {{limit}}.",
        { length: text.length, limit: maxLength },
      );
    }
    return validate?.(text) ?? null;
  };

  const handleOpen = async () => {
    setBusy(true);
    try {
      const selected = await open({ multiple: false, directory: false, filters });
      if (typeof selected !== "string") return;
      const extension = selected.split(".").pop()?.toLowerCase() ?? "";
      if (!extensions.includes(extension)) {
        toast.error(
          t(
            "common.textFileActions.unsupportedType",
            "Unsupported file type. Choose a {{extensions}} file.",
            { extensions: extensions.map(ext => `.${ext}`).join(", ") },
          ),
        );
        return;
      }
      const text = await readTextFile(selected);
      const validationError = getValidationError(text);
      if (validationError) {
        toast.error(
          t("common.textFileActions.invalidContent", "File was not opened: {{error}}", {
            error: validationError,
          }),
        );
        return;
      }
      await onOpen(text);
    } catch (error) {
      console.error("Failed to open text file:", error);
      toast.error(
        t("common.textFileActions.openFailed", "Could not open the file: {{error}}", {
          error: String(error),
        }),
      );
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(t("common.copied", "Copied!"));
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  };

  const isDisabled = disabled || busy;

  return (
    <div className={`flex shrink-0 items-center justify-end gap-1 ${className}`}>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="flex items-center gap-1 !px-2 !py-1"
        onClick={() => void handleSave()}
        disabled={isDisabled || value.length === 0}
      >
        <Save className="h-3.5 w-3.5" />
        {t("common.save", "Save")}
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="flex items-center gap-1 !px-2 !py-1"
        onClick={() => void handleOpen()}
        disabled={isDisabled}
      >
        <FolderOpen className="h-3.5 w-3.5" />
        {t("common.open", "Open")}
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="flex items-center gap-1 !px-2 !py-1"
        onClick={() => void handleCopy()}
        disabled={value.length === 0}
      >
        <Copy className="h-3.5 w-3.5" />
        {t("common.copy", "Copy")}
      </Button>
    </div>
  );
};
