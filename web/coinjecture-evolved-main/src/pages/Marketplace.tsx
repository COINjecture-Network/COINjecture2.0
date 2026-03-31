import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { MarketplaceSection } from "@/components/MarketplaceSection";

const Marketplace = () => {
  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6">
          <MarketplaceSection />
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default Marketplace;
