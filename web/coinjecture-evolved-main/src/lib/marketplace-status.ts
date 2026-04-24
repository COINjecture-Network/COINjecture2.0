/**
 * Marketplace listing status from JSON-RPC matches Rust `Debug` on the enum
 * (e.g. `"Open"`, `"Solved"`). The UI historically expected `"OPEN"`.
 */
export function isMarketplaceListingOpen(status: string | undefined | null): boolean {
  return (status ?? "").toLowerCase() === "open";
}
