import React from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../ui/Button";
import { Input } from "../ui/Input";

interface StoredApiKeyDisplayProps {
  disabled?: boolean;
  loading?: boolean;
  onDelete: () => void;
  onReplace: () => void;
}

interface ApiKeyEditorProps {
  disabled?: boolean;
  loading?: boolean;
  placeholder?: string;
  saveDisabled?: boolean;
  showCancel?: boolean;
  value: string;
  onBlur?: () => void;
  onCancel?: () => void;
  onChange: (value: string) => void;
  onSave: () => void;
  hint?: React.ReactNode;
}

export const StoredApiKeyDisplay: React.FC<StoredApiKeyDisplayProps> = ({
  disabled = false,
  loading = false,
  onDelete,
  onReplace,
}) => {
  const { t } = useTranslation();
  const isDisabled = disabled || loading;
  const replaceLabel = t("settings.advanced.remoteStt.apiKey.clickToReplace");

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          type="text"
          value="************************************************"
          readOnly
          aria-label={replaceLabel}
          title={replaceLabel}
          onClick={onReplace}
          onFocus={onReplace}
          className="min-w-0 flex-1 cursor-pointer select-none border-mid-gray/40 bg-mid-gray/10 text-text/45"
        />
        <Button
          variant="secondary"
          size="sm"
          onClick={onReplace}
          disabled={isDisabled}
        >
          {t("settings.advanced.remoteStt.apiKey.replace")}
        </Button>
      </div>
      <div className="flex items-center gap-2 text-sm text-green-400">
        <span className="inline-flex h-2 w-2 rounded-full bg-green-400" />
        <span>{t("settings.advanced.remoteStt.apiKey.statusStored")}</span>
      </div>
      <p className="text-xs text-text/60">
        {t("settings.advanced.remoteStt.apiKey.statusStoredHint")}
      </p>
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={onDelete}
          disabled={isDisabled}
        >
          {t("settings.advanced.remoteStt.apiKey.clear")}
        </Button>
      </div>
    </div>
  );
};

export const ApiKeyEditor: React.FC<ApiKeyEditorProps> = ({
  disabled = false,
  hint,
  loading = false,
  onCancel,
  onBlur,
  onChange,
  onSave,
  placeholder,
  saveDisabled = false,
  showCancel = false,
  value,
}) => {
  const { t } = useTranslation();
  const isDisabled = disabled || loading;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          type="password"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onBlur={onBlur}
          placeholder={placeholder}
          disabled={isDisabled}
          className="min-w-0 flex-1"
        />
        <Button
          variant="secondary"
          size="sm"
          onMouseDown={(event) => event.preventDefault()}
          onClick={onSave}
          disabled={isDisabled || saveDisabled || value.trim().length === 0}
        >
          {t("settings.advanced.remoteStt.apiKey.save")}
        </Button>
        {showCancel && onCancel ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={onCancel}
            disabled={isDisabled}
          >
            {t("settings.advanced.remoteStt.apiKey.cancel")}
          </Button>
        ) : null}
      </div>
      {hint ? <p className="text-xs text-text/60">{hint}</p> : null}
    </div>
  );
};
