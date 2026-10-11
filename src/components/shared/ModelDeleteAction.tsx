import React from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import type { ModelDeletionViewJob } from "@/lib/modelDeletion";
import { sessionToast as toast } from "@/lib/sessionToast";
import { Button } from "../ui/Button";

interface Props {
  modelName: string;
  job?: ModelDeletionViewJob;
  secondsRemaining: number;
  busy: boolean;
  compact?: boolean;
  allowDelete?: boolean;
  onDelete: () => Promise<unknown>;
  onUndo: () => Promise<unknown>;
}

export const ModelDeleteAction: React.FC<Props> = ({
  modelName, job, secondsRemaining, busy, compact = false, allowDelete = true, onDelete, onUndo,
}) => {
  const { t } = useTranslation();
  const deleting = job != null && (job.status === "deleting" || secondsRemaining === 0);
  const undo = job?.status === "pending" && !deleting;
  const label = deleting ? t("modelSelector.deletion.deleting") : undo
    ? t("modelSelector.deletion.undo") : t("common.delete");
  const title = undo ? t("modelSelector.deletion.undoModel", { modelName })
    : deleting ? t("modelSelector.deletion.deletingModel", { modelName })
    : t("modelSelector.deleteModel", { modelName });
  const onClick = async (event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (busy || deleting) return;
    try {
      await (undo ? onUndo() : onDelete());
    } catch (error) {
      toast.error(t(undo ? "modelSelector.deletion.cancelFailed" : "modelSelector.deletion.failed"), {
        description: String(error),
      });
    }
  };
  const contents = undo ? <>
    <span>{label}</span><span className="tabular-nums">{t("modelSelector.deletion.seconds", { count: secondsRemaining })}</span>
  </> : !job && compact ? <Trash2 aria-hidden="true" className="h-3 w-3" /> : label;
  return (
    <div className={`${compact ? "w-[88px]" : "w-24"} shrink-0 flex justify-end`}>
      {(allowDelete || job) && (compact ? <button
        type="button" onClick={onClick} onKeyDown={event => event.stopPropagation()}
        disabled={busy || deleting} title={title} aria-label={title}
        className={`inline-flex h-5 max-w-full items-center justify-end gap-1 whitespace-nowrap rounded px-1 text-[10px] transition-colors disabled:opacity-50 disabled:cursor-default ${undo ? "text-text/70 hover:bg-mid-gray/10 hover:text-text" : job ? "text-text/40" : "text-red-400 hover:text-red-300 hover:bg-red-500/10"}`}
      >{contents}</button> : <Button
        type="button" variant={job ? "ghost" : "danger"} size="sm"
        className="flex h-7 w-full items-center justify-center gap-1 whitespace-nowrap !px-2"
        onClick={onClick} onKeyDown={event => event.stopPropagation()}
        disabled={busy || deleting} title={title} aria-label={title}
      >{contents}</Button>)}
    </div>
  );
};
