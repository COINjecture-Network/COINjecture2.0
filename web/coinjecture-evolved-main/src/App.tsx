import { lazy, Suspense } from "react";
import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { WalletProvider } from "@/contexts/WalletContext";
import { AuthProvider, AuthModal } from "@/lib/auth";

const Index = lazy(() => import("./pages/Index"));
const SolverLab = lazy(() => import("./pages/SolverLab"));
const Cli = lazy(() => import("./pages/Cli"));
const API = lazy(() => import("./pages/API"));
const Explore = lazy(() => import("./pages/Explore"));
const Marketplace = lazy(() => import("./pages/Marketplace"));
const Whitepaper = lazy(() => import("./pages/Whitepaper"));
const Roadmap = lazy(() => import("./pages/Roadmap"));
const BountySubmit = lazy(() => import("./pages/BountySubmit"));
const Wallet = lazy(() => import("./pages/Wallet"));
const NotFound = lazy(() => import("./pages/NotFound"));

const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      const msg = error instanceof Error ? error.message : String(error);
      if (
        msg.includes("Cannot connect to RPC server") ||
        msg.includes("ERR_CONNECTION_REFUSED") ||
        msg.includes("Failed to fetch")
      ) {
        return;
      }
      console.error("Query error:", error);
    },
  }),
  defaultOptions: {
    queries: {
      retry: 2,
      retryDelay: (attempt) => Math.min(800 * 2 ** attempt, 4000),
      staleTime: 5_000,
      refetchOnWindowFocus: false,
    },
  },
});

const App = () => (
  <QueryClientProvider client={queryClient}>
    <ThemeProvider attribute="class" defaultTheme="dark" enableSystem>
      <WalletProvider>
        <AuthProvider>
          <TooltipProvider>
            <Toaster />
            <Sonner />
            <BrowserRouter>
              <AuthModal />
              <Suspense
                fallback={
                  <div className="flex h-screen w-full items-center justify-center bg-background" />
                }
              >
                <Routes>
                  <Route path="/" element={<Index />} />
                  <Route path="/solver-lab" element={<SolverLab />} />
                  <Route path="/terminal" element={<Navigate to="/solver-lab" replace />} />
                  <Route path="/cli" element={<Cli />} />
                  <Route path="/api" element={<API />} />
                  <Route path="/explore" element={<Explore />} />
                  <Route path="/metrics" element={<Navigate to="/explore" replace />} />
                  <Route path="/marketplace" element={<Marketplace />} />
                  <Route path="/whitepaper" element={<Whitepaper />} />
                  <Route path="/roadmap" element={<Roadmap />} />
                  <Route path="/bounty-submit" element={<BountySubmit />} />
                  <Route path="/wallet" element={<Wallet />} />
                  {/* ADD ALL CUSTOM ROUTES ABOVE THE CATCH-ALL "*" ROUTE */}
                  <Route path="*" element={<NotFound />} />
                </Routes>
              </Suspense>
            </BrowserRouter>
          </TooltipProvider>
        </AuthProvider>
      </WalletProvider>
    </ThemeProvider>
  </QueryClientProvider>
);

export default App;
