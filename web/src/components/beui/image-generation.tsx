import * as React from "react"
import { Download, Image as ImageIcon, Loader2, Maximize2 } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { cn } from "@/lib/utils"

export type ImageGenVariant = "beui-frame" | "minimal" | "assistant-image-gen"

export interface ImageGenerationProps {
  src?: string
  prompt?: string
  status?: "generating" | "completed" | "failed"
  aspectLabel?: string
  variant?: ImageGenVariant
  className?: string
}

/**
 * Zero-regex helper to detect image generation tools or extract generated image URLs/data URIs.
 */
export function extractToolImagePayload(
  toolName: string,
  args: unknown,
  outputText: string
): { src?: string; prompt?: string; isImageTool: boolean } {
  const lower = toolName.toLowerCase()
  const isImageTool =
    lower.includes("generate_image") ||
    lower.includes("image_gen") ||
    lower.includes("html_to_image") ||
    lower.includes("svg_animator")

  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null

  const prompt =
    (typeof argsObj?.prompt === "string" && argsObj.prompt) ||
    (typeof argsObj?.description === "string" && argsObj.description) ||
    (typeof argsObj?.title === "string" && argsObj.title) ||
    undefined

  let src: string | undefined
  if (outputText.startsWith("data:image/")) {
    src = outputText.trim()
  } else if (outputText.includes("data:image/")) {
    const start = outputText.indexOf("data:image/")
    let end = start
    while (
      end < outputText.length &&
      outputText[end] !== '"' &&
      outputText[end] !== "'" &&
      outputText[end] !== " " &&
      outputText[end] !== "\n"
    ) {
      end += 1
    }
    src = outputText.slice(start, end)
  } else if (outputText.startsWith("{")) {
    try {
      const parsed = JSON.parse(outputText) as Record<string, unknown>
      const candidate =
        parsed.image_url ?? parsed.url ?? parsed.data_url ?? parsed.dataUrl
      if (typeof candidate === "string" && (candidate.startsWith("http") || candidate.startsWith("data:image/"))) {
        src = candidate
      }
    } catch {
      // ignore non-JSON output
    }
  }

  return { src, prompt, isImageTool }
}

/**
 * @beui/image-generation — Visual AI image generation card with live shimmer canvas,
 * prompt metadata, fullscreen lightbox zoom, and one-click download.
 */
export const ImageGeneration: React.FC<ImageGenerationProps> = ({
  src,
  prompt,
  status = src ? "completed" : "generating",
  aspectLabel = "PNG",
  variant = "beui-frame",
  className,
}) => {
  const [zoomOpen, setZoomOpen] = React.useState(false)

  if ((variant === "minimal" || variant === "assistant-image-gen") && src) {
    return (
      <figure
        className={cn(
          "my-2.5 inline-block max-w-full",
          variant === "assistant-image-gen" &&
            "overflow-hidden rounded-2xl border border-border/65 bg-card/75 p-2 shadow-2xs",
          className
        )}
      >
        <img
          src={src}
          alt={prompt || "Generated visual"}
          onClick={() => setZoomOpen(true)}
          className="max-h-80 cursor-zoom-in rounded-xl border border-border/60 object-contain"
        />
        {prompt && (
          <figcaption className="mt-1.5 px-1 text-[11px] text-muted-foreground">
            {prompt}
          </figcaption>
        )}
      </figure>
    )
  }

  return (
    <>
      <div
        className={cn(
          "my-2.5 w-full max-w-md overflow-hidden rounded-xl border border-border/75 bg-card/85 shadow-xs",
          className
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between gap-2 border-b border-border/55 bg-muted/30 px-3 py-1.5 text-xs">
          <span className="flex min-w-0 items-center gap-1.5 font-medium text-foreground">
            <ImageIcon className="size-3.5 shrink-0 text-primary" />
            <span className="truncate">
              {status === "generating" ? "Generating visual…" : "Generated Image"}
            </span>
          </span>

          <div className="flex shrink-0 items-center gap-1">
            <Badge variant="outline" className="px-1.5 py-0 text-[10px]">
              {aspectLabel}
            </Badge>
            {src && (
              <>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => setZoomOpen(true)}
                  title="Inspect full size"
                  aria-label="Inspect full size"
                >
                  <Maximize2 className="size-3" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  asChild
                  title="Download image"
                  aria-label="Download image"
                >
                  <a href={src} download="openz-image.png">
                    <Download className="size-3" />
                  </a>
                </Button>
              </>
            )}
          </div>
        </div>

        {/* Canvas */}
        <div className="relative flex min-h-44 items-center justify-center bg-muted/20 p-2">
          {status === "generating" || !src ? (
            <div className="flex h-44 w-full flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-muted/35 px-4 text-center">
              <Loader2 className="size-5 animate-spin text-primary" />
              <span className="text-xs font-medium text-foreground/85">
                Rendering visual output…
              </span>
              {prompt && (
                <span className="line-clamp-2 max-w-xs text-[11px] text-muted-foreground">
                  {prompt}
                </span>
              )}
            </div>
          ) : (
            <img
              src={src}
              alt={prompt || "Generated image"}
              onClick={() => setZoomOpen(true)}
              className="max-h-72 w-auto cursor-zoom-in rounded-lg object-contain transition-transform hover:scale-[1.01]"
            />
          )}
        </div>

        {prompt && src && (
          <div className="border-t border-border/45 px-3 py-1.5 text-[11px] text-muted-foreground">
            {prompt}
          </div>
        )}
      </div>

      {src && (
        <Dialog open={zoomOpen} onOpenChange={setZoomOpen}>
          <DialogContent className="sm:max-w-3xl">
            <DialogHeader>
              <DialogTitle>Visual Output Preview</DialogTitle>
              {prompt && <DialogDescription>{prompt}</DialogDescription>}
            </DialogHeader>
            <div className="flex max-h-[75vh] items-center justify-center overflow-auto rounded-lg bg-muted/30 p-2">
              <img
                src={src}
                alt={prompt || "Generated image"}
                className="max-h-[70vh] w-auto rounded-md object-contain"
              />
            </div>
          </DialogContent>
        </Dialog>
      )}
    </>
  )
}
