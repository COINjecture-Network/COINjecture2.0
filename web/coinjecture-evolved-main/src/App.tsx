import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import Index from "./pages/Index";
import Terminal from "./pages/Terminal";
import API from "./pages/API";
import Metrics from "./pages/Metrics";
import Marketplace from "./pages/Marketplace";
import Whitepaper from "./pages/Whitepaper";
import BountySubmit from "./pages/BountySubmit";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient();

const App = () => (
  <QueryClientProvider client={queryClient}>
    <ThemeProvider attribute="class" defaultTheme="dark" enableSystem>
      <TooltipProvider>
        <Toaster />
        <Sonner />
        <BrowserRouter>
          <Routes>
            <Route path="/" element={<Index />} />
            <Route path="/terminal" element={<Terminal />} />
            <Route path="/api" element={<API />} />
            <Route path="/metrics" element={<Metrics />} />
            <Route path="/marketplace" element={<Marketplace />} />
            <Route path="/whitepaper" element={<Whitepaper />} />
            <Route path="/bounty-submit" element={<BountySubmit />} />
            {/* ADD ALL CUSTOM ROUTES ABOVE THE CATCH-ALL "*" ROUTE */}
            <Route path="*" element={<NotFound />} />
          </Routes>
        </BrowserRouter>
      </TooltipProvider>
    </ThemeProvider>
  </QueryClientProvider>
);

export default App;
