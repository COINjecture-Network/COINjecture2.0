import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { ApiSection } from "@/components/ApiSection";

const API = () => {
  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6">
          <ApiSection />
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default API;
