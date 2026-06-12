import { GripHorizontal, GripVertical } from "lucide-react";
import * as ResizablePrimitive from "react-resizable-panels";

import { cn } from "@/lib/utils";

const ResizablePanelGroup = ({ className, ...props }: React.ComponentProps<typeof ResizablePrimitive.PanelGroup>) => (
  <ResizablePrimitive.PanelGroup
    className={cn("flex h-full min-h-0 w-full data-[panel-group-direction=vertical]:flex-col", className)}
    {...props}
  />
);

const ResizablePanel = ResizablePrimitive.Panel;

const ResizableHandle = ({
  withHandle,
  className,
  ...props
}: React.ComponentProps<typeof ResizablePrimitive.PanelResizeHandle> & {
  withHandle?: boolean;
}) => (
  <ResizablePrimitive.PanelResizeHandle
    className={cn(
      "group relative z-30 flex shrink-0 touch-none select-none bg-border/60 transition-colors hover:bg-primary/30 active:bg-primary/45",
      "w-2 data-[panel-group-direction=vertical]:h-2 data-[panel-group-direction=vertical]:w-full",
      "data-[panel-group-direction=vertical]:cursor-row-resize data-[panel-group-direction=horizontal]:cursor-col-resize",
      "after:absolute after:bg-transparent",
      "data-[panel-group-direction=horizontal]:after:inset-y-0 data-[panel-group-direction=horizontal]:after:left-1/2 data-[panel-group-direction=horizontal]:after:w-3 data-[panel-group-direction=horizontal]:after:-translate-x-1/2",
      "data-[panel-group-direction=vertical]:after:inset-x-0 data-[panel-group-direction=vertical]:after:top-1/2 data-[panel-group-direction=vertical]:after:h-3 data-[panel-group-direction=vertical]:after:-translate-y-1/2",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1",
      className,
    )}
    {...props}
  >
    {withHandle ? (
      <div className="pointer-events-none absolute left-1/2 top-1/2 z-10 flex -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-sm border border-border/80 bg-background/95 p-0.5 shadow-sm group-data-[panel-group-direction=vertical]:rotate-90">
        <GripVertical className="h-3 w-3 text-muted-foreground" />
      </div>
    ) : (
      <>
        <span className="pointer-events-none absolute left-1/2 top-1/2 hidden -translate-x-1/2 -translate-y-1/2 group-data-[panel-group-direction=vertical]:block">
          <GripHorizontal className="h-3 w-3 text-muted-foreground/80" />
        </span>
        <span className="pointer-events-none absolute left-1/2 top-1/2 hidden -translate-x-1/2 -translate-y-1/2 group-data-[panel-group-direction=horizontal]:block">
          <GripVertical className="h-3 w-3 text-muted-foreground/80" />
        </span>
      </>
    )}
  </ResizablePrimitive.PanelResizeHandle>
);

export { ResizablePanelGroup, ResizablePanel, ResizableHandle };
