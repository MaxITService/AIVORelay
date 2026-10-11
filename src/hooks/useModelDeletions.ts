import { useEffect, useState, useSyncExternalStore } from "react";
import {
  ensureModelDeletionTracking,
  getModelDeletionState,
  modelDeletionSecondsRemaining,
  reconcileExpiredModelDeletions,
  reconcileModelDeletions,
  subscribeModelDeletionState,
} from "@/lib/modelDeletion";
import i18n from "@/i18n";
import { sessionToast as toast } from "@/lib/sessionToast";

export const useModelDeletions = () => {
  const state = useSyncExternalStore(subscribeModelDeletionState, getModelDeletionState);
  const [now, setNow] = useState(() => performance.now());
  const pending = Array.from(state.jobs.values()).some(job => job.status === "pending");
  useEffect(() => {
    void ensureModelDeletionTracking().then(reconcileModelDeletions).catch(error => {
      toast.error(i18n.t("modelSelector.deletion.syncFailed"), { description: String(error) });
    });
  }, []);
  useEffect(() => {
    setNow(performance.now());
    if (!pending) return;
    const timer = window.setInterval(() => setNow(performance.now()), 100);
    return () => window.clearInterval(timer);
  }, [pending]);
  useEffect(() => {
    if (pending) reconcileExpiredModelDeletions();
  }, [now, state.jobs, pending]);
  return {
    ...state,
    secondsRemaining: (modelId: string) => modelDeletionSecondsRemaining(state.jobs.get(modelId), Math.max(now, performance.now())),
  };
};
