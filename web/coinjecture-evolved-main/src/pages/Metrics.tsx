import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { MetricsSection } from "@/components/MetricsSection";

const Metrics = () => {
  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6">
          <MetricsSection />
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default Metrics;
