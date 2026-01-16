import React, { useEffect, useRef, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";

export interface DropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
  className?: string;
}

interface DropdownProps {
  options: DropdownOption[];
  className?: string;
  selectedValue: string | null;
  onSelect: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  onRefresh?: () => void;
}

export const Dropdown: React.FC<DropdownProps> = ({
  options,
  selectedValue,
  onSelect,
  className = "",
  placeholder = "Select an option...",
  disabled = false,
  onRefresh,
}) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const searchBufferRef = useRef("");
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Reset highlighted index when dropdown opens
  useEffect(() => {
    if (isOpen) {
      const selectedIndex = options.findIndex(o => o.value === selectedValue);
      setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
    }
  }, [isOpen, options, selectedValue]);

  // Scroll highlighted item into view
  useEffect(() => {
    if (isOpen && highlightedIndex >= 0 && listRef.current) {
      const items = listRef.current.querySelectorAll('button');
      if (items[highlightedIndex]) {
        items[highlightedIndex].scrollIntoView({ block: 'nearest' });
      }
    }
  }, [highlightedIndex, isOpen]);

  const selectedOption = options.find(
    (option) => option.value === selectedValue,
  );

  const handleSelect = (value: string) => {
    onSelect(value);
    setIsOpen(false);
  };

  const handleToggle = () => {
    if (disabled) return;
    if (!isOpen && onRefresh) onRefresh();
    setIsOpen(!isOpen);
  };

  // Keyboard navigation handler
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!isOpen) {
      if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
        e.preventDefault();
        setIsOpen(true);
      }
      return;
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setHighlightedIndex(prev =>
          prev < options.length - 1 ? prev + 1 : prev
        );
        break;
      case 'ArrowUp':
        e.preventDefault();
        setHighlightedIndex(prev => prev > 0 ? prev - 1 : prev);
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (highlightedIndex >= 0 && highlightedIndex < options.length) {
          const option = options[highlightedIndex];
          if (!option.disabled) {
            handleSelect(option.value);
          }
        }
        break;
      case 'Escape':
        e.preventDefault();
        setIsOpen(false);
        break;
      case 'Home':
        e.preventDefault();
        setHighlightedIndex(0);
        break;
      case 'End':
        e.preventDefault();
        setHighlightedIndex(options.length - 1);
        break;
      default:
        // Type-ahead search: jump to first option starting with typed letter(s)
        if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
          e.preventDefault();
          
          // Clear previous timeout
          if (searchTimeoutRef.current) {
            clearTimeout(searchTimeoutRef.current);
          }
          
          // Add to search buffer
          searchBufferRef.current += e.key.toLowerCase();
          
          // Find first matching option
          const searchStr = searchBufferRef.current;
          const matchIndex = options.findIndex(opt =>
            opt.label.toLowerCase().startsWith(searchStr)
          );
          
          if (matchIndex >= 0) {
            setHighlightedIndex(matchIndex);
          }
          
          // Clear buffer after 500ms of no typing
          searchTimeoutRef.current = setTimeout(() => {
            searchBufferRef.current = "";
          }, 500);
        }
        break;
    }
  }, [isOpen, options, highlightedIndex, handleSelect]);

  return (
    <div className={`relative ${className}`} ref={dropdownRef}>
      <button
        type="button"
        className={`px-3 py-2 text-sm font-medium bg-[#1e1e1e]/80 border border-[#3c3c3c] rounded-md min-w-[200px] text-left flex items-center justify-between transition-all duration-200 ${
          disabled
            ? "opacity-40 cursor-not-allowed"
            : "hover:bg-[#252525]/80 hover:border-[#4a4a4a] cursor-pointer"
        } ${selectedOption?.className || "text-[#e8e8e8]"}`}
        onClick={handleToggle}
        onKeyDown={handleKeyDown}
        disabled={disabled}
      >
        <span className="truncate">{selectedOption?.label || placeholder}</span>
        <svg
          className={`w-4 h-4 ml-2 transition-transform duration-200 text-[#6b6b6b] ${isOpen ? "transform rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M19 9l-7 7-7-7"
          />
        </svg>
      </button>
      {isOpen && !disabled && (
        <div
          ref={listRef}
          className="absolute top-full left-0 right-0 mt-1 bg-[#252525]/98 backdrop-blur-xl border border-[#3c3c3c] rounded-lg shadow-[0_12px_40px_rgba(0,0,0,0.5)] z-50 max-h-60 overflow-y-auto p-1"
          onKeyDown={handleKeyDown}
        >
          {options.length === 0 ? (
            <div className="px-3 py-2 text-sm text-[#6b6b6b]">
              {t("common.noOptionsFound")}
            </div>
          ) : (
            options.map((option, index) => (
              <button
                key={option.value}
                type="button"
                className={`w-full px-3 py-2 text-sm text-left rounded-md transition-all duration-150 ${
                  selectedValue === option.value
                    ? `bg-[#ff4d8d]/20 font-medium ${option.className || "text-[#ff4d8d]"}`
                    : index === highlightedIndex
                      ? `bg-[#ffffff]/10 ${option.className || "text-[#e8e8e8]"}`
                      : `hover:bg-[#ffffff]/5 ${option.className || "text-[#e8e8e8]"}`
                } ${option.disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}`}
                onClick={() => handleSelect(option.value)}
                disabled={option.disabled}
                onMouseEnter={() => setHighlightedIndex(index)}
              >
                <span className="truncate">{option.label}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
};
