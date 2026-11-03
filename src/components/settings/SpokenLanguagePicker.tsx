import React, { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { recognitionLanguage } from "@/lib/constants/languages";

export interface SpokenLanguageOption {
  value: string;
  label: string;
  nativeLabel: string;
  searchText: string;
}

const searchText = (text: string) => text.normalize("NFKD").replace(/\p{M}/gu, "").toLowerCase();

export const SpokenLanguagePicker: React.FC<{
  options: SpokenLanguageOption[];
  selected: string[];
  languageLabel: (code: string) => string;
  unavailable: string[];
  disabled: boolean;
  selectionDisabled: boolean;
  saving: boolean;
  onChange: (languages: string[]) => void;
}> = ({ options, selected, languageLabel, unavailable, disabled, selectionDisabled, saving, onChange }) => {
  const { t } = useTranslation();
  const id = useId();
  const field = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState<React.CSSProperties>();
  const inputDisabled = disabled || selectionDisabled;
  const expanded = open && !inputDisabled;
  const filtered = options.filter(option => searchText(option.searchText).includes(searchText(query.trim())));
  const activeIndex = Math.min(active, filtered.length - 1);
  const selectedCode = (value: string) => selected.find(code => recognitionLanguage(code) === recognitionLanguage(value));
  const remove = (code: string) => {
    if (!disabled && !saving) onChange(selected.filter(value => recognitionLanguage(value) !== recognitionLanguage(code)));
  };
  const toggle = (value: string) => {
    if (inputDisabled || saving) return;
    const existing = selectedCode(value);
    if (existing) remove(existing); else onChange([...selected, value]);
    setQuery(""); setActive(0);
  };

  useEffect(() => {
    const outside = (event: MouseEvent) => {
      if (!field.current?.contains(event.target as Node) && !menu.current?.contains(event.target as Node)) {
        setOpen(false); setQuery("");
      }
    };
    document.addEventListener("mousedown", outside);
    return () => document.removeEventListener("mousedown", outside);
  }, []);

  useLayoutEffect(() => {
    if (!expanded) return;
    const place = () => {
      if (!field.current) return;
      const rect = field.current.getBoundingClientRect();
      const below = window.innerHeight - rect.bottom - 12;
      const above = rect.top - 12;
      const up = below < 180 && above > below;
      const width = Math.min(rect.width, window.innerWidth - 16);
      setPosition({ position: "fixed", width, left: Math.max(8, Math.min(rect.left, window.innerWidth - width - 8)),
        maxHeight: Math.max(0, Math.min(240, up ? above : below)),
        ...(up ? { bottom: window.innerHeight - rect.top + 4 } : { top: rect.bottom + 4 }) });
    };
    place();
    const observer = new ResizeObserver(place);
    if (field.current) observer.observe(field.current);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { observer.disconnect(); window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [expanded]);

  useEffect(() => {
    if (expanded) menu.current?.querySelector(`[data-index="${activeIndex}"]`)?.scrollIntoView({ block: "nearest" });
  }, [expanded, activeIndex, query]);

  return <div>
    <div ref={field} className="flex flex-wrap items-center gap-1.5 rounded border border-amber-500/30 bg-black/10 p-1.5 focus-within:border-amber-400">
      {selected.map(code => <button key={code} type="button" disabled={disabled || saving}
        className={`inline-flex items-center gap-1 rounded border border-amber-500/30 px-2 py-1 disabled:cursor-not-allowed ${unavailable.includes(code) ? "text-text/40 line-through" : "text-amber-400"}`}
        onClick={() => remove(code)} aria-label={t("settings.spokenLanguage.remove", { language: languageLabel(code) })}>
        {languageLabel(code)}<X size={12} aria-hidden="true" />
      </button>)}
      <input ref={input} type="text" role="combobox" autoComplete="off" spellCheck={false}
        className="min-w-[140px] flex-1 bg-transparent px-1 py-1 text-text outline-none placeholder:text-text/50 disabled:cursor-not-allowed disabled:opacity-50"
        disabled={inputDisabled} value={query} placeholder={t("settings.spokenLanguage.search")}
        aria-label={t("settings.spokenLanguage.add")} aria-expanded={expanded} aria-autocomplete="list" aria-haspopup="listbox"
        aria-controls={expanded ? `${id}-list` : undefined}
        aria-activedescendant={expanded && activeIndex >= 0 ? `${id}-option-${activeIndex}` : undefined}
        onFocus={() => { setOpen(true); setActive(0); }} onClick={() => setOpen(true)}
        onChange={event => { setQuery(event.target.value); setActive(0); setOpen(true); }}
        onBlur={event => {
          if (!field.current?.contains(event.relatedTarget as Node) && !menu.current?.contains(event.relatedTarget as Node)) {
            setOpen(false); setQuery("");
          }
        }}
        onKeyDown={event => {
          if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229 || event.defaultPrevented) return;
          if (event.key === "Escape" && expanded) {
            event.preventDefault(); event.stopPropagation(); setOpen(false); setQuery("");
          } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault(); event.stopPropagation(); setOpen(true);
            setActive(!expanded ? (event.key === "ArrowUp" ? Math.max(0, filtered.length - 1) : 0)
              : Math.max(0, Math.min(filtered.length - 1, activeIndex + (event.key === "ArrowDown" ? 1 : -1))));
          } else if (event.key === "Enter") {
            event.preventDefault(); event.stopPropagation();
            if (expanded && filtered[activeIndex]) toggle(filtered[activeIndex].value); else setOpen(true);
          }
        }} />
      <button type="button" tabIndex={-1} disabled={inputDisabled} aria-label={t("settings.spokenLanguage.showLanguages")}
        className="p-1 text-text/60 disabled:opacity-40" onMouseDown={event => event.preventDefault()}
        onClick={() => { if (expanded) { setOpen(false); setQuery(""); } else { input.current?.focus(); setOpen(true); } }}>
        <ChevronDown size={14} aria-hidden="true" />
      </button>
    </div>
    {expanded && position && createPortal(<div ref={menu} id={`${id}-list`} role="listbox" aria-multiselectable="true"
      aria-label={t("settings.spokenLanguage.add")} aria-busy={saving} style={position}
      className="z-[100] overflow-y-auto overscroll-contain rounded border border-mid-gray/50 bg-[#1a1a1a] p-1 text-xs text-text shadow-xl">
      {filtered.length === 0 && <p role="status" className="px-2 py-3 text-text/60">{t("settings.spokenLanguage.noResults")}</p>}
      {filtered.map((option, index) => <button key={option.value} id={`${id}-option-${index}`} data-index={index}
        type="button" role="option" aria-selected={Boolean(selectedCode(option.value))} aria-disabled={saving} tabIndex={-1}
        className={`flex w-full items-center gap-2 rounded px-2 py-2 text-left ${index === activeIndex ? "bg-white/10" : "hover:bg-white/5"}`}
        onMouseDown={event => event.preventDefault()} onMouseEnter={() => setActive(index)} onClick={() => toggle(option.value)}>
        <Check size={14} aria-hidden="true" className={`shrink-0 text-amber-400 ${selectedCode(option.value) ? "" : "invisible"}`} />
        <span className="flex-1">{option.label}{option.nativeLabel !== option.label && <span className="ml-2 text-text/50">{option.nativeLabel}</span>}</span>
        <span className="text-text/40">{option.value}</span>
      </button>)}
    </div>, document.body)}
  </div>;
};
