import { Navigation } from "@/components/Navigation";
import { Hero } from "@/components/Hero";
import { MetricsSection } from "@/components/MetricsSection";
import { MarketplaceSection } from "@/components/MarketplaceSection";
import { Footer } from "@/components/Footer";

const Index = () => {
  return (
    <div className="min-h-screen">
      <Navigation />
      <Hero />
      <MetricsSection />
      <MarketplaceSection />
      <Footer />
    </div>
  );
};

export default Index;
