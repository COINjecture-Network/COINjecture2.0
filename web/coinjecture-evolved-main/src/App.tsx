import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { WalletProvider } from "@/contexts/WalletContext";
import { AuthProvider, AuthModal } from "@/lib/auth";
import Index from "./pages/Index";
import SolverLab from "./pages/SolverLab";
import Cli from "./pages/Cli";
import API from "./pages/API";
import Metrics from "./pages/Metrics";
import Marketplace from "./pages/Marketplace";
import Whitepaper from "./pages/Whitepaper";
import Roadmap from "./pages/Roadmap";
import BountySubmit from "./pages/BountySubmit";
import Wallet from "./pages/Wallet";
import NotFound from "./pages/NotFound";

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
              <Routes>
                <Route path="/" element={<Index />} />
                <Route path="/solver-lab" element={<SolverLab />} />
                <Route path="/terminal" element={<Navigate to="/solver-lab" replace />} />
                <Route path="/cli" element={<Cli />} />
                <Route path="/api" element={<API />} />
                <Route path="/metrics" element={<Metrics />} />
                <Route path="/marketplace" element={<Marketplace />} />
                <Route path="/whitepaper" element={<Whitepaper />} />
                <Route path="/roadmap" element={<Roadmap />} />
                <Route path="/bounty-submit" element={<BountySubmit />} />
                <Route path="/wallet" element={<Wallet />} />
                {/* ADD ALL CUSTOM ROUTES ABOVE THE CATCH-ALL "*" ROUTE */}
                <Route path="*" element={<NotFound />} />
              </Routes>
            </BrowserRouter>
          </TooltipProvider>
        </AuthProvider>
      </WalletProvider>
    </ThemeProvider>
  </QueryClientProvider>
);

export default App;
