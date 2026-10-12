import React from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import { WindowDetails } from "../ui/WindowDetails";
import { describeProfileOutputPlan, type PlanItem, type ProfileOutputPlanInput } from "@/lib/profileOutputPlan";
import { openLivePreviewSettings } from "@/lib/transcriptionProfileNavigation";

interface Props {
  scopeKey: string;
  input: ProfileOutputPlanInput;
}

/** Collapsed-by-default summary of what the current output settings do; follows live edits. */
export const ProfileOutputPlanPanel: React.FC<Props> = ({ scopeKey, input }) => {
  const { t } = useTranslation();
  const plan = describeProfileOutputPlan(input);
  const renderItem = (item: PlanItem, index: number) => (
    <li key={`${item.key}-${index}`}>
      {t(`profileOutput.plan.${item.key}`, item.values)}
      {item.link === "livePreview" && <>
        {" "}
        <button type="button" className="underline underline-offset-2 hover:text-text" onClick={openLivePreviewSettings}>
          {t("profileOutput.planPreviewLink")}
        </button>
      </>}
    </li>
  );
  return (
    <WindowDetails stateKey={`profile-${scopeKey}-output-plan`} id={`profile-output-${scopeKey}-plan`}
      className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden">
      <summary className="flex items-center gap-2 px-3 py-2 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
        <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
        <span className="text-xs font-semibold text-text/70">{t("profileOutput.planTitle")}</span>
      </summary>
      <div className="space-y-3 border-t border-mid-gray/15 px-4 py-3 text-xs text-text/80">
        <div>
          <p className="mb-1 font-semibold text-text/70">{t("profileOutput.planSteps")}</p>
          <ol className="list-decimal space-y-1 pl-5">{plan.steps.map(renderItem)}</ol>
        </div>
        {plan.ifs.length > 0 && <div>
          <p className="mb-1 font-semibold text-text/70">{t("profileOutput.planIfs")}</p>
          <ul className="list-disc space-y-1 pl-5">{plan.ifs.map(renderItem)}</ul>
        </div>}
      </div>
    </WindowDetails>
  );
};
