import { THEME_FAMILIES } from "../shared/lib/colorEngine"

type LogLevel = "info" | "warn" | "error" | "debug"

const isDev = import.meta.env.DEV

const fallbackPalette = THEME_FAMILIES[0].dark

const cssVarByLevel: Record<LogLevel, { varName: string; fallback: string }> = {
  info: { varName: "--info", fallback: fallbackPalette.infoHex },
  warn: { varName: "--warning", fallback: fallbackPalette.warningHex },
  error: { varName: "--destructive", fallback: fallbackPalette.destructiveHex },
  debug: { varName: "--success", fallback: fallbackPalette.successHex },
}

const resolveLevelStyle = (level: LogLevel): string => {
  const spec = cssVarByLevel[level]
  if (typeof document === "undefined") {
    return `color: ${spec.fallback}; font-weight: 600;`
  }
  const computed = getComputedStyle(document.documentElement)
    .getPropertyValue(spec.varName)
    .trim()
  return `color: ${computed || spec.fallback}; font-weight: 600;`
}

const formatMessage = (level: LogLevel, message: string) => {
  const prefix = `[WEB] ${level.toUpperCase()}`
  return [`%c${prefix} %c${message}`, resolveLevelStyle(level), "color: inherit; font-weight: normal;"]
}

function log(level: LogLevel, message: string, data?: unknown) {
  if (!isDev && level === "debug") return

  const [prompt, style, reset] = formatMessage(level, message)

  if (data === undefined) {
    if (level === "error") console.error(prompt, style, reset)
    else if (level === "warn") console.warn(prompt, style, reset)
    else console.log(prompt, style, reset)
    return
  }

  // Handle data with grouping for a cleaner console
  console.groupCollapsed(prompt, style, reset)
  
  if (data instanceof Error) {
    console.error(data.message)
    if (data.stack) console.debug(data.stack)
  } else {
    console.dir(data)
  }
  
  console.groupEnd()
}

export const logger = {
  info: (msg: string, data?: unknown) => log("info", msg, data),
  warn: (msg: string, data?: unknown) => log("warn", msg, data),
  error: (msg: string, data?: unknown) => log("error", msg, data),
  debug: (msg: string, data?: unknown) => log("debug", msg, data),
}