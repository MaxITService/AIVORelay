import React from "react";
import { BookOpen, ChevronDown, ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";

export type TtsHelpItem = {
  term: string;
  description: React.ReactNode;
};

export type TtsHelpLink = {
  label: string;
  href: string;
};

type TtsHelpDisclosureProps = {
  summary: React.ReactNode;
  items?: TtsHelpItem[];
  links?: TtsHelpLink[];
};

export const TtsHelpDisclosure: React.FC<TtsHelpDisclosureProps> = ({
  summary,
  items = [],
  links = [],
}) => {
  const { t } = useTranslation();

  return (
    <details className="group px-6 py-3">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-3 rounded-md text-sm font-medium text-[#d7b9ff] outline-none transition-colors hover:text-[#eadcff] focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/40 [&::-webkit-details-marker]:hidden">
        <span className="flex min-w-0 items-center gap-2">
          <BookOpen className="h-4 w-4 shrink-0" aria-hidden="true" />
          <span>{t("textToSpeech.help.tellMeMore", "Tell me more")}</span>
        </span>
        <ChevronDown
          className="h-4 w-4 shrink-0 transition-transform group-open:rotate-180"
          aria-hidden="true"
        />
      </summary>

      <div className="mt-3 space-y-3 text-xs leading-relaxed text-text/70">
        <div>{summary}</div>

        {items.length > 0 && (
          <dl className="grid gap-2 sm:grid-cols-[minmax(7rem,0.36fr)_minmax(0,1fr)]">
            {items.map((item) => (
              <React.Fragment key={item.term}>
                <dt className="font-semibold text-text/85">{item.term}</dt>
                <dd className="min-w-0 text-text/65">{item.description}</dd>
              </React.Fragment>
            ))}
          </dl>
        )}

        {links.length > 0 && (
          <div className="flex flex-wrap gap-x-4 gap-y-2 pt-1">
            {links.map((link) => (
              <a
                key={`${link.href}:${link.label}`}
                className="inline-flex items-center gap-1 text-[#d7b9ff] underline-offset-4 hover:underline focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/40"
                href={link.href}
                target="_blank"
                rel="noopener noreferrer"
              >
                {link.label}
                <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
              </a>
            ))}
          </div>
        )}
      </div>
    </details>
  );
};
