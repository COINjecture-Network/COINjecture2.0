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

export function BrandLogo({ className, size = "md" }: BrandLogoProps) {
  return (
    <div
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-xl",
        "ring-2 ring-primary/40 ring-offset-2 ring-offset-background",
        "bg-gradient-to-br from-primary/15 via-accent-purple/12 to-transparent",
        "shadow-[0_0_20px_hsl(var(--glow-primary)/0.28)]",
        outerSize[size],
        className,
      )}
    >
      <img
        src="/coinjecture-mark.png"
        alt="COINjecture"
        className={cn("rounded-md object-contain", imgSize[size])}
      />
    </div>
  );
}
