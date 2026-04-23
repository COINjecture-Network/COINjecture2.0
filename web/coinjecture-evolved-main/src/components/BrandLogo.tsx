import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

const outerSize: Record<"sm" | "md" | "lg", string> = {
  sm: "h-10 w-10 p-1",
  md: "h-14 w-14 p-1.5",
  lg: "h-[4.5rem] w-[4.5rem] p-2",
};

const imgSize: Record<"sm" | "md" | "lg", string> = {
  sm: "h-8 w-8",
  md: "h-11 w-11",
  lg: "h-14 w-14",
};

type BrandLogoProps = {
  className?: string;
  size?: "sm" | "md" | "lg";
};

const symbolGlow: CSSProperties = {
  filter: [
    "drop-shadow(0 0 1px hsl(var(--primary) / 0.9))",
    "drop-shadow(0 0 6px hsl(var(--primary) / 0.45))",
    "drop-shadow(0 0 14px hsl(var(--accent-purple) / 0.4))",
    "drop-shadow(0 0 24px hsl(var(--glow-primary) / 0.28))",
  ].join(" "),
};

export function BrandLogo({ className, size = "md" }: BrandLogoProps) {
  return (
    <div
      className={cn(
        "inline-flex shrink-0 items-center justify-center overflow-visible rounded-xl",
        "ring-2 ring-primary/50 ring-offset-2 ring-offset-background",
        "shadow-[0_0_28px_hsl(var(--glow-primary)/0.35)]",
        outerSize[size],
        className,
      )}
    >
      <div
        className={cn(
          "flex items-center justify-center overflow-visible rounded-md",
          imgSize[size],
        )}
      >
        <img
          src="/COINjecture.png"
          alt="COINjecture"
          className="h-full w-full object-contain"
          style={symbolGlow}
        />
      </div>
    </div>
  );
}
