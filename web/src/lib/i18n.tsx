"use client";

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
} from "react";

/**
 * A component's messages. Plain strings with `{name}` placeholders, so a
 * dictionary stays JSON-serializable and can be passed from a server layout.
 * There are no plural forms: write messages that read correctly for any count.
 */
export type Messages = Record<string, string>;

/** A language file: component namespace → any subset of that component's messages. */
export type LocaleMessages = Record<string, Partial<Messages>>;

type LocaleContextValue = {
  locale?: string;
  messages: LocaleMessages;
};

const NO_MESSAGES: LocaleMessages = {};

const LocaleContext = createContext<LocaleContextValue>({ messages: NO_MESSAGES });

/** Without a locale, digits stay plain so server and client render the same text. */
export function formatNumber(value: number, locale?: string) {
  return locale ? new Intl.NumberFormat(locale).format(value) : String(value);
}

export interface LocaleProviderProps {
  /** BCP 47 tag, e.g. `de`, `pt-BR`. Drives `Intl` number formatting. */
  locale: string;
  /**
   * Component namespace → translated strings. Missing keys fall back to English.
   * Pass a stable object (a module constant, imported JSON or a memoized value):
   * a new object each render gives every translated component a new `t`.
   */
  messages?: LocaleMessages;
  children: ReactNode;
}

export function LocaleProvider({ locale, messages = NO_MESSAGES, children }: LocaleProviderProps) {
  const value = useMemo(() => ({ locale, messages }), [locale, messages]);
  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale(): { locale?: string } {
  return { locale: useContext(LocaleContext).locale };
}

function isWordChar(code: number): boolean {
  return (
    (code >= 48 && code <= 57) ||
    (code >= 65 && code <= 90) ||
    (code >= 97 && code <= 122) ||
    code === 95
  );
}

export function interpolateTemplate(
  template: string,
  values: Record<string, string | number>,
  locale?: string,
): string {
  let out = "";
  let i = 0;
  const n = template.length;
  while (i < n) {
    const openIdx = template.indexOf("{", i);
    if (openIdx === -1) {
      out += template.slice(i);
      break;
    }
    out += template.slice(i, openIdx);
    const closeIdx = template.indexOf("}", openIdx + 1);
    if (closeIdx === -1) {
      out += template.slice(openIdx);
      break;
    }
    const token = template.slice(openIdx + 1, closeIdx);
    let validWord = token.length > 0;
    for (let k = 0; k < token.length; k += 1) {
      if (!isWordChar(token.charCodeAt(k))) {
        validWord = false;
        break;
      }
    }
    if (validWord && Object.prototype.hasOwnProperty.call(values, token)) {
      const value = values[token];
      if (value === undefined) {
        out += template.slice(openIdx, closeIdx + 1);
      } else {
        out += typeof value === "number" ? formatNumber(value, locale) : value;
      }
      i = closeIdx + 1;
    } else if (!validWord) {
      out += "{";
      i = openIdx + 1;
    } else {
      out += template.slice(openIdx, closeIdx + 1);
      i = closeIdx + 1;
    }
  }
  return out;
}

/**
 * Resolves a component's strings: `labels` prop, then the provider's
 * messages for `namespace`, then the component's English `defaults`.
 * Numbers passed as values are formatted for the provider's locale.
 */
export function useMessages<T extends Messages>(
  namespace: string,
  defaults: T,
  labels?: Partial<T>,
) {
  const { locale, messages } = useContext(LocaleContext);
  const provided = messages[namespace];
  // Inline `labels={{ … }}` is a new object every render. Keying it by content
  // keeps `t` one identity until a label changes, so effects and memos that
  // depend on `t` don't re-run per render, and render-time reads never lag.
  const labelsKey = labels ? JSON.stringify(labels) : "";
  const stableLabels = useMemo(
    () => (labelsKey ? (JSON.parse(labelsKey) as Partial<T>) : undefined),
    [labelsKey],
  );

  return useCallback(
    (key: keyof T & string, values?: Record<string, string | number>) => {
      const template = stableLabels?.[key] ?? provided?.[key] ?? defaults[key];
      if (!values) return template;
      return interpolateTemplate(template, values, locale);
    },
    [defaults, stableLabels, locale, provided],
  );
}
