import { Loader2 } from "lucide-react";
import { useRecentBlockRewardMetrics } from "@/lib/hooks/useRecentBlockRewardMetrics";
import { formatBeans } from "@/lib/chain-metrics";

type EmissionHintProps = {
  variant?: "compact" | "detailed";
  className?: string;
};

export function EmissionHint({ variant = "detailed", className = "" }: EmissionHintProps) {
  const { data, isLoading, isError } = useRecentBlockRewardMetrics();

  if (isLoading && !data) {
    return (
      <p className={`text-xs text-muted-foreground flex items-center gap-2 ${className}`}>
        <Loader2 className="h-3 w-3 animate-spin shrink-0" />
        Loading live block rewards…
      </p>
    );
  }

  if (isError && !data) {
    return (
      <p className={`text-xs text-muted-foreground ${className}`}>
        Could not load chain rewards — presets use defaults until RPC is reachable.
      </p>
    );
  }

  if (!data) return null;

  if (variant === "compact") {
    const label =
      data.recentAvgAtoms != null
        ? formatBeans(data.recentAvgAtoms)
        : data.latestRewardAtoms != null
          ? formatBeans(data.latestRewardAtoms)
          : null;

    if (!label) {
      return (
        <p className={`text-xs text-muted-foreground ${className}`}>
          {data.hintSecondary ?? "Block rewards appear once the chain produces coinbase blocks."}
        </p>
      );
    }

    return (
      <div className={className}>
        <div className="font-semibold text-sm">
          Recent avg block reward: {label} BEANS
        </div>
        {data.sampleCount > 0 ? (
          <p className="text-xs text-muted-foreground mt-1">
            Based on last {data.sampleCount} block{data.sampleCount === 1 ? "" : "s"} · emission decays as √W grows
          </p>
        ) : null}
      </div>
    );
  }

  return (
    <div className={`rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-xs ${className}`}>
      {data.hintPrimary ? (
        <p className="font-medium text-foreground">{data.hintPrimary}</p>
      ) : null}
      {data.hintSecondary ? (
        <p className={`text-muted-foreground ${data.hintPrimary ? "mt-1" : ""}`}>{data.hintSecondary}</p>
      ) : null}
    </div>
  );
}
